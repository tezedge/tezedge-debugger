// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    convert::TryFrom,
    io::{self, Write},
    mem,
    net::{SocketAddr, IpAddr},
    os::unix::net::UnixStream,
    path::Path,
};
use bpf_ring_buffer::{RingBuffer, RingBufferSync, RingBufferData};
use passfd::FdPassingExt;
use super::{EventId, DataDescriptor, DataTag, Command};

pub enum SnifferEvent {
    Data {
        id: EventId,
        data: Vec<u8>,
        net: bool,
        incoming: bool,
        error: Option<i32>,
    },
    Connect {
        id: EventId,
        address: SocketAddr,
        error: Option<i32>,
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
        address: Result<SocketAddr, i32>,
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
    Data {
        id: EventId,
        code: SnifferErrorCode,
        net: bool,
        incoming: bool,
    },
    BindBadAddress {
        id: EventId,
        code: SnifferErrorCode,
    },
    ConnectBadAddress {
        id: EventId,
        code: SnifferErrorCode,
    },
    AcceptBadAddress {
        id: EventId,
        code: SnifferErrorCode,
    },
    Debug {
        id: EventId,
        code: SnifferErrorCode,
    },
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

    fn data(
        id: EventId,
        code: i32,
        actual_length: usize,
        net: bool,
        incoming: bool,
    ) -> Result<(EventId, usize), Self> {
        Self::code(id.clone(), code, actual_length).map_err(|code| SnifferError::Data {
            id,
            code,
            net,
            incoming,
        })
    }

    fn debug(id: EventId, code: i32, actual_length: usize) -> Result<(EventId, usize), Self> {
        Self::code(id.clone(), code, actual_length).map_err(|code| SnifferError::Debug { id, code })
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SnifferErrorCode {
    SliceTooShort(usize, usize),
    Unknown(i32),
    UnknownAddressFamily(u16),
    Fault,
}

impl RingBufferData for SnifferEvent {
    type Error = SnifferError;

    fn from_rb_slice(value: &[u8]) -> Result<Self, Self::Error> {
        fn parse_socket_address(b: &[u8]) -> Result<SocketAddr, SnifferErrorCode> {
            let e = SnifferErrorCode::SliceTooShort(28, b.len());
            let address_family = u16::from_ne_bytes(TryFrom::try_from(&b[0..2]).map_err(|_| e)?);
            let port = u16::from_be_bytes(TryFrom::try_from(&b[2..4]).map_err(|_| e)?);
            match address_family {
                2 => {
                    let ip = <[u8; 4]>::try_from(&b[4..8]).map_err(|_| e)?;
                    Ok(SocketAddr::new(IpAddr::V4(ip.into()), port))
                },
                10 => {
                    let ip = <[u8; 16]>::try_from(&b[8..24]).map_err(|_| e)?;
                    Ok(SocketAddr::new(IpAddr::V6(ip.into()), port))
                },
                u => Err(SnifferErrorCode::UnknownAddressFamily(u)),
            }
        }

        let descriptor = DataDescriptor::try_from(value)
            .map_err(|()| SnifferError::SliceTooShort(value.len()))?;
        let data = &value[mem::size_of::<DataDescriptor>()..];
        let error = if descriptor.error != 0 {
            Some(descriptor.error as i32)
        } else {
            None
        };
        match descriptor.tag {
            DataTag::Write => {
                SnifferError::data(descriptor.id, descriptor.size, data.len(), false, false).map(
                    |(id, size)| SnifferEvent::Data {
                        id,
                        data: data[..size].to_vec(),
                        net: false,
                        incoming: false,
                        error,
                    },
                )
            },
            DataTag::Read => {
                SnifferError::data(descriptor.id, descriptor.size, data.len(), false, true).map(
                    |(id, size)| SnifferEvent::Data {
                        id,
                        data: data[..size].to_vec(),
                        net: false,
                        incoming: true,
                        error,
                    },
                )
            },
            DataTag::Send => {
                SnifferError::data(descriptor.id, descriptor.size, data.len(), true, false).map(
                    |(id, size)| SnifferEvent::Data {
                        id,
                        data: data[..size].to_vec(),
                        net: true,
                        incoming: false,
                        error,
                    },
                )
            },
            DataTag::Recv => {
                SnifferError::data(descriptor.id, descriptor.size, data.len(), true, true).map(
                    |(id, size)| SnifferEvent::Data {
                        id,
                        data: data[..size].to_vec(),
                        net: true,
                        incoming: true,
                        error,
                    },
                )
            },
            DataTag::Connect => Ok(SnifferEvent::Connect {
                id: descriptor.id.clone(),
                address: parse_socket_address(data).map_err(|code| {
                    SnifferError::ConnectBadAddress {
                        id: descriptor.id,
                        code,
                    }
                })?,
                error,
            }),
            DataTag::Bind => Ok(SnifferEvent::Bind {
                id: descriptor.id.clone(),
                address: parse_socket_address(data).map_err(|code| {
                    SnifferError::BindBadAddress {
                        id: descriptor.id,
                        code,
                    }
                })?,
            }),
            DataTag::Listen => Ok(SnifferEvent::Listen { id: descriptor.id }),
            DataTag::Accept => Ok(SnifferEvent::Accept {
                id: descriptor.id.clone(),
                listen_on_fd: 0,
                address: match error {
                    Some(error) => Err(error),
                    None => Ok(parse_socket_address(data).map_err(|code| {
                        SnifferError::AcceptBadAddress {
                            id: descriptor.id,
                            code,
                        }
                    })?),
                },
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
        let rb = RingBuffer::new(fd, 0x8000000)?;

        Ok((BpfModuleClient { stream }, rb))
    }

    pub fn new_sync<P>(path: P) -> io::Result<(Self, RingBufferSync)>
    where
        P: AsRef<Path>,
    {
        let stream = UnixStream::connect(path)?;
        let fd = stream.recv_fd()?;
        let rb = RingBufferSync::new(fd, 0x8000000)?;

        Ok((BpfModuleClient { stream }, rb))
    }

    pub fn send_command(&mut self, cmd: Command) -> io::Result<()> {
        self.stream.write_fmt(format_args!("{}\n", cmd))
    }
}
