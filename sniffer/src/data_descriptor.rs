// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::{mem, ptr, convert::TryFrom, fmt};

pub struct DataDescriptor {
    pub id: EventId,
    pub tag: DataTag,
    pub size: i32,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct EventId {
    pub socket_id: SocketId,
    pub ts_lo: u32,
    pub ts_hi: u32,
}

impl EventId {
    pub fn ts(&self) -> u64 {
        ((self.ts_hi.clone() as u64) << 32) + (self.ts_lo.clone() as u64)
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ts = ((self.ts_hi as u64) << 32) | (self.ts_lo as u64);
        write!(f, "{}:{}", self.socket_id, ts)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct SocketId {
    pub pid: u32,
    pub fd: u32,
}

impl fmt::Display for SocketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.pid, self.fd)
    }
}

impl TryFrom<&[u8]> for DataDescriptor {
    type Error = ();

    // TODO: rewrite safe
    fn try_from(v: &[u8]) -> Result<Self, Self::Error> {
        if v.len() >= mem::size_of::<Self>() {
            Ok(unsafe { ptr::read(v.as_ptr() as *const Self) })
        } else {
            Err(())
        }
    }
}

#[repr(u32)]
#[derive(Debug)]
pub enum DataTag {
    Write,
    SendTo,
    SendMsg,

    Read,
    RecvFrom,

    Connect,
    SocketName,
    Close,

    Debug,
}
