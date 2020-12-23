#![cfg_attr(feature = "probes", no_std)]

use core::{convert::TryFrom, mem};

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

impl DataDescriptor {
    pub fn ctor(fd: u32, tag: DataTag) -> impl FnOnce(i32) -> Self {
        move |size| DataDescriptor {
            tag: tag,
            fd: fd,
            size: size,
        }
    }
}

#[repr(u32)]
#[derive(Debug)]
pub enum DataTag {
    Write,
    SendTo,
    SendMsg,
    SendMsgAncillary,

    Read,
    RecvFrom,

    Connect,
    Close,
}

pub enum Address {
    Inet {
        port: u16,
        ip: [u8; 4],
        reserved: [u8; 8],
    },
    Inet6 {
        port: u16,
        flow_info: [u8; 4],
        ip: [u8; 16],
        scope_id: [u8; 4],
    },
}

impl Address {
    pub const RAW_SIZE: usize = 28;
}

impl TryFrom<&[u8]> for Address {
    type Error = ();

    fn try_from(b: &[u8]) -> Result<Self, Self::Error> {
        let address_family = u16::from_le_bytes(TryFrom::try_from(&b[0..2]).map_err(|_| ())?);
        let port = u16::from_be_bytes(TryFrom::try_from(&b[2..4]).map_err(|_| ())?);
        match address_family {
            2 => Ok(Address::Inet {
                port: port,
                ip: TryFrom::try_from(&b[4..8]).map_err(|_| ())?,
                reserved: TryFrom::try_from(&b[8..16]).map_err(|_| ())?,
            }),
            10 => Ok(Address::Inet6 {
                port: port,
                flow_info: TryFrom::try_from(&b[4..8]).map_err(|_| ())?,
                ip: TryFrom::try_from(&b[8..24]).map_err(|_| ())?,
                scope_id: TryFrom::try_from(&b[24..28]).map_err(|_| ())?,
            }),
            _ => Err(()),
        }
    }
}

pub enum SyscallRelevantContext {
    Empty,

    Write { fd: u32, data: &'static [u8] },
    SendTo { fd: u32, data: &'static [u8] },
    SendMsg { fd: u32, message: &'static [u8] },

    Read { fd: u32, data_ptr: usize },
    RecvFrom { fd: u32, data_ptr: usize },

    Connect { fd: u32, address: &'static [u8] },
}

impl SyscallRelevantContext {
    pub fn empty() -> Self {
        let a = [0u8; mem::size_of::<Self>()];
        unsafe { mem::transmute(a) }
    }
}
