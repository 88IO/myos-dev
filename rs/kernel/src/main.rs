#![no_std]
#![no_main]

mod align;
mod console;
mod font;
mod graphics;
mod pci;

use align::Aligned64;
use console::Console;
use graphics::{PixelColor, PixelWriter};
use pci::{ClassCode, Pci};

use common::FrameBufferConfig;
use core::arch::asm;
use core::fmt::Write;
use xhci::registers::runtime::EventRingSegmentTableSizeRegister;

use core::num::NonZeroUsize;
use xhci::context::DeviceHandler;
use xhci::context::Endpoint;
use xhci::context::EndpointHandler;
use xhci::context::{Input, InputHandler};
use xhci::ring::trb::command::ConfigureEndpoint;
use xhci::{accessor::Mapper, registers::InterrupterRegisterSet, Registers};

#[derive(Clone)]
struct MemoryMapper;

impl Mapper for MemoryMapper {
    unsafe fn map(&mut self, phys_start: usize, _bytes: usize) -> NonZeroUsize {
        NonZeroUsize::new_unchecked(phys_start)
    }
    fn unmap(&mut self, _virt_start: usize, _bytes: usize) {}
}

#[repr(C)]
struct EventRingSegmentTableEntry {
    segment_base_addr: u64,
    segment_size: u16,
    _reserve1: u16,
    _reserve2: u32,
}
impl EventRingSegmentTableEntry {
    fn new(segment_base_addr: u64, segment_size: u16) -> Self {
        Self {
            segment_base_addr,
            segment_size,
            _reserve1: 0,
            _reserve2: 0,
        }
    }
}

const DEVICE_SIZE: u8 = 8;

#[no_mangle]
pub extern "sysv64" fn kernel_main(config: FrameBufferConfig) {
    let pixel_writer = PixelWriter::new(&config);
    let (horizontal_resolution, vertical_resolution) = config.mode_info.resolution();

    for x in 0..horizontal_resolution {
        for y in 0..vertical_resolution {
            pixel_writer.write(x, y, &PixelColor::new(255, 255, 255));
        }
    }

    let mut console = Console::new(
        &pixel_writer,
        PixelColor::new(0, 0, 0),
        PixelColor::new(255, 255, 255),
    );

    let (num_device, devices) = Pci::scan();

    writeln!(console, "ScanAllBus:").unwrap();

    for i in 0..num_device {
        let dev = devices[i];
        let vendor_id = dev.vendor_id();
        let class_code = dev.class_code();
        let header_type = dev.header_type();
        writeln!(
            console,
            "{}: vend {:04x}, class {}, head {:02x}",
            dev, vendor_id, class_code, header_type
        )
        .unwrap();
    }

    let xhci_dev = {
        let mut xhci_dev = None;
        for i in 0..num_device {
            if devices[i].class_code() == ClassCode::new(0x0c, 0x03, 0x30) {
                xhci_dev = Some(devices[i]);
                if devices[i].vendor_id() == 0x8086 {
                    break;
                }
            }
        }
        xhci_dev
    }
    .unwrap();

    let mmio_base = xhci_dev.bar(0).unwrap() & !0xfusize;
    writeln!(console, "mmio_base: {:x}", mmio_base).unwrap();

    let mut regs = unsafe { Registers::new(mmio_base, MemoryMapper) };

    // host controller must be halted before resetting
    {
        let mut usbcmd = regs.operational.usbcmd.read_volatile();
        usbcmd
            .clear_interrupter_enable()
            .clear_host_system_error_enable()
            .clear_host_system_error_enable();
        if !regs.operational.usbsts.read_volatile().hc_halted() {
            usbcmd.clear_run_stop();
        }

        regs.operational.usbcmd.write_volatile(usbcmd);
    }

    while !regs.operational.usbsts.read_volatile().hc_halted() {}

    // reset controller
    regs.operational.usbcmd.update_volatile(|u| {
        u.set_host_controller_reset();
    });
    while regs
        .operational
        .usbcmd
        .read_volatile()
        .host_controller_reset()
    {}
    while regs
        .operational
        .usbsts
        .read_volatile()
        .controller_not_ready()
    {}

    let max_ports = regs.capability.hcsparams1.read_volatile().number_of_ports();
    let max_slots = regs
        .capability
        .hcsparams1
        .read_volatile()
        .number_of_device_slots();
    writeln!(console, "MaxPorts: {}, MaxSlots: {}", max_ports, max_slots).unwrap();

    // Set "Max Slots Enabled" field in CONFIG.
    regs.operational.config.update_volatile(|u| {
        u.set_max_device_slots_enabled(DEVICE_SIZE);
    });

    let max_scratchpad_buffers = regs
        .capability
        .hcsparams2
        .read_volatile()
        .max_scratchpad_buffers();

    writeln!(console, "MaxScratchpadBuffers: {}", max_scratchpad_buffers).unwrap();
    if max_scratchpad_buffers > 0 {}

    // let dcaddr_arr = unsafe {
    //     core::slice::from_raw_parts(
    //         regs.operational.dcbaap.read_volatile().get() as *const *const xhci::context::Device<8>,
    //         regs.operational
    //             .config
    //             .read_volatile()
    //             .max_device_slots_enabled() as usize,
    //     )
    // };

    let mut int_reg_sets = unsafe {
        InterrupterRegisterSet::new(
            mmio_base,
            regs.capability.rtsoff.read_volatile(),
            MemoryMapper,
        )
    };
    let mut primary_interrupter = int_reg_sets.interrupter_mut(0);
    let cr_cycle_bit = true;
    let cr_write_index = 0;

    let mut cr_buf = Aligned64::new([[0u32; 4]; 8]);
    writeln!(console, "cr_buf = {:p}", cr_buf.as_ptr()).unwrap();

    regs.operational.crcr.update_volatile(|u| {
        u.set_command_ring_pointer(cr_buf.as_ptr() as u64);
        if cr_cycle_bit {
            u.set_ring_cycle_state();
        }
    });

    let mut er_buf = Aligned64::new([[0u32; 4]; 32]);
    let mut erst = Aligned64::new(EventRingSegmentTableEntry::new(
        er_buf.as_ptr() as u64,
        er_buf.len() as u16,
    ));

    primary_interrupter.erstsz.update_volatile(|u| u.set(1));

    primary_interrupter
        .erdp
        .update_volatile(|u| u.set_event_ring_dequeue_pointer(er_buf.as_ptr() as u64));

    primary_interrupter.erstba.update_volatile(|u| {
        u.set(&erst as *const Aligned64<_> as u64);
    });

    // Enable interrupt for the primary interrupter
    primary_interrupter.iman.update_volatile(|u| {
        u.set_interrupt_enable().set_0_interrupt_pending();
    });

    // Enable interrupt for the controller
    regs.operational.usbcmd.update_volatile(|u| {
        u.set_interrupter_enable();
    });

    // Configure MSI
    let bsp_local_apic_id = unsafe { (core::ptr::read(0xfee00020 as *const u32) >> 24) as u8 };
    let msi_msg_addr = 0xfee00000u32 | ((bsp_local_apic_id as u32) << 12);
    let msi_msg_data = 0xc000u32 | 0x40u32;

    xhci_dev
        .configure_msi(msi_msg_addr, msi_msg_data, 0)
        .unwrap();

    // Run the controller
    regs.operational.usbcmd.update_volatile(|u| {
        u.set_run_stop();
    });

    while regs.operational.usbsts.read_volatile().hc_halted() {}

    // if int0_reg.erdp.read_volatile().dequeue_erst_segment_index() == er_producer_cycle {
    //     writeln!(console, "ER has at least one element").unwrap();
    // } else {
    //     writeln!(console, "ER has no element").unwrap();
    // }

    // // init xhc
    // op_reg.usbcmd.update_volatile(|u| {
    //     u.set_run_stop();
    // });

    // write!(console, "CCS+CSC: ").unwrap();
    // for port_reg in regs.port_register_set.into_iter() {
    //     let port_status = port_reg.portsc;
    //     write!(
    //         console,
    //         "{}",
    //         port_status.current_connect_status() as u8
    //             + port_status.connect_status_change() as u8 * 2
    //     )
    //     .unwrap();
    // }
    // writeln!(console, "").unwrap();
    // writeln!(
    //     console,
    //     "SEGM TABLE Base={:p} Size={}",
    //     erst.as_ptr(),
    //     erst.len()
    // )
    // .unwrap();

    // for (i, seg_table_entry) in erst.iter().enumerate() {
    //     writeln!(
    //         console,
    //         "SEGM TABLE {}: Base={:p} Size={}",
    //         i, seg_table_entry.segment_base_addr as *const u8, seg_table_entry.segment_size
    //     )
    //     .unwrap();
    // }

    // // Interrupter Register Set
    // let ir0 = regs.interrupter_register_set.interrupter(0);
    // writeln!(
    //     console,
    //     "ERSTSZ={} ERSTBA={:p} ERDP={:p}",
    //     ir0.erstsz.read_volatile().get(),
    //     ir0.erstba.read_volatile().get() as *const u8,
    //     ir0.erdp.read_volatile().event_ring_dequeue_pointer() as *const u8
    // )
    // .unwrap();

    // let dcaddr_arr = unsafe {
    //     core::slice::from_raw_parts(
    //         op_reg.dcbaap.read_volatile().get() as *const *const xhci::context::Device<8>,
    //         op_reg.config.read_volatile().max_device_slots_enabled() as usize,
    //     )
    // };

    // for (slot_id, dcaddr) in dcaddr_arr.iter().enumerate() {
    //     if dcaddr.is_null() {
    //         continue;
    //     }

    //     let mut dev_context = unsafe { dcaddr.read() };
    //     let sc = dev_context.slot();
    //     writeln!(
    //         console,
    //         "DevCtx {}: Hub={}, #EP={}, slot state {:?}, usb dev addr {:02x}",
    //         slot_id,
    //         sc.hub() as u8,
    //         sc.context_entries(),
    //         sc.slot_state(),
    //         sc.usb_device_address()
    //     )
    //     .unwrap();

    //     for ep_index in 1..=sc.context_entries() as usize {
    //         let ep = dev_context.endpoint(ep_index);
    //         writeln!(
    //             console,
    //             "EP {}: state {:?}, type {:?}, MaxBSiz {}, MaxPSiz {}",
    //             ep_index,
    //             ep.endpoint_state(),
    //             ep.endpoint_type(),
    //             ep.max_burst_size(),
    //             ep.max_packet_size()
    //         )
    //         .unwrap();
    //     }

    //     let mut input_context = Input::new_32byte();
    //     let ep_enabling = 1;
    //     input_context
    //         .control_mut()
    //         .set_add_context_flag(2 * ep_enabling);
    //     {
    //         let src = dev_context.endpoint_mut(ep_enabling + 1);
    //         let dst = input_context.device_mut().endpoint_mut(ep_enabling + 1);
    //         unsafe {
    //             core::ptr::copy(
    //                 src as *mut dyn EndpointHandler as *mut Endpoint<8>,
    //                 dst as *mut dyn EndpointHandler as *mut Endpoint<8>,
    //                 1,
    //             );
    //         }
    //     }

    //     let mut cmd: ConfigureEndpoint = ConfigureEndpoint::new();
    //     if cr_cycle_bit {
    //         cmd.set_cycle_bit();
    //     } else {
    //         cmd.clear_cycle_bit();
    //     }
    //     cmd.set_slot_id(u8::try_from(slot_id).unwrap());
    //     cmd.set_input_context_pointer(&input_context as *const Input<8> as u64);

    //     cr_buf[0].copy_from_slice(&cmd.into_raw());

    //     writeln!(console, "Put a command to {:p}", cr_buf[0].as_ptr()).unwrap();

    //     writeln!(
    //         console,
    //         "writing doorbell. R/S={}, CRR={}",
    //         op_reg.usbcmd.read_volatile().run_stop(),
    //         op_reg.crcr.read_volatile().command_ring_running()
    //     )
    //     .unwrap();

    //     regs.doorbell.update_volatile_at(0, |u| {
    //         u.set_doorbell_stream_id(0).set_doorbell_target(0);
    //     });
    //     // doorbell_registers[0].DB.Write(0);

    //     writeln!(
    //         console,
    //         "doorbell written. R/S={}, CRR={}",
    //         op_reg.usbcmd.read_volatile().run_stop(),
    //         op_reg.crcr.read_volatile().command_ring_running()
    //     )
    //     .unwrap();
    // }

    writeln!(console, "Finished").unwrap();
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
