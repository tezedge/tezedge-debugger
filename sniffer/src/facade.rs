// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    convert::TryFrom,
    mem,
    net::{SocketAddr, IpAddr},
    str,
};
use redbpf::{load::Loader, Module as RawModule, ringbuf::RingBuffer, HashMap};
use super::{SocketId, EventId, DataDescriptor, DataTag, Address, bpf_code::CODE};

pub struct Module(RawModule);

impl From<Address> for SocketAddr {
    fn from(a: Address) -> Self {
        match a {
            Address::Inet { port, ip, .. } => SocketAddr::new(IpAddr::V4(ip.into()), port),
            Address::Inet6 { port, ip, .. } => SocketAddr::new(IpAddr::V6(ip.into()), port),
        }
    }
}

pub enum SnifferEvent<'a> {
    Write { id: EventId, data: &'a [u8] },
    Read { id: EventId, data: &'a [u8] },
    Connect { id: EventId, address: SocketAddr },
    LocalAddress { id: EventId, address: SocketAddr },
    Close { id: EventId },
    Debug { id: EventId, msg: &'a str },
}

#[derive(Debug)]
pub enum SnifferError {
    SliceTooShort(usize),
    Write { id: EventId, code: SnifferErrorCode },
    Read { id: EventId, code: SnifferErrorCode },
    Debug { id: EventId, code: SnifferErrorCode },
}

impl SnifferError {
    fn code(
        id: EventId,
        code: i32,
        actual_length: usize,
    ) -> Result<(EventId, usize), SnifferErrorCode> {
        match code {
            -14 => Err(SnifferErrorCode::Fault),
            e if e < 0 => Err(SnifferErrorCode::Unknown(e)),
            e if actual_length < (e as usize) => {
                Err(SnifferErrorCode::SliceTooShort(actual_length, e as usize))
            },
            _ => return Ok((id, code as usize)),
        }
    }

    fn write(id: EventId, code: i32, actual_length: usize) -> Result<(EventId, usize), Self> {
        Self::code(id.clone(), code, actual_length).map_err(|code| SnifferError::Write { id, code })
    }

    fn read(id: EventId, code: i32, actual_length: usize) -> Result<(EventId, usize), Self> {
        Self::code(id.clone(), code, actual_length).map_err(|code| SnifferError::Read { id, code })
    }

    fn debug(id: EventId, code: i32, actual_length: usize) -> Result<(EventId, usize), Self> {
        Self::code(id.clone(), code, actual_length).map_err(|code| SnifferError::Debug { id, code })
    }
}

#[derive(Debug)]
pub enum SnifferErrorCode {
    SliceTooShort(usize, usize),
    Unknown(i32),
    Fault,
}

impl<'a> TryFrom<&'a [u8]> for SnifferEvent<'a> {
    type Error = SnifferError;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        let descriptor = DataDescriptor::try_from(value)
            .map_err(|()| SnifferError::SliceTooShort(value.len()))?;
        let data = &value[mem::size_of::<DataDescriptor>()..];
        match descriptor.tag {
            DataTag::Write | DataTag::SendTo | DataTag::SendMsg => {
                SnifferError::write(descriptor.id, descriptor.size, data.len()).map(|(id, size)| {
                    SnifferEvent::Write {
                        id,
                        data: &data[..size],
                    }
                })
            },
            DataTag::Read | DataTag::RecvFrom => {
                SnifferError::read(descriptor.id, descriptor.size, data.len()).map(|(id, size)| {
                    SnifferEvent::Read {
                        id,
                        data: &data[..size],
                    }
                })
            },
            DataTag::Connect => {
                Ok(SnifferEvent::Connect {
                    id: descriptor.id,
                    // should not fail, already checked inside bpf code
                    address: Address::try_from(data).unwrap().into(),
                })
            },
            DataTag::SocketName => {
                Ok(SnifferEvent::LocalAddress {
                    id: descriptor.id,
                    // should not fail, already checked inside bpf code
                    address: Address::try_from(data).unwrap().into(),
                })
            },
            DataTag::Close => Ok(SnifferEvent::Close { id: descriptor.id }),
            DataTag::Debug => {
                SnifferError::debug(descriptor.id, descriptor.size, data.len()).map(|(id, size)| {
                    let msg = str::from_utf8(&data[..size]).unwrap();
                    SnifferEvent::Debug { id, msg }
                })
            },
        }
    }
}

impl Module {
    // TODO: handle error
    pub fn load() -> Self {
        let mut loaded = Loader::load(CODE).expect("Error loading BPF program");
        for probe in loaded.kprobes_mut() {
            probe
                .attach_kprobe(&probe.name(), 0)
                .expect(&format!("Error attaching kprobe program {}", probe.name()));
        }
        Module(loaded.module)
    }

    pub fn main_buffer(&self) -> RingBuffer {
        let rb_map = self
            .0
            .maps
            .iter()
            .find(|m| m.name == "main_buffer")
            .unwrap();
        RingBuffer::from_map(&rb_map).unwrap()
    }

    fn outgoing_connections_map(&self) -> HashMap<SocketId, u32> {
        let map = self
            .0
            .maps
            .iter()
            .find(|m| m.name == "outgoing_connections")
            .unwrap();
        HashMap::new(map).unwrap()
    }

    pub fn ignore(&self, id: SocketId) {
        self.outgoing_connections_map().delete(id);
    }
}
