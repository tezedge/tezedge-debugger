// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    convert::TryFrom,
    mem,
    net::{SocketAddr, IpAddr},
};
use redbpf::{load::Loader, Module, ringbuf::RingBuffer, ringbuf_sync::RingBufferSync, HashMap, Map};
use super::{SocketId, EventId, DataDescriptor, DataTag, address::Address, bpf_code::CODE};

pub struct BpfModule(Module);

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
    Bind { id: EventId, address: SocketAddr },
    Listen { id: EventId },
    Accept { id: EventId, listen_on_fd: u32, address: SocketAddr },
    Close { id: EventId },
    Debug { id: EventId, msg: String },
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
            DataTag::Write => {
                SnifferError::write(descriptor.id, descriptor.size, data.len()).map(|(id, size)| {
                    SnifferEvent::Write {
                        id,
                        data: &data[..size],
                    }
                })
            },
            DataTag::Read => {
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
            DataTag::Bind => {
                Ok(SnifferEvent::Bind {
                    id: descriptor.id,
                    // should not fail, already checked inside bpf code
                    address: Address::try_from(data).unwrap().into(),
                })
            },
            DataTag::Listen => Ok(SnifferEvent::Listen { id: descriptor.id }),
            DataTag::Accept => {
                Ok(SnifferEvent::Accept {
                    id: descriptor.id,
                    listen_on_fd: u32::from_le_bytes(TryFrom::try_from(&data[0..4]).unwrap()),
                    address: Address::try_from(&data[4..]).unwrap().into(),
                })
            },
            DataTag::Close => Ok(SnifferEvent::Close { id: descriptor.id }),
            DataTag::Debug => {
                SnifferError::debug(descriptor.id, descriptor.size, data.len()).map(|(id, size)| {
                    let msg = hex::encode(&data[..size]);
                    SnifferEvent::Debug { id, msg }
                })
            },
        }
    }
}

impl BpfModule {
    // TODO: handle error
    pub fn load(namespace: &str) -> Self {
        let mut loaded = Loader::load(CODE).expect("Error loading BPF program");
        for probe in loaded.kprobes_mut() {
            // try to detach the kprobe, if previous run of the sniffer did not cleanup
            let _ = probe
                .detach_kprobe_namespace(namespace, &probe.name());
            probe
                .attach_kprobe_namespace(namespace, &probe.name(), 0)
                .expect(&format!("Error attaching kprobe program {}", probe.name()));
        }
        BpfModule(loaded.module)
    }

    fn main_buffer_map(&self) -> &Map {
        self
            .0
            .maps
            .iter()
            .find(|m| m.name == "main_buffer")
            .unwrap()
    }

    pub fn main_buffer(&self) -> RingBuffer {
        let rb_map = self.main_buffer_map();
        RingBuffer::from_map(&rb_map).unwrap()
    }

    pub fn main_buffer_sync(&self) -> RingBufferSync {
        let rb_map = self.main_buffer_map();
        RingBufferSync::from_map(&rb_map).unwrap()
    }

    fn connections_map(&self) -> HashMap<SocketId, u32> {
        let map = self
            .0
            .maps
            .iter()
            .find(|m| m.name == "connections")
            .unwrap();
        HashMap::new(map).unwrap()
    }

    pub fn ignore(&self, id: SocketId) {
        self.connections_map().delete(id);
    }

    fn ports_to_watch_map(&self) -> HashMap<u16, u32> {
        let map = self
            .0
            .maps
            .iter()
            .find(|m| m.name == "ports")
            .unwrap();
        HashMap::new(map).unwrap()
    }

    pub fn watch_port(&self, port: u16) {
        self.ports_to_watch_map().set(port, 1)
    }
}
