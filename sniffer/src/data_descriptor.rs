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
    //ts_start_lo: u32,
    //ts_start_hi: u32,
    ts_finish_lo: u32,
    ts_finish_hi: u32,
}

impl EventId {
    pub fn new(socket_id: SocketId, ts_start: u64, ts_finish: u64) -> Self {
        EventId {
            socket_id: socket_id,
            //ts_start_lo: (ts_start & 0xffffffff) as u32,
            //ts_start_hi: (ts_start >> 32) as u32,
            ts_finish_lo: (ts_finish & 0xffffffff) as u32,
            ts_finish_hi: (ts_finish >> 32) as u32,
        }
    }

    pub fn ts_start(&self) -> u64 {
        0//((self.ts_start_hi.clone() as u64) << 32) + (self.ts_start_lo.clone() as u64)
    }

    pub fn ts_finish(&self) -> u64 {
        ((self.ts_finish_hi.clone() as u64) << 32) + (self.ts_finish_lo.clone() as u64)
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ts_start = self.ts_start();
        let ts_finish = self.ts_finish();
        write!(f, "{}:{}..{}", self.socket_id, ts_start, ts_finish)
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
