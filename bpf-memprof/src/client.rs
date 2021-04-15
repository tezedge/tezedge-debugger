// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    io::{self, Write},
    fmt,
    os::unix::net::UnixStream,
    path::Path,
};
use passfd::FdPassingExt;
use bpf_ring_buffer::{RingBufferData, RingBufferSync};
use super::event::{Event, EventKind, Stack};

pub struct Client {
    stream: UnixStream,
}

impl Client {
    pub fn new<P>(path: P) -> io::Result<(Self, RingBufferSync)>
    where
        P: AsRef<Path>,
    {
        let stream = UnixStream::connect(path)?;
        let fd = stream.recv_fd()?;
        let rb = RingBufferSync::new(fd, 0x40000000)?;

        Ok((Client { stream }, rb))
    }

    pub fn send_command<C>(&mut self, cmd: C) -> io::Result<()>
    where
        C: fmt::Display,
    {
        self.stream.write_fmt(format_args!("{}\n", cmd))
    }
}

struct HexInt(u64);

impl fmt::Debug for HexInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(&format!("0x{:016x}", self.0))
            .finish()
    }
}

impl fmt::Debug for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            &EventKind::Brk { addr } => {
                f.debug_struct("Brk")
                    .field("addr", &HexInt(addr))
                    .finish()
            },
            &EventKind::MMap { addr, len } => {
                f.debug_struct("MMap")
                    .field("addr", &HexInt(addr))
                    .field("length", &HexInt(len))
                    .finish()
            },
            &EventKind::MUnmap { addr, len } => {
                f.debug_struct("MUnmap")
                    .field("addr", &HexInt(addr))
                    .field("length", &HexInt(len))
                    .finish()
            },
        }
    }
}

impl fmt::Debug for Stack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut g = f.debug_tuple("Stack");
        g.field(&self.length);
        for &s in &self.ips {
            if s == 0 {
                break;
            }
            g.field(&HexInt(s as u64));
        }
        g.finish()
    }
}

impl RingBufferData for Event {
    type Error = u8;

    fn from_rb_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        use core::convert::TryFrom;

        const STACK_DEPTH: usize = 0x7f;
        const HEADER_SIZE: usize = 0x28;

        if slice.len() != HEADER_SIZE + (STACK_DEPTH + 1) * 8 {
            return Err(0);
        }

        let kind = match slice[0] {
            1 => EventKind::Brk {
                addr: u64::from_ne_bytes(TryFrom::try_from(&slice[0x08..0x10]).unwrap()),
            },
            2 => EventKind::MMap {
                addr: u64::from_ne_bytes(TryFrom::try_from(&slice[0x08..0x10]).unwrap()),
                len: u64::from_ne_bytes(TryFrom::try_from(&slice[0x10..0x18]).unwrap()),
            },
            3 => EventKind::MUnmap {
                addr: u64::from_ne_bytes(TryFrom::try_from(&slice[0x08..0x10]).unwrap()),
                len: u64::from_ne_bytes(TryFrom::try_from(&slice[0x10..0x18]).unwrap()),
            },
            _ => return Err(1),
        };
        let pid = u64::from_ne_bytes(TryFrom::try_from(&slice[0x20..0x28]).unwrap()) as _;

        let stack_bytes = &slice[HEADER_SIZE..];
        let code = i64::from_ne_bytes(TryFrom::try_from(&stack_bytes[..0x08]).unwrap());
        let stack = if code < 0 {
            Err(code)
        } else {
            let length = code as usize;
            let mut ips = [0; STACK_DEPTH];
            for (i, c) in stack_bytes[0x08..].chunks(0x08).enumerate() {
                ips[i] = u64::from_ne_bytes(TryFrom::try_from(c).unwrap()) as usize;
            }
            Ok(Stack {
                length,
                ips,
            })
        };

        Ok(Event { kind, pid, stack })
    }
}
