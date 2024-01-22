#![no_std]
#![no_main]

use core::arch::asm;
use common::PixelFormat;
use common::FrameBufferConfig;

struct PixelColor {
    r: u8,
    g: u8,
    b: u8
}

struct PixelWriter<'a> {
    config: &'a FrameBufferConfig,
    _write: fn (&Self, x: usize, y: usize, color: PixelColor)
}

impl<'a> PixelWriter<'a> {
    fn new(config: &'a FrameBufferConfig) -> Self {
        match config.mode_info.pixel_format() {
            PixelFormat::Rgb => Self {
                config,
                _write: Self::write_rgb
            },
            PixelFormat::Bgr => Self {
                config,
                _write: Self::write_bgr
            },
            _ => panic!()
        }
    }

    fn write_rgb(&self, x: usize, y: usize, color: PixelColor) {
        let pixel = self.pixel_at(x, y);
        pixel[0] = color.r;
        pixel[1] = color.g;
        pixel[2] = color.b;
    }

    fn write_bgr(&self, x: usize, y: usize, color: PixelColor) {
        let pixel = self.pixel_at(x, y);
        pixel[0] = color.b;
        pixel[1] = color.g;
        pixel[2] = color.r;
    }

    fn pixel_at(&self, x: usize, y: usize) -> &mut [u8] {
        unsafe { 
            core::slice::from_raw_parts_mut(
                self.config.frame_buffer.offset(
                    4 * (self.config.mode_info.stride() * y + x) as isize),
                4
            )
        }
    }

    fn write(&self, x: usize, y: usize, color: PixelColor) {
        (self._write)(self, x, y, color);
    }
}

#[no_mangle]
pub extern "sysv64" fn kernel_main(config: FrameBufferConfig) {
    let pixel_writer = PixelWriter::new(&config);
    let (horizontal_resolution, vertical_resolution) = config.mode_info.resolution();
    
    for x in 0..horizontal_resolution {
        for y in 0..vertical_resolution {
            pixel_writer.write(x, y, PixelColor { r: 255, g: 255, b: 255 });
        }
    }
    for x in 0..200 {
        for y in 0..100 {
            pixel_writer.write(x, y, PixelColor { r: 0, g: 255, b: 0 });
        }
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