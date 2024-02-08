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

use core::num::NonZeroUsize;
use xhci::context::DeviceHandler;
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
    let cap_reg = &mut regs.capability;
    let op_reg = &mut regs.operational;
    // writeln!(console,
    //     "CAPLENGTH={:02x} HCIVERSION={:04x} \
    //     DBOFF={:08x} RTSOFF={:08x} \
    //     HCSPARAMS1={:02x}.{:04x}.{:02x} \
    //     HCCPARAMS1={:04x}.{:02x}.{}.{}.{}.{}.{}.{}.{}.{}.{}.{}.{}.{}",
    //     cap_reg.caplength.read_volatile().get(),
    //     cap_reg.hciversion.read_volatile().get(),
    //     cap_reg.dboff.read_volatile().get(),
    //     cap_reg.rtsoff.read_volatile().get(),
    //     cap_reg.hcsparams1.read_volatile().number_of_ports(),
    //     cap_reg.hcsparams1.read_volatile().number_of_interrupts(),
    //     cap_reg.hcsparams1.read_volatile().number_of_device_slots(),
    //     cap_reg.hccparams1.read_volatile().xhci_extended_capabilities_pointer(),
    //     cap_reg.hccparams1.read_volatile().maximum_primary_stream_array_size(),
    //     cap_reg.hccparams1.read_volatile().contiguous_frame_id_capability(),
    //     cap_reg.hccparams1.read_volatile().stopped_edtla_capability(),
    //     cap_reg.hccparams1.read_volatile().stopped_short_packet_capability(),
    //     cap_reg.hccparams1.read_volatile().parse_all_event_data(),
    //     cap_reg.hccparams1.read_volatile().no_secondary_sid_support(),
    //     cap_reg.hccparams1.read_volatile().latency_tolerance_messaging_capability(),
    //     cap_reg.hccparams1.read_volatile().light_hc_reset_capability(),
    //     cap_reg.hccparams1.read_volatile().port_indicators(),
    //     cap_reg.hccparams1.read_volatile().port_power_control(),
    //     cap_reg.hccparams1.read_volatile().context_size(),
    //     cap_reg.hccparams1.read_volatile().bw_negotiation_capability(),
    //     cap_reg.hccparams1.read_volatile().addressing_capability()
    //     ).unwrap();

    let max_ports = cap_reg.hcsparams1.read_volatile().number_of_ports();
    let max_slots_enabled = op_reg.config.read_volatile().max_device_slots_enabled();

    writeln!(
        console,
        "max_ports = {}, max_slots_enabled = {}",
        max_ports, max_slots_enabled
    )
    .unwrap();

    // command ring
    let cr_buf = Aligned64::new([[0u32; 4]; 8]);
    let _cr_enqueue_ptr = 0usize;
    let cr_cycle_bit = 1u8;

    op_reg.crcr.update_volatile(|u| {
        u.set_command_ring_pointer(cr_buf.as_ptr() as u64);
        if cr_cycle_bit == 1 {
            u.set_ring_cycle_state();
        }
    });

    writeln!(console, "cr_buf = {:016x}", cr_buf.as_ptr() as u64,).unwrap();

    // event ring
    let mut int_reg_sets = unsafe {
        InterrupterRegisterSet::new(mmio_base, cap_reg.rtsoff.read_volatile(), MemoryMapper)
    };
    let mut int0_reg = int_reg_sets.interrupter_mut(0);
    let er_producer_cycle = 1u8;

    let erstba = int0_reg.erstba.read_volatile();
    int0_reg.erstba.write_volatile(erstba);

    let erst_size = int0_reg.erstsz.read_volatile().get();
    let erst = unsafe {
        core::slice::from_raw_parts_mut(
            int0_reg.erstba.read_volatile().get() as *mut EventRingSegmentTableEntry,
            erst_size as usize,
        )
    };
    let _erst_index = 0;

    int0_reg
        .erdp
        .update_volatile(|u| u.set_event_ring_dequeue_pointer(erst[0].segment_base_addr));

    for seg in erst.iter() {
        unsafe {
            core::ptr::write_bytes(
                seg.segment_base_addr as *mut u8,
                0,
                16 * seg.segment_size as usize,
            )
        }
    }

    if int0_reg.erdp.read_volatile().dequeue_erst_segment_index() == er_producer_cycle {
        writeln!(console, "ER has at least one element").unwrap();
    } else {
        writeln!(console, "ER has no element").unwrap();
    }

    // init xhc
    op_reg.usbcmd.update_volatile(|u| {
        u.set_run_stop();
    });

    write!(console, "CCS+CSC: ").unwrap();
    for port_reg in regs.port_register_set.into_iter() {
        let port_status = port_reg.portsc;
        write!(
            console,
            "{}",
            port_status.current_connect_status() as u8
                + port_status.connect_status_change() as u8 * 2
        )
        .unwrap();
    }
    writeln!(console, "").unwrap();
    writeln!(
        console,
        "SEGM TABLE Base={:p} Size={}",
        erst.as_ptr(),
        erst.len()
    )
    .unwrap();

    for (i, seg_table_entry) in erst.iter().enumerate() {
        writeln!(
            console,
            "SEGM TABLE {}: Base={:p} Size={}",
            i, seg_table_entry.segment_base_addr as *const u8, seg_table_entry.segment_size
        )
        .unwrap();
    }

    // Interrupter Register Set
    let ir0 = regs.interrupter_register_set.interrupter(0);
    writeln!(
        console,
        "ERSTSZ={} ERSTBA={:p} ERDP={:p}",
        ir0.erstsz.read_volatile().get(),
        ir0.erstba.read_volatile().get() as *const u8,
        ir0.erdp.read_volatile().event_ring_dequeue_pointer() as *const u8
    )
    .unwrap();

    let dcaddr_arr = unsafe {
        core::slice::from_raw_parts(
            op_reg.dcbaap.read_volatile().get() as *const *const xhci::context::Device<8>,
            op_reg.config.read_volatile().max_device_slots_enabled() as usize,
        )
    };

    let mut slot_id = -1;
    for dcaddr in dcaddr_arr {
        slot_id += 1;
        if dcaddr.is_null() {
            continue;
        }

        let dev_context = unsafe { dcaddr.read() };
        let sc = dev_context.slot();
        writeln!(
            console,
            "DevCtx {}: Hub={}, #EP={}, slot state {:?}, usb dev addr {:02x}",
            slot_id,
            sc.hub() as u8,
            sc.context_entries(),
            sc.slot_state(),
            sc.usb_device_address()
        )
        .unwrap();

        for ep_index in 1..=sc.context_entries() as usize {
            let ep = dev_context.endpoint(ep_index);
            writeln!(
                console,
                "EP {}: state {:?}, type {:?}, MaxBSiz {}, MaxPSiz {}",
                ep_index,
                ep.endpoint_state(),
                ep.endpoint_type(),
                ep.max_burst_size(),
                ep.max_packet_size()
            )
            .unwrap();
        }
        // for (size_t ep_index = 0; ep_index < sc.bits.context_entries; ++ep_index)
        // {
        //     const auto& ep = dcaddr->ep_contexts[ep_index];
        //     printf("EP %u: state %u, type %u, MaxBSiz %u, MaxPSiz %u\n",
        //         ep_index, ep.bits.ep_state, ep.bits.ep_type,
        //         ep.bits.max_burst_size, ep.bits.max_packet_size);
        // }
    }

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
