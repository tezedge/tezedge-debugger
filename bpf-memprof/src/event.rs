// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#[derive(Debug)]
pub enum Event {
    Brk {
        addr: u64,
    },
    MMap {
        addr: u64,
        len: u64,
        prot: u32,
        flags: u32,
        fd: u32,
        pg_off: u64,
    },
    MUnmap {
        addr: u64,
        len: u64,
    },
}

impl Event {
    pub fn to_bytes(&self) -> [u8; 40] {
        let mut output = [0; 40];
        match self {
            &Event::Brk { addr } => {
                output[0] = 1;
                output[8..16].clone_from_slice(&addr.to_le_bytes());
            },
            &Event::MMap { addr, len, prot, flags, fd, pg_off } => {
                output[0] = 2;
                output[8..16].clone_from_slice(&addr.to_le_bytes());
                output[16..24].clone_from_slice(&len.to_le_bytes());
                output[24..28].clone_from_slice(&prot.to_le_bytes());
                output[28..32].clone_from_slice(&flags.to_le_bytes());
                output[4..8].clone_from_slice(&fd.to_le_bytes());
                output[32..].clone_from_slice(&pg_off.to_le_bytes());
            },
            &Event::MUnmap { addr, len } => {
                output[0] = 3;
                output[8..16].clone_from_slice(&addr.to_le_bytes());
                output[16..24].clone_from_slice(&len.to_le_bytes());
            },
        }

        output
    }
}
