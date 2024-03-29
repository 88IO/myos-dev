#![no_std]
#![no_main]

#[macro_use]
extern crate alloc;

use common::FrameBufferConfig;
use core::arch::asm;
use elf::endian::AnyEndian;
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode, FileType, Directory};
use uefi::table::boot::{AllocateType, MemoryType};
use uefi_services::println;

const EFI_PAGE_SIZE: usize = 0x1000;

#[entry]
fn efi_main(handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    //@ initializing panic_handler
    uefi_services::init(&mut system_table).unwrap();

    let bt = system_table.boot_services();

    println!("Hello, world!");

    let mut root_dir = {
        let mut fs = bt.get_image_file_system(handle).unwrap();
        fs.open_volume().unwrap()
    };

    println!("Dump Memory Map");
    dump_memmap(&bt, &mut root_dir);

    let fb_config = read_gop(&bt);

    println!("Read Kernel file");
    let mut kernel_file = match root_dir
        .open(
            cstr16!("\\kernel.elf"),
            FileMode::Read,
            FileAttribute::empty(),
        )
        .unwrap()
        .into_type()
        .unwrap()
    {
        FileType::Regular(file) => file,
        FileType::Dir(_) => panic!(),
    };

    let kernel_size = kernel_file
        .get_boxed_info::<FileInfo>()
        .unwrap()
        .file_size() as usize;

    let buffer_pool = bt
        .allocate_pool(MemoryType::LOADER_DATA, kernel_size)
        .unwrap();

    let kernel_buffer = unsafe { core::slice::from_raw_parts_mut(buffer_pool, kernel_size) };
    kernel_file.read(kernel_buffer).unwrap();

    println!("Load Elf");
    let entry_point_addr = load_elf(&bt, kernel_buffer);

    unsafe {
        bt.free_pool(buffer_pool).unwrap();
    }

    //@start exit_boot
    let (_system_table, _memmap) = system_table.exit_boot_services(MemoryType::LOADER_DATA);
    //@end exit_boot

    //@start run_kernel
    let entry_point: extern "sysv64" fn(fb_config: FrameBufferConfig) =
        unsafe { core::mem::transmute(entry_point_addr) };
    entry_point(fb_config);
    //@end run_kernel

    println!("All done");

    loop {
        unsafe {
            asm!("hlt");
        }
    }
    #[allow(unreachable_code)]
    Status::SUCCESS
}

fn dump_memmap(bt: &BootServices, root_dir: &mut Directory) {
    let mut memmap_buf = [0u8; 4096 * 4];

    let memmap = bt.memory_map(&mut memmap_buf).unwrap();

    let mut memmap_file = match root_dir
        .open(
            cstr16!("\\memmap"),
            FileMode::CreateReadWrite,
            FileAttribute::empty(),
        )
        .unwrap()
        .into_type()
        .unwrap()
    {
        FileType::Regular(file) => file,
        FileType::Dir(_) => panic!(),
    };

    memmap_file
        .write("Index, Type, Type(name), PhysicalStart, NumberOfPages, Attribute\n".as_bytes())
        .unwrap();

    for (i, desc) in memmap.entries().enumerate() {
        let line = format!(
            "{}, {:x}, {:?}, {:08x}, {:x}, {:x}\n",
            i,
            desc.ty.0,
            desc.ty,
            desc.phys_start,
            desc.page_count,
            desc.att.bits() & 0xfffff
        );
        memmap_file.write(line.as_bytes()).unwrap();
    }
}

fn read_gop(bt: &BootServices) -> FrameBufferConfig {
    let gop_handle = bt.get_handle_for_protocol::<GraphicsOutput>().unwrap();
    let mut gop = bt
        .open_protocol_exclusive::<GraphicsOutput>(gop_handle)
        .unwrap();

    FrameBufferConfig {
        frame_buffer: gop.frame_buffer().as_mut_ptr(),
        mode_info: gop.current_mode_info(),
    }
}

fn load_elf(bt: &BootServices, kernel_buffer: &mut [u8]) -> u64 {
    let elf = elf::ElfBytes::<AnyEndian>::minimal_parse(&kernel_buffer).unwrap();

    let mut kernel_first_addr = usize::MAX;
    let mut kernel_last_addr = usize::MIN;
    for phdr in elf.segments().unwrap().iter() {
        if phdr.p_type != elf::abi::PT_LOAD {
            continue;
        }
        kernel_first_addr = kernel_first_addr.min(phdr.p_vaddr as usize);
        kernel_last_addr = kernel_last_addr.max((phdr.p_vaddr + phdr.p_memsz) as usize);
    }

    bt.allocate_pages(
        AllocateType::Address(kernel_first_addr as u64),
        MemoryType::LOADER_DATA,
        (kernel_last_addr - kernel_first_addr + EFI_PAGE_SIZE - 1) / EFI_PAGE_SIZE,
    )
    .unwrap();

    for phdr in elf.segments().unwrap().iter() {
        if phdr.p_type != elf::abi::PT_LOAD {
            continue;
        }
        let ofs = phdr.p_offset as usize; let fsize = phdr.p_filesz as usize;
        let msize = phdr.p_memsz as usize;
        let dest = unsafe { core::slice::from_raw_parts_mut(phdr.p_vaddr as *mut u8, msize) };
        dest[..fsize].copy_from_slice(&kernel_buffer[ofs..ofs + fsize]);
        dest[fsize..].fill(0);
    }

    elf.ehdr.e_entry
}