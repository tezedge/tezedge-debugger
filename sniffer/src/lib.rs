#![cfg_attr(feature = "probes", no_std)]

#[cfg(feature = "facade")]
pub mod facade;

#[cfg(feature = "facade")]
pub mod bpf_code;

#[repr(C)]
pub struct DataDescriptor {
    pub tag: u32,
    pub fd: u32,
    pub size: i32,
}
