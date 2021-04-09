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
use super::event::Event;

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

impl RingBufferData for Event {
    type Error = u8;

    fn from_rb_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        use core::convert::TryFrom;

        if slice.len() != 40 {
            return Err(0);
        }

        match slice[0] {
            1 => Ok(Event::Brk {
                addr: u64::from_le_bytes(TryFrom::try_from(&slice[8..16]).map_err(|_| 2)?),
            }),
            2 => Ok(Event::MMap {
                addr: u64::from_le_bytes(TryFrom::try_from(&slice[8..16]).map_err(|_| 3)?),
                len: u64::from_le_bytes(TryFrom::try_from(&slice[16..24]).map_err(|_| 4)?),
                prot: u32::from_le_bytes(TryFrom::try_from(&slice[24..28]).map_err(|_| 5)?),
                flags: u32::from_le_bytes(TryFrom::try_from(&slice[28..32]).map_err(|_| 6)?),
                fd: u32::from_le_bytes(TryFrom::try_from(&slice[4..8]).map_err(|_| 7)?),
                pg_off: u64::from_le_bytes(TryFrom::try_from(&slice[32..]).map_err(|_| 8)?),
            }),
            3 => Ok(Event::MUnmap {
                addr: u64::from_le_bytes(TryFrom::try_from(&slice[8..16]).map_err(|_| 9)?),
                len: u64::from_le_bytes(TryFrom::try_from(&slice[16..24]).map_err(|_| 10)?),
            }),
            _ => Err(1),
        }
    }
}
