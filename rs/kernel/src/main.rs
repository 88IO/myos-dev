#![no_std]
#![no_main]

use core::arch::asm;

#[no_mangle]
pub extern "sysv64" fn kernel_main(frame_buf_base: *mut u8, frame_buf_size: usize) {
    let frame_buffer = unsafe {
        core::slice::from_raw_parts_mut(frame_buf_base, frame_buf_size)
    };
    for i in 0..frame_buffer.len() {
        frame_buffer[i] = (i % 256) as u8;
    }

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