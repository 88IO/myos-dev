use common::{FrameBufferConfig, PixelFormat};

pub struct PixelColor {
    pub r: u8,
    pub g: u8,
    pub b: u8
}

impl PixelColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

pub struct PixelWriter<'a> {
    pub config: &'a FrameBufferConfig,
    _write: fn (&Self, usize, usize, &PixelColor)
}

impl<'a> PixelWriter<'a> {
    pub fn new(config: &'a FrameBufferConfig) -> Self {
        match config.mode_info.pixel_format() {
            PixelFormat::Rgb => Self {
                config,
                _write: |_self, x, y, color| {
                    let pixel = _self.pixel_at(x, y);
                    pixel[0] = color.r;
                    pixel[1] = color.g;
                    pixel[2] = color.b;
                }
            },
            PixelFormat::Bgr => Self {
                config,
                _write: |_self, x, y, color| {
                    let pixel = _self.pixel_at(x, y);
                    pixel[0] = color.b;
                    pixel[1] = color.g;
                    pixel[2] = color.r;
                }
            },
            _ => panic!()
        }
    }

    pub fn pixel_at(&self, x: usize, y: usize) -> &mut [u8] {
        unsafe { 
            core::slice::from_raw_parts_mut(
                self.config.frame_buffer.offset(
                    4 * (self.config.mode_info.stride() * y + x) as isize),
                4
            )
        }
    }

    pub fn write(&self, x: usize, y: usize, color: &PixelColor) {
        (self._write)(self, x, y, color);
    }
}