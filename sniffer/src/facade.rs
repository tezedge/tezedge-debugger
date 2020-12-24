use std::{
    convert::TryFrom,
    mem,
    net::{SocketAddr, IpAddr},
};
use redbpf::{load::Loader, Module as RawModule, ringbuf::RingBuffer, HashMap};
use futures::stream::StreamExt;
use super::{DataDescriptor, DataTag, Address, bpf_code::CODE};

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
    Write { fd: u32, data: &'a [u8] },
    Read { fd: u32, data: &'a [u8] },
    Connect { fd: u32, address: SocketAddr },
    LocalAddress { fd: u32, address: SocketAddr },
    Close { fd: u32 },
}

#[derive(Debug)]
pub enum SnifferError {
    SliceTooShort(usize),
    Write { fd: u32, code: SnifferErrorCode },
    Read { fd: u32, code: SnifferErrorCode },
    Connect { fd: u32 },
}

impl SnifferError {
    fn code(fd: u32, code: i32, actual_length: usize) -> Result<u32, SnifferErrorCode> {
        match code {
            -14 => Err(SnifferErrorCode::Fault),
            e if e < 0 => Err(SnifferErrorCode::Unknown(e)),
            e if actual_length < (e as usize) => {
                Err(SnifferErrorCode::SliceTooShort(actual_length, e as usize))
            },
            _ => return Ok(fd),
        }
    }

    fn write(fd: u32, code: i32, actual_length: usize) -> Result<u32, Self> {
        Self::code(fd, code, actual_length).map_err(|code| SnifferError::Write { fd, code })
    }

    fn read(fd: u32, code: i32, actual_length: usize) -> Result<u32, Self> {
        Self::code(fd, code, actual_length).map_err(|code| SnifferError::Read { fd, code })
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
                SnifferError::write(descriptor.fd, descriptor.size, data.len())
                    .map(|fd| SnifferEvent::Write { fd, data })
            },
            DataTag::Read | DataTag::RecvFrom => {
                SnifferError::read(descriptor.fd, descriptor.size, data.len())
                    .map(|fd| SnifferEvent::Read { fd, data })
            },
            DataTag::Connect => {
                Ok(SnifferEvent::Connect {
                    fd: descriptor.fd,
                    // should not fail, already checked inside bpf code
                    address: Address::try_from(data).unwrap().into(),
                })
            },
            DataTag::SocketName => {
                Ok(SnifferEvent::LocalAddress {
                    fd: descriptor.fd,
                    // should not fail, already checked inside bpf code
                    address: Address::try_from(data).unwrap().into(),
                })
            },
            DataTag::Close => Ok(SnifferEvent::Close { fd: descriptor.fd }),
        }
    }
}

pub struct Event<T> {
    pub map_name: String,
    pub items: Vec<T>,
}

impl Module {
    // TODO: handle error
    pub fn load() -> (Self, impl StreamExt<Item = Event<DataDescriptor>>) {
        let mut loaded = Loader::load(CODE).expect("Error loading BPF program");
        for probe in loaded.kprobes_mut() {
            probe
                .attach_kprobe(&probe.name(), 0)
                .expect(&format!("Error attaching kprobe program {}", probe.name()));
        }
        let events = loaded.events.map(|(map_name, items_bytes)| {
            let mut items = Vec::with_capacity(items_bytes.len());
            for bytes in items_bytes {
                match DataDescriptor::try_from(bytes.as_ref()) {
                    Ok(item) => items.push(item),
                    Err(()) => todo!("log en error"),
                }
            }
            Event {
                map_name: map_name,
                items: items,
            }
        });
        (Module(loaded.module), events)
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

    fn outgoing_connections_map(&self) -> HashMap<u32, u32> {
        let map = self
            .0
            .maps
            .iter()
            .find(|m| m.name == "outgoing_connections")
            .unwrap();
        HashMap::new(map).unwrap()
    }

    pub fn ignore(&self, fd: u32) {
        self.outgoing_connections_map().delete(fd);
    }
}
