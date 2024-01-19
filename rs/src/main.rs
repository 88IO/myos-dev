#![no_std]
#![no_main]

#[macro_use]
extern crate alloc;

use uefi::prelude::*;
use uefi::proto::media::file::{File, FileMode, FileAttribute, FileType};
use core::fmt::Write;
use uefi::proto::media::fs::SimpleFileSystem;


#[entry]
fn efi_main(handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    // initializing panic_handler and logger
    uefi_services::init(&mut system_table).unwrap();

    writeln!(system_table.stdout(), "Hello, world!").unwrap();

    let mut memmap_buf= [0u8; 4096 * 4];

    let memmap = system_table.boot_services()
        .memory_map(&mut memmap_buf).unwrap();

    let mut root_dir = {
        let sp = system_table
            .boot_services()
            .get_image_file_system(handle)
            .unwrap();
        let fs = sp.get_mut().unwrap();
        fs.open_volume().unwrap()
    };
    
    let mut file = match root_dir
        .open(cstr16!("\\memmap"), FileMode::CreateReadWrite, FileAttribute::empty())
        .unwrap()
        .into_type()
        .unwrap()
    {
        FileType::Regular(file) => file,
        FileType::Dir(_) => panic!()
    };

    file.write("Index, Type, Type(name), PhysicalStart, NumberOfPages, Attribute\n".as_bytes()).unwrap();

    let mut i = 0;
    while let Some(desc) = memmap.get(i) {
        let line = format!("{}, {:x}, {:?}, {:08x}, {:x}, {:x}\n",
            i,
            desc.ty.0,
            desc.ty,
            desc.phys_start,
            desc.page_count,
            desc.att.bits() & 0xfffff);
        file.write(line.as_bytes()).unwrap();
        i += 1;
    }

    drop(file);

    loop { }

    #[allow(unreachable_code)]
    Status::SUCCESS
}