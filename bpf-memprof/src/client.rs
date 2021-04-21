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
use serde::{Serialize, ser};
use super::event::{Pod, Hex32, Hex64, CommonHeader};
use super::event::{
    KFree, KMAlloc, KMAllocNode, CacheAlloc, CacheAllocNode, CacheFree, PageAlloc, PageAllocExtFrag,
    PageAllocZoneLocked, PageFree, PageFreeBatched, PagePcpuDrain,
};
use super::event::PageFaultUser;

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

impl Serialize for Hex32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_str(&format!("{:08x}", &self.0))
    }
}

impl Serialize for Hex64 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_str(&format!("{:016x}", &self.0))
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    KFree(KFree),
    KMAlloc(KMAlloc),
    KMAllocNode(KMAllocNode),
    CacheAlloc(CacheAlloc),
    CacheAllocNode(CacheAllocNode),
    CacheFree(CacheFree),
    PageAlloc(PageAlloc),
    PageAllocExtFrag(PageAllocExtFrag),
    PageAllocZoneLocked(PageAllocZoneLocked),
    PageFree(PageFree),
    PageFreeBatched(PageFreeBatched),
    PagePcpuDrain(PagePcpuDrain),
    PageFaultUser(PageFaultUser),
}

#[derive(Serialize)]
pub struct Event {
    pub header: CommonHeader,
    pub pid: u32,
    pub event: EventKind,
}

impl RingBufferData for Event {
    type Error = u8;

    fn from_rb_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        use core::convert::TryFrom;

        if slice.len() < 0x10 {
            return Err(0);
        }

        let header = CommonHeader::from_slice(&slice[0x00..0x08]).unwrap();
        let pid = u32::from_ne_bytes(TryFrom::try_from(&slice[0x08..0x0c]).unwrap());
        let discriminant = u32::from_ne_bytes(TryFrom::try_from(&slice[0x0c..0x10]).unwrap());
        let slice = &slice[0x10..];
        let event = match discriminant {
            x if Some(x) == KFree::DISCRIMINANT => {
                EventKind::KFree(KFree::from_slice(slice).ok_or(0)?)
            },
            x if Some(x) == KMAlloc::DISCRIMINANT => {
                EventKind::KMAlloc(KMAlloc::from_slice(slice).ok_or(0)?)
            },
            x if Some(x) == KMAllocNode::DISCRIMINANT => {
                EventKind::KMAllocNode(KMAllocNode::from_slice(slice).ok_or(0)?)
            },
            x if Some(x) == CacheAlloc::DISCRIMINANT => {
                EventKind::CacheAlloc(CacheAlloc::from_slice(slice).ok_or(0)?)
            },
            x if Some(x) == CacheAllocNode::DISCRIMINANT => {
                EventKind::CacheAllocNode(CacheAllocNode::from_slice(slice).ok_or(0)?)
            },
            x if Some(x) == CacheFree::DISCRIMINANT => {
                EventKind::CacheFree(CacheFree::from_slice(slice).ok_or(0)?)
            },
            x if Some(x) == PageAlloc::DISCRIMINANT => {
                EventKind::PageAlloc(PageAlloc::from_slice(slice).ok_or(0)?)
            },
            x if Some(x) == PageAllocExtFrag::DISCRIMINANT => {
                EventKind::PageAllocExtFrag(PageAllocExtFrag::from_slice(slice).ok_or(0)?)
            },
            x if Some(x) == PageAllocZoneLocked::DISCRIMINANT => {
                EventKind::PageAllocZoneLocked(PageAllocZoneLocked::from_slice(slice).ok_or(0)?)
            },
            x if Some(x) == PageFree::DISCRIMINANT => {
                EventKind::PageFree(PageFree::from_slice(slice).ok_or(0)?)
            },
            x if Some(x) == PageFreeBatched::DISCRIMINANT => {
                EventKind::PageFreeBatched(PageFreeBatched::from_slice(slice).ok_or(0)?)
            },
            x if Some(x) == PagePcpuDrain::DISCRIMINANT => {
                EventKind::PagePcpuDrain(PagePcpuDrain::from_slice(slice).ok_or(0)?)
            },
            x if Some(x) == PageFaultUser::DISCRIMINANT => {
                EventKind::PageFaultUser(PageFaultUser::from_slice(slice).ok_or(0)?)
            },
            _ => return Err(1),
        };

        Ok(Event { header, pid, event })
    }
}
