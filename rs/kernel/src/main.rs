#![no_std]
#![no_main]

mod graphics;
mod font;
mod console;

use graphics::{PixelColor, PixelWriter};
use console::Console;

use core::arch::asm;
use core::fmt::Write;
use common::FrameBufferConfig;

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
        &pixel_writer, PixelColor::new(0, 0, 0), PixelColor::new(255, 255, 255));


    for i in 0..60 {
        writeln!(console, "line {}", i).unwrap();
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