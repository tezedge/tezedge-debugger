// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    io::{self, Write},
    fmt,
    os::unix::net::UnixStream,
    path::Path,
};
use passfd::FdPassingExt;
use ebpf_user::RingBufferRegistry;
use serde::{Serialize, ser, Deserialize, de};
use super::{event::{Pod, Hex32, Hex64, CommonHeader}, STACK_MAX_DEPTH};
use super::event::{
    KFree, KMAlloc, KMAllocNode, CacheAlloc, CacheAllocNode, CacheFree, PageAlloc, PageFree,
    PageFreeBatched,
};
use super::event::{RssStat, PercpuAlloc, PercpuFree, AddToPageCache, RemoveFromPageCache};
use super::event::MigratePages;

pub struct Client {
    stream: UnixStream,
}

pub trait ClientCallback {
    fn arrive(&mut self, client: &mut Client, data: &[u8]);
}

impl Client {
    pub fn connect<P, F>(path: P, cb: F) -> io::Result<RingBufferRegistry>
    where
        P: AsRef<Path>,
        F: ClientCallback + 'static,
    {
        let stream = UnixStream::connect(path)?;
        let fd = stream.recv_fd()?;
        let mut rb = RingBufferRegistry::default();
        let mut client = Client { stream };
        let mut cb = cb;
        rb.add_fd(fd, move |data| cb.arrive(&mut client, data))
            .map_err(|_| io::Error::last_os_error())?;

        Ok(rb)
    }

    pub fn send_command<C>(&mut self, cmd: C) -> io::Result<()>
    where
        C: fmt::Display,
    {
        self.stream.write_fmt(format_args!("{}\n", cmd))
    }
}

impl<'de> Deserialize<'de> for Hex32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        use self::de::Error;

        let s = String::deserialize(deserializer)?;
        u32::from_str_radix(&s, 16)
            .map_err(|e| Error::custom(e))
            .map(Hex32)
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

impl<'de> Deserialize<'de> for Hex64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        use self::de::Error;

        let s = String::deserialize(deserializer)?;
        u64::from_str_radix(&s, 16)
            .map_err(|e| Error::custom(e))
            .map(Hex64)
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    KFree(KFree),
    KMAlloc(KMAlloc),
    KMAllocNode(KMAllocNode),
    CacheAlloc(CacheAlloc),
    CacheAllocNode(CacheAllocNode),
    CacheFree(CacheFree),
    PageAlloc(PageAlloc),
    PageFree(PageFree),
    PageFreeBatched(PageFreeBatched),
    RssStat(RssStat),
    PercpuAlloc(PercpuAlloc),
    PercpuFree(PercpuFree),
    AddToPageCache(AddToPageCache),
    RemoveFromPageCache(RemoveFromPageCache),
    MigratePages(MigratePages),
}

#[derive(Clone, PartialEq, Eq)]
pub struct Stack {
    length: usize,
    ips: [Hex64; STACK_MAX_DEPTH],
}

impl fmt::Debug for Stack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut t = f.debug_tuple("Stack");
        for ip in self.ips() {
            t.field(ip);
        }
        t.finish()
    }
}

impl Serialize for Stack {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        use self::ser::SerializeTuple;

        let mut s = serializer.serialize_tuple(self.length)?;
        for ip in self.ips() {
            s.serialize_element(ip)?;
        }
        s.end()
    }
}

impl<'de> Deserialize<'de> for Stack {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct V;

        impl<'de> de::Visitor<'de> for V {
            type Value = Stack;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "sequence of Hex64 prefixed by u64 (its count)")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let mut stack = Stack {
                    length: 0,
                    ips: [Hex64(0); STACK_MAX_DEPTH],
                };
                while let Some(ip) = seq.next_element()? {
                    stack.ips[stack.length] = ip;
                    stack.length += 1;
                }

                Ok(stack)
            }
        }

        deserializer.deserialize_seq(V)
    }
}

impl Stack {
    pub fn from_frames(f: &[u64]) -> Self {
        let mut s = Stack {
            length: f.len(),
            ips: [Hex64(0); STACK_MAX_DEPTH],
        };
        for (i, ip) in f.iter().enumerate() {
            s.ips[i].0 = *ip;
        }
        s
    }

    pub fn ips(&self) -> &[Hex64] {
        &self.ips[..self.length]
    }

    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        use std::{convert::TryFrom, mem};

        let s = mem::size_of::<u64>();
        if slice.len() < s {
            return None;
        }

        let mut stack = Stack {
            length: u64::from_ne_bytes(TryFrom::try_from(&slice[0..s]).unwrap()) as usize,
            ips: [Hex64(0); STACK_MAX_DEPTH],
        };

        let slice = &slice[s..];
        if stack.length > STACK_MAX_DEPTH || slice.len() < stack.length * s {
            return None;
        }

        for i in 0..stack.length {
            let slice = &slice[(i * s)..((i + 1) * s)];
            stack.ips[i] = Hex64(u64::from_ne_bytes(TryFrom::try_from(slice).unwrap()));
        }

        Some(stack)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Event {
    pub header: CommonHeader,
    pub pid: u32,
    pub event: EventKind,
    pub stack: Stack,
}

impl Event {
    pub fn from_slice(slice: &[u8]) -> Result<Self, u8> {
        use core::convert::TryFrom;

        if slice.len() < 0x10 {
            return Err(0);
        }

        let header = CommonHeader::from_slice(&slice[0x00..0x08]).unwrap();
        let pid = u32::from_ne_bytes(TryFrom::try_from(&slice[0x08..0x0c]).unwrap());
        let discriminant = u32::from_ne_bytes(TryFrom::try_from(&slice[0x0c..0x10]).unwrap());
        let slice = &slice[0x10..];
        let (event, size) = match discriminant {
            x if Some(x) == KFree::DISCRIMINANT => {
                (EventKind::KFree(KFree::from_slice(slice).ok_or(0)?), KFree::SIZE)
            },
            x if Some(x) == KMAlloc::DISCRIMINANT => {
                (EventKind::KMAlloc(KMAlloc::from_slice(slice).ok_or(0)?), KMAlloc::SIZE)
            },
            x if Some(x) == KMAllocNode::DISCRIMINANT => {
                (EventKind::KMAllocNode(KMAllocNode::from_slice(slice).ok_or(0)?), KMAllocNode::SIZE)
            },
            x if Some(x) == CacheAlloc::DISCRIMINANT => {
                (EventKind::CacheAlloc(CacheAlloc::from_slice(slice).ok_or(0)?), CacheAlloc::SIZE)
            },
            x if Some(x) == CacheAllocNode::DISCRIMINANT => {
                (EventKind::CacheAllocNode(CacheAllocNode::from_slice(slice).ok_or(0)?), CacheAllocNode::SIZE)
            },
            x if Some(x) == CacheFree::DISCRIMINANT => {
                (EventKind::CacheFree(CacheFree::from_slice(slice).ok_or(0)?), CacheFree::SIZE)
            },
            x if Some(x) == PageAlloc::DISCRIMINANT => {
                (EventKind::PageAlloc(PageAlloc::from_slice(slice).ok_or(0)?), PageAlloc::SIZE)
            },
            x if Some(x) == PageFree::DISCRIMINANT => {
                (EventKind::PageFree(PageFree::from_slice(slice).ok_or(0)?), PageFree::SIZE)
            },
            x if Some(x) == PageFreeBatched::DISCRIMINANT => {
                (EventKind::PageFreeBatched(PageFreeBatched::from_slice(slice).ok_or(0)?), PageFreeBatched::SIZE)
            },
            x if Some(x) == RssStat::DISCRIMINANT => {
                (EventKind::RssStat(RssStat::from_slice(slice).ok_or(0)?), RssStat::SIZE)
            },
            x if Some(x) == PercpuAlloc::DISCRIMINANT => {
                (EventKind::PercpuAlloc(PercpuAlloc::from_slice(slice).ok_or(0)?), PercpuAlloc::SIZE)
            },
            x if Some(x) == PercpuFree::DISCRIMINANT => {
                (EventKind::PercpuFree(PercpuFree::from_slice(slice).ok_or(0)?), PercpuFree::SIZE)
            },
            x if Some(x) == AddToPageCache::DISCRIMINANT => {
                (EventKind::AddToPageCache(AddToPageCache::from_slice(slice).ok_or(0)?), AddToPageCache::SIZE)
            },
            x if Some(x) == RemoveFromPageCache::DISCRIMINANT => {
                (EventKind::RemoveFromPageCache(RemoveFromPageCache::from_slice(slice).ok_or(0)?), RemoveFromPageCache::SIZE)
            },
            x if Some(x) == MigratePages::DISCRIMINANT => {
                (EventKind::MigratePages(MigratePages::from_slice(slice).ok_or(0)?), MigratePages::SIZE)
            },
            _ => return Err(1),
        };
        let slice = &slice[size..];
        let stack = Stack::from_slice(slice).ok_or(0)?;

        Ok(Event { header, pid, event, stack })
    }
}
