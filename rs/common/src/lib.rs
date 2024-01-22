#![no_std]

pub use uefi::proto::console::gop::{ModeInfo, PixelFormat};

#[repr(C)]
pub struct FrameBufferConfig {
    pub frame_buffer: *mut u8,
    pub mode_info: ModeInfo
}