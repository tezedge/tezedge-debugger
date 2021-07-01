// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#[derive(Clone)]
pub struct SyscallContextFull {
    pub inner: SyscallContext,
    pub ts: u64,
}

#[derive(Clone)]
pub enum SyscallContext {
    Empty,

    Bind {
        fd: u32,
        address: &'static [u8],
    },
    Listen {
        fd: u32,
        unused: usize,
    },
    Connect {
        fd: u32,
        address: &'static [u8],
    },
    Accept {
        listen_on_fd: u32,
        address: &'static [u8],
    },
    Write {
        fd: u32,
        data_ptr: usize,
    },
    Read {
        fd: u32,
        data_ptr: usize,
    },
    Send {
        fd: u32,
        data_ptr: usize,
    },
    Recv {
        fd: u32,
        data_ptr: usize,
    },
}
