#![no_std]
#![no_main]

#[macro_use]
extern crate alloc;

use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::proto::media::file::{File, FileMode, FileAttribute, FileType, FileInfo};
use uefi::table::boot::{AllocateType, MemoryType};
use uefi_services::println;
//use core::fmt::Write;
use core::arch::asm;

const KERNEL_BASE_ADDR: u64 = 0x100000;
const EFI_PAGE_SIZE: usize = 0x1000;

#[entry]
fn efi_main(handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    //@ initializing panic_handler
    uefi_services::init(&mut system_table).unwrap();

    let bt = system_table.boot_services();

    println!("Hello, world!");

    //@start read_memmap
    let mut memmap_buf= [0u8; 4096 * 4];

    let memmap = bt.memory_map(&mut memmap_buf).unwrap();

    let mut root_dir = {
        let mut fs = bt
            .get_image_file_system(handle)
            .unwrap();
        fs.open_volume().unwrap()
    };
    
    let mut memmap_file = match root_dir
        .open(cstr16!("\\memmap"), FileMode::CreateReadWrite, FileAttribute::empty())
        .unwrap()
        .into_type()
        .unwrap()
    {
        FileType::Regular(file) => file,
        FileType::Dir(_) => panic!()
    };

    memmap_file.write("Index, Type, Type(name), PhysicalStart, NumberOfPages, Attribute\n".as_bytes()).unwrap();

    for (i, desc) in memmap.entries().enumerate() {
        let line = format!("{}, {:x}, {:?}, {:08x}, {:x}, {:x}\n",
            i,
            desc.ty.0,
            desc.ty,
            desc.phys_start,
            desc.page_count,
            desc.att.bits() & 0xfffff);
        memmap_file.write(line.as_bytes()).unwrap();
    }

    drop(memmap_file);
    //@end read_memmap

    //@start read_gop
    let mut gop = {
        let gop_handle = bt
            .get_handle_for_protocol::<GraphicsOutput>()
            .unwrap();
        bt
            .open_protocol_exclusive::<GraphicsOutput>(gop_handle)
            .unwrap()
    };

    let mut frame_buffer = gop.frame_buffer();
    let frame_buf_base = frame_buffer.as_mut_ptr();
    let frame_buf_size = frame_buffer.size();
    drop(gop);
    //@end read_gop

    //@start read_kernel
    let mut kernel_file = match root_dir
        .open(cstr16!("\\kernel.elf"), FileMode::Read, FileAttribute::empty())
        .unwrap()
        .into_type()
        .unwrap()
    {
        FileType::Regular(file) => file,
        FileType::Dir(_) => panic!()
    };

    let kernel_size = kernel_file.get_boxed_info::<FileInfo>().unwrap().file_size() as usize;

    bt
        .allocate_pages(
            AllocateType::Address(KERNEL_BASE_ADDR),
            MemoryType::LOADER_DATA, 
            (kernel_size + EFI_PAGE_SIZE - 1) / EFI_PAGE_SIZE
        )
        .unwrap();

    kernel_file.read(
        unsafe {
            core::slice::from_raw_parts_mut(KERNEL_BASE_ADDR as *mut u8, kernel_size)
        }
    ).unwrap();
    //@end read_kernel

    //@start exit_boot
    let (_system_table, _memmap) = system_table
        .exit_boot_services(MemoryType::LOADER_DATA);
    //@end exit_boot

    //@start run_kernel
    let entry_point: extern "sysv64" fn(frame_buf_base: *mut u8, frame_buf_size: usize) = unsafe {
        core::mem::transmute(*((KERNEL_BASE_ADDR + 24) as *const u64))
    };
    entry_point(frame_buf_base, frame_buf_size);
    //@end run_kernel

    println!("All done");

    loop { 
        unsafe { asm!("hlt"); }
    }
    #[allow(unreachable_code)]
    Status::SUCCESS
}