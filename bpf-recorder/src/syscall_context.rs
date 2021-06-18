// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::convert::TryFrom;
use bpf_recorder::DataTag;

#[derive(Clone, Copy)]
pub struct SyscallContext {
    pub data: SyscallContextData,
    pub ts: u64,
}

impl SyscallContext {
    #[allow(dead_code)]
    #[inline(always)]
    pub fn to_ne_bytes(self) -> [u8; 0x20] {
        let SyscallContext { data, ts } = self;
        let mut b = [0; 0x20];
        let (p, q) = match data {
            SyscallContextData::Empty => (0, 0),
            SyscallContextData::Bind { fd, addr_ptr, addr_len } => {
                b[..4].clone_from_slice(&0x5u32.to_ne_bytes());
                b[4..8].clone_from_slice(&fd.to_ne_bytes());
                (addr_ptr, addr_len)
            },
            SyscallContextData::Connect { fd, addr_ptr, addr_len } => {
                b[..4].clone_from_slice(&0x6u32.to_ne_bytes());
                b[4..8].clone_from_slice(&fd.to_ne_bytes());
                (addr_ptr, addr_len)
            },
            SyscallContextData::Accept { listen_on_fd, addr_ptr, addr_len } => {
                b[..4].clone_from_slice(&0x7u32.to_ne_bytes());
                b[4..8].clone_from_slice(&listen_on_fd.to_ne_bytes());
                (addr_ptr, addr_len)
            },
            SyscallContextData::Write { fd, data_ptr } => {
                b[..4].clone_from_slice(&0x8u32.to_ne_bytes());
                b[4..8].clone_from_slice(&fd.to_ne_bytes());
                (data_ptr, 0)
            },
            SyscallContextData::Read { fd, data_ptr } => {
                b[..4].clone_from_slice(&0x9u32.to_ne_bytes());
                b[4..8].clone_from_slice(&fd.to_ne_bytes());
                (data_ptr, 0)
            },
            SyscallContextData::Send { fd, data_ptr } => {
                b[..4].clone_from_slice(&0xau32.to_ne_bytes());
                b[4..8].clone_from_slice(&fd.to_ne_bytes());
                (data_ptr, 0)
            },
            SyscallContextData::Recv { fd, data_ptr } => {
                b[..4].clone_from_slice(&0xbu32.to_ne_bytes());
                b[4..8].clone_from_slice(&fd.to_ne_bytes());
                (data_ptr, 0)
            },
        };
        b[0x08..0x10].clone_from_slice(&p.to_ne_bytes());
        b[0x10..0x18].clone_from_slice(&q.to_ne_bytes());
        b[0x18..0x20].clone_from_slice(&ts.to_ne_bytes());
        b
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn from_ne_bytes(bytes: &[u8; 0x20]) -> Self {
        let data = match u32::from_ne_bytes(TryFrom::try_from(&bytes[0x00..0x04]).unwrap()) {
            0x5 => {
                let fd = u32::from_ne_bytes(TryFrom::try_from(&bytes[0x04..0x08]).unwrap());
                let addr_ptr = u64::from_ne_bytes(TryFrom::try_from(&bytes[0x08..0x10]).unwrap());
                let addr_len = u64::from_ne_bytes(TryFrom::try_from(&bytes[0x08..0x10]).unwrap());
                SyscallContextData::Bind { fd, addr_ptr, addr_len }
            },
            0x6 => {
                let fd = u32::from_ne_bytes(TryFrom::try_from(&bytes[0x04..0x08]).unwrap());
                let addr_ptr = u64::from_ne_bytes(TryFrom::try_from(&bytes[0x08..0x10]).unwrap());
                let addr_len = u64::from_ne_bytes(TryFrom::try_from(&bytes[0x08..0x10]).unwrap());
                SyscallContextData::Connect { fd, addr_ptr, addr_len }
            },
            0x7 => {
                let fd = u32::from_ne_bytes(TryFrom::try_from(&bytes[0x04..0x08]).unwrap());
                let addr_ptr = u64::from_ne_bytes(TryFrom::try_from(&bytes[0x08..0x10]).unwrap());
                let addr_len = u64::from_ne_bytes(TryFrom::try_from(&bytes[0x08..0x10]).unwrap());
                SyscallContextData::Accept { listen_on_fd: fd, addr_ptr, addr_len }
            },
            0x8 => {
                let fd = u32::from_ne_bytes(TryFrom::try_from(&bytes[0x04..0x08]).unwrap());
                let data_ptr = u64::from_ne_bytes(TryFrom::try_from(&bytes[0x08..0x10]).unwrap());
                SyscallContextData::Write { fd, data_ptr }
            },
            0x9 => {
                let fd = u32::from_ne_bytes(TryFrom::try_from(&bytes[0x04..0x08]).unwrap());
                let data_ptr = u64::from_ne_bytes(TryFrom::try_from(&bytes[0x08..0x10]).unwrap());
                SyscallContextData::Read { fd, data_ptr }
            },
            0xa => {
                let fd = u32::from_ne_bytes(TryFrom::try_from(&bytes[0x04..0x08]).unwrap());
                let data_ptr = u64::from_ne_bytes(TryFrom::try_from(&bytes[0x08..0x10]).unwrap());
                SyscallContextData::Send { fd, data_ptr }
            },
            0xb => {
                let fd = u32::from_ne_bytes(TryFrom::try_from(&bytes[0x04..0x08]).unwrap());
                let data_ptr = u64::from_ne_bytes(TryFrom::try_from(&bytes[0x08..0x10]).unwrap());
                SyscallContextData::Recv { fd, data_ptr }
            },
            _ => SyscallContextData::Empty,
        };
        let ts = u64::from_ne_bytes(TryFrom::try_from(&bytes[0x18..0x20]).unwrap());
        SyscallContext { data, ts }
    }
}

#[derive(Clone, Copy)]
pub enum SyscallContextData {
    Empty,

    Bind {
        fd: u32,
        addr_ptr: u64,
        addr_len: u64,
    },
    Connect {
        fd: u32,
        addr_ptr: u64,
        addr_len: u64,
    },
    Accept {
        listen_on_fd: u32,
        addr_ptr: u64,
        addr_len: u64,
    },
    Write {
        fd: u32,
        data_ptr: u64,
    },
    Read {
        fd: u32,
        data_ptr: u64,
    },
    Send {
        fd: u32,
        data_ptr: u64,
    },
    Recv {
        fd: u32,
        data_ptr: u64,
    },
}

impl SyscallContextData {
    #[inline(always)]
    pub fn tag(&self) -> DataTag {
        match self {
            &SyscallContextData::Empty => DataTag::Close,
            &SyscallContextData::Bind { .. } => DataTag::Bind,
            &SyscallContextData::Connect { .. } => DataTag::Connect,
            &SyscallContextData::Accept { .. } => DataTag::Accept,
            &SyscallContextData::Write { .. } => DataTag::Write,
            &SyscallContextData::Read { .. } => DataTag::Read,
            &SyscallContextData::Send { .. } => DataTag::Send,
            &SyscallContextData::Recv { .. } => DataTag::Recv,
        }
    }
}