// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    convert::TryFrom,
    io::{self, Write},
    fmt, mem,
    net::{SocketAddr, IpAddr},
    os::unix::net::UnixStream,
    path::Path,
    str::FromStr,
};
use bpf_ring_buffer::{RingBuffer, RingBufferSync, RingBufferData};
use passfd::FdPassingExt;
use super::{EventId, DataDescriptor, DataTag};

pub enum SnifferEvent {
    Data {
        id: EventId,
        data: Vec<u8>,
        net: bool,
        incoming: bool,
    },
    Connect {
        id: EventId,
        address: SocketAddr,
    },
    Bind {
        id: EventId,
        address: SocketAddr,
    },
    Listen {
        id: EventId,
    },
    Accept {
        id: EventId,
        listen_on_fd: u32,
        address: SocketAddr,
    },
    Close {
        id: EventId,
    },
    GetFd {
        id: EventId,
    },
    Debug {
        id: EventId,
        msg: String,
    },
}

#[derive(Debug)]
pub enum SnifferError {
    SliceTooShort(usize),
    Data { id: EventId, code: SnifferErrorCode, net: bool, incoming: bool },
    AcceptBadAddress { id: EventId },
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
            _ => Ok((id, code as usize)),
        }
    }

    fn data(id: EventId, code: i32, actual_length: usize, net: bool, incoming: bool) -> Result<(EventId, usize), Self> {
        Self::code(id.clone(), code, actual_length).map_err(|code| SnifferError::Data { id, code, net, incoming })
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

impl RingBufferData for SnifferEvent {
    type Error = SnifferError;

    fn from_rb_slice(value: &[u8]) -> Result<Self, Self::Error> {
        fn parse_socket_address(b: &[u8]) -> Result<SocketAddr, ()> {
            let address_family = u16::from_le_bytes(TryFrom::try_from(&b[0..2]).map_err(|_| ())?);
            let port = u16::from_be_bytes(TryFrom::try_from(&b[2..4]).map_err(|_| ())?);
            match address_family {
                2 => {
                    let ip = <[u8; 4]>::try_from(&b[4..8]).map_err(|_| ())?;
                    Ok(SocketAddr::new(IpAddr::V4(ip.into()), port))
                },
                10 => {
                    let ip = <[u8; 16]>::try_from(&b[8..24]).map_err(|_| ())?;
                    Ok(SocketAddr::new(IpAddr::V6(ip.into()), port))
                },
                _ => Err(()),
            }
        }

        let descriptor = DataDescriptor::try_from(value)
            .map_err(|()| SnifferError::SliceTooShort(value.len()))?;
        let data = &value[mem::size_of::<DataDescriptor>()..];
        match descriptor.tag {
            DataTag::Write => {
                SnifferError::data(descriptor.id, descriptor.size, data.len(), false, false).map(|(id, size)| {
                    SnifferEvent::Data {
                        id,
                        data: data[..size].to_vec(),
                        net: false,
                        incoming: false,
                    }
                })
            },
            DataTag::Read => {
                SnifferError::data(descriptor.id, descriptor.size, data.len(), false, true).map(|(id, size)| {
                    SnifferEvent::Data {
                        id,
                        data: data[..size].to_vec(),
                        net: false,
                        incoming: true,
                    }
                })
            },
            DataTag::Send => {
                SnifferError::data(descriptor.id, descriptor.size, data.len(), true, false).map(|(id, size)| {
                    SnifferEvent::Data {
                        id,
                        data: data[..size].to_vec(),
                        net: true,
                        incoming: false,
                    }
                })
            },
            DataTag::Recv => {
                SnifferError::data(descriptor.id, descriptor.size, data.len(), true, true).map(|(id, size)| {
                    SnifferEvent::Data {
                        id,
                        data: data[..size].to_vec(),
                        net: true,
                        incoming: true,
                    }
                })
            },
            DataTag::Connect => {
                Ok(SnifferEvent::Connect {
                    id: descriptor.id,
                    // should not fail, already checked inside bpf code
                    address: parse_socket_address(data).unwrap(),
                })
            },
            DataTag::Bind => {
                Ok(SnifferEvent::Bind {
                    id: descriptor.id,
                    // should not fail, already checked inside bpf code
                    address: parse_socket_address(data).unwrap(),
                })
            },
            DataTag::Listen => Ok(SnifferEvent::Listen { id: descriptor.id }),
            DataTag::Accept => Ok(SnifferEvent::Accept {
                id: descriptor.id.clone(),
                listen_on_fd: u32::from_le_bytes(TryFrom::try_from(&data[0..4]).unwrap()),
                // should not fail, already checked inside bpf code
                // but happens, probably due to old kernel (5.8) has bad sync
                address: parse_socket_address(&data[4..])
                    .map_err(|()| SnifferError::AcceptBadAddress { id: descriptor.id })?,
            }),
            DataTag::Close => Ok(SnifferEvent::Close { id: descriptor.id }),
            DataTag::GetFd => Ok(SnifferEvent::GetFd { id: descriptor.id }),
            DataTag::Debug => {
                SnifferError::debug(descriptor.id, descriptor.size, data.len()).map(|(id, size)| {
                    let msg = hex::encode(&data[..size]);
                    SnifferEvent::Debug { id, msg }
                })
            },
        }
    }
}

pub enum Command {
    WatchPort { port: u16 },
    IgnoreConnection { pid: u32, fd: u32 },
    FetchCounter,
}

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

pub struct BpfModuleClient {
    stream: UnixStream,
}

impl BpfModuleClient {
    pub fn new<P, D>(path: P) -> io::Result<(Self, RingBuffer<D>)>
    where
        P: AsRef<Path>,
    {
        let stream = UnixStream::connect(path)?;
        let fd = stream.recv_fd()?;
        let rb = RingBuffer::new(fd, 0x40000000)?;

        Ok((BpfModuleClient { stream }, rb))
    }

    pub fn new_sync<P>(path: P) -> io::Result<(Self, RingBufferSync)>
    where
        P: AsRef<Path>,
    {
        let stream = UnixStream::connect(path)?;
        let fd = stream.recv_fd()?;
        let rb = RingBufferSync::new(fd, 0x40000000)?;

        Ok((BpfModuleClient { stream }, rb))
    }

    pub fn send_command(&mut self, cmd: Command) -> io::Result<()> {
        self.stream.write_fmt(format_args!("{}\n", cmd))
    }
}
