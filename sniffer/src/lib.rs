#![cfg_attr(feature = "probes", no_std)]

#[cfg(feature = "facade")]
pub mod facade;

#[cfg(feature = "facade")]
pub mod bpf_code;

#[repr(C)]
pub struct DataDescriptor {
    pub tag: DataTag,
    pub fd: u32,
    pub size: i32,
}

#[repr(u32)]
#[derive(Debug)]
pub enum DataTag {
    SendTo = 0x01,
    Write = 0x02,
    Ancillary = 0x03,
    SendMsg = 0x04,
    Connect = 0x10,
    Close = 0xff,
}
