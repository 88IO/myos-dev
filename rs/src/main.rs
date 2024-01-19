#![no_std]
#![no_main]

use uefi::prelude::*;
use core::panic::PanicInfo;
use core::fmt::Write;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[entry]
fn efi_main(_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    writeln!(system_table.stdout(), "Hello, world!").unwrap();

    loop {}
    //Status::SUCCESS
}