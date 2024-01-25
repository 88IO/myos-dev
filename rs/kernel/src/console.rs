use crate::graphics::{PixelWriter, PixelColor};
use crate::font::{Ascii, GLYPH_HEIGHT, GLYPH_WIDTH};

pub struct Console<'a> {
    writer: &'a PixelWriter<'a>,
    cursor_col: usize,
    cursor_row: usize,
    max_cols: usize,
    max_rows: usize,
    fg_color: PixelColor,
    bg_color: PixelColor
}

impl<'a> Console<'a> {
    pub fn new(writer: &'a PixelWriter<'a>, fg_color: PixelColor, bg_color: PixelColor) -> Self {
        let (h, v) = writer.config.mode_info.resolution();

        Self { 
            writer,
            fg_color,
            bg_color,
            cursor_col: 0, 
            cursor_row: 0,
            max_cols: h / GLYPH_WIDTH,
            max_rows: v / GLYPH_HEIGHT
        }
    }

    pub fn new_line(&mut self) {
        self.cursor_col = 0;

        if self.cursor_row < self.max_rows - 1 {
            self.cursor_row += 1;
            return;
        }

        let stride = self.writer.config.mode_info.stride();
        let bufs = unsafe { 
            core::slice::from_raw_parts_mut(
                self.writer.config.frame_buffer as *mut u32,
                stride * GLYPH_HEIGHT * self.max_rows
            )
        };
        bufs.copy_within(stride * GLYPH_HEIGHT.., 0);

        for y in (self.max_rows - 1)*GLYPH_HEIGHT..self.max_rows*GLYPH_HEIGHT {
            for x in 0..self.max_cols*GLYPH_WIDTH {
                self.writer.write(x, y, &self.bg_color);
            }
        }
    }
}

impl core::fmt::Write for Console<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            match c {
                '\n' => {
                    self.new_line();
                },
                _ => {
                    if self.cursor_col == self.max_cols - 1 {
                        self.new_line();
                    }
                    self.writer.write_ascii(self.cursor_col * GLYPH_WIDTH, self.cursor_row * GLYPH_HEIGHT, c, &self.fg_color);
                    self.cursor_col += 1;
                }
            }
        }
        Ok(())
    }
}

