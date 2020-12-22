#![cfg_attr(feature = "probes", no_std)]

#[cfg(feature = "facade")]
pub mod facade;

#[cfg(feature = "facade")]
pub mod bpf_code;

#[repr(C)]
pub struct DataDescriptor {
    pub tag: i32,
    pub fd: u32,
    pub offset: u32,
    pub size: u32,
    pub overall_size: u32,
}
