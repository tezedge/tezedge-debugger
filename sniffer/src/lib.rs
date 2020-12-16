#![cfg_attr(feature = "probes", no_std)]

#[cfg(feature = "facade")]
pub mod facade;

#[repr(C)]
pub struct SnifferItem {
    pub tag: i32,
    pub fd: u32,
    pub offset: u32,
    pub size: u32,
    pub data: [u8; Self::SIZE],
}

impl SnifferItem {
    pub const SIZE: usize = 0x80;
}
