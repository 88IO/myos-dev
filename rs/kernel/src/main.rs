#![no_std]
#![no_main]

mod graphics;
mod font;
mod console;
mod pci;

use graphics::{PixelColor, PixelWriter};
use console::Console;
use pci::Pci;

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

    let (num_device, devices) = Pci::scan();

    writeln!(console, "ScanAllBus:").unwrap();

    for i in 0..num_device {
        let dev = devices[i];
        let vendor_id = dev.vendor_id();
        let class_code = dev.class_code();
        let header_type = dev.header_type();
        writeln!(console, "{}.{}.{}: vend {:04x}, class {:08x}, head {:02x}",
            dev.bus, dev.device, dev.function, vendor_id, class_code, header_type).unwrap();
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