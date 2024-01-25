use crate::graphics::{PixelColor, PixelWriter};

// 8x16 glyph
pub const FONTS: &[u8; 4096] = include_bytes!("../assets/hankaku.bin");
pub const GLYPH_HEIGHT: usize = 16;
pub const GLYPH_WIDTH: usize = 8;

pub trait Ascii {
    fn write_ascii(&self, x: usize, y: usize, c: char, color: &PixelColor);
    fn write_str(&self, x: usize, y: usize, s: &str, color: &PixelColor);
}

impl Ascii for PixelWriter<'_> {
    fn write_ascii(&self, x: usize, y: usize, c: char, color: &PixelColor) {
        let ci = c as u8 as usize;
        let glyph = if ci < 256 {
            &FONTS[ci*16..(ci+1)*16]
        } else {
            &FONTS[0..16]
        };

        for dy in 0..16 {
            for dx in 0..8 {
                if glyph[dy] << dx & 0x80 != 0 {
                    self.write(x + dx, y + dy, color);
                }
            }
        }
    }

    fn write_str(&self, x: usize, y: usize, s: &str, color: &PixelColor) {
        for (i, c) in s.chars().enumerate() {
            self.write_ascii(x + 8 * i, y, c, color);
        }
    }
}