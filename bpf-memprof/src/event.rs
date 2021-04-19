// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#[cfg_attr(feature = "client", derive(serde::Serialize))]
#[cfg_attr(feature = "client", serde(tag = "type", rename_all = "snake_case"))]
pub enum EventKind {
    Brk {
        addr: u64,
    },
    MMap {
        addr: u64,
        len: u64,
        //pg_off: u64,
    },
    MUnmap {
        addr: u64,
        len: u64,
    },
    PageAlloc {
        order: u64,
    },
}

pub struct Stack {
    pub length: usize,
    pub ips: [usize; 127],
}

impl Default for Stack {
    fn default() -> Self {
        Stack {
            length: 0,
            ips: [0; 127],
        }
    }
}

impl Stack {
    pub fn ips(&self) -> &[usize] {
        &self.ips[..(self.length / 8)]
    }
}

#[cfg_attr(feature = "client", derive(Debug))]
pub struct Event {
    pub kind: EventKind,
    pub pid: u32,
    pub stack: Result<Stack, i64>,
}

impl EventKind {
    pub fn to_bytes(&self) -> [u8; 0x20] {
        let mut output = [0; 0x20];
        match self {
            &EventKind::Brk { addr } => {
                output[0x00] = 1;
                output[0x08..0x10].clone_from_slice(&addr.to_ne_bytes());
            },
            &EventKind::MMap { addr, len } => {
                output[0x00] = 2;
                output[0x08..0x10].clone_from_slice(&addr.to_ne_bytes());
                output[0x10..0x18].clone_from_slice(&len.to_ne_bytes());
            },
            &EventKind::MUnmap { addr, len } => {
                output[0x00] = 3;
                output[0x08..0x10].clone_from_slice(&addr.to_ne_bytes());
                output[0x10..0x18].clone_from_slice(&len.to_ne_bytes());
            },
            &EventKind::PageAlloc { order } => {
                output[0x00] = 4;
                output[0x08..0x10].clone_from_slice(&order.to_ne_bytes());
            },
        }
        output
    }
}
