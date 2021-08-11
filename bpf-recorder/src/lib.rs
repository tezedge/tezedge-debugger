// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![cfg_attr(feature = "kern", no_std)]

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
pub use self::client::{SnifferEvent, SnifferError, SnifferErrorCode, BpfModuleClient};

use core::{fmt, mem, ptr, convert::TryFrom};

#[cfg(feature = "user")]
use core::str::FromStr;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct SocketId {
    pub pid: u32,
    pub fd: u32,
}

impl SocketId {
    #[inline(always)]
    pub fn to_ne_bytes(self) -> [u8; mem::size_of::<Self>()] {
        (((self.pid as u64) << 32) + (self.fd as u64)).to_ne_bytes()
    }
}

impl fmt::Display for SocketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.pid, self.fd)
    }
}

pub enum Command {
    WatchPort { port: u16 },
    IgnoreConnection { pid: u32, fd: u32 },
    FetchCounter,
}

#[cfg(feature = "user")]
impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut words = s.split(' ');
        match words.next() {
            Some("watch_port") => {
                let port = words
                    .next()
                    .ok_or_else(|| "bad port".to_string())?
                    .parse()
                    .map_err(|e| format!("failed to parse port: {}", e))?;
                Ok(Command::WatchPort { port })
            },
            Some("ignore_connection") => {
                let pid = words
                    .next()
                    .ok_or_else(|| "bad pid".to_string())?
                    .parse()
                    .map_err(|e| format!("failed to parse pid: {}", e))?;
                let fd = words
                    .next()
                    .ok_or_else(|| "bad fd".to_string())?
                    .parse()
                    .map_err(|e| format!("failed to parse fd: {}", e))?;
                Ok(Command::IgnoreConnection { pid, fd })
            },
            Some("fetch_counter") => Ok(Command::FetchCounter),
            _ => Err("unexpected command".to_string()),
        }
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::WatchPort { port } => write!(f, "watch_port {}", port),
            Command::IgnoreConnection { pid, fd } => write!(f, "ignore_connection {} {}", pid, fd),
            Command::FetchCounter => write!(f, "fetch_counter"),
        }
    }
}

#[repr(C)]
pub struct DataDescriptor {
    pub id: EventId,
    pub tag: DataTag,
    pub error: i16,
    pub size: i32,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct EventId {
    pub socket_id: SocketId,
    //ts: Range<u64>,
    pub ts: u64,
}

impl EventId {
    #[inline(always)]
    pub fn new(socket_id: SocketId, _ts_start: u64, ts_finish: u64) -> Self {
        EventId {
            socket_id,
            ts: ts_finish,
        }
    }

    pub fn ts_start(&self) -> u64 {
        0
    }

    pub fn ts_finish(&self) -> u64 {
        self.ts
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.socket_id, self.ts)
    }
}

impl TryFrom<&[u8]> for DataDescriptor {
    type Error = ();

    // TODO: rewrite safe
    fn try_from(v: &[u8]) -> Result<Self, Self::Error> {
        if v.len() >= mem::size_of::<Self>() {
            Ok(unsafe { ptr::read(v as *const [u8] as *const Self) })
        } else {
            Err(())
        }
    }
}

#[repr(u16)]
#[derive(Debug)]
pub enum DataTag {
    Write,
    Read,
    Send,
    Recv,

    Connect,
    Bind,
    Listen,
    Accept,
    Close,

    GetFd,
    Debug,
}
