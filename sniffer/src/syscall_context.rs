// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::{mem, ptr};
use redbpf_probes::{maps::{HashMap, RingBuffer}, helpers, registers::Registers};
use super::{data_descriptor::{EventId, DataTag}, send};

#[derive(Clone)]
pub struct SyscallContextKey {
    pid: u32,
}

#[derive(Clone)]
pub struct SyscallContextFull {
    inner: SyscallContext,
    ts: u64,
}

#[derive(Clone)]
pub enum SyscallContext {
    Empty,

    Write {
        fd: u32,
        data_ptr: usize,
    },
    SendTo {
        fd: u32,
        data_ptr: usize,
    },

    Read {
        fd: u32,
        data_ptr: usize,
    },
    RecvFrom {
        fd: u32,
        data_ptr: usize,
    },

    Connect {
        fd: u32,
        address: &'static [u8],
    },
    Bind {
        fd: u32,
        address: &'static [u8],
    },
    Listen {
        fd: u32,
        unused: usize,
    },
    Accept {
        listen_on_fd: u32,
        address: &'static [u8],
    },
}

impl SyscallContext {
    #[inline(always)]
    pub fn push(self, regs: &Registers, map: &mut HashMap<SyscallContextKey, SyscallContextFull>, rb: &mut RingBuffer) {
        let _ = regs;
        let id = helpers::bpf_get_current_pid_tgid();
        let key = SyscallContextKey {
            pid: (id & 0xffffffff) as u32,
        };
        if map.get(&key).is_some() {
            send::sized::<typenum::U8, typenum::B1>(EventId::unknown_fd(), DataTag::Debug, 0xdeadbeef_u64.to_be_bytes().as_ref(), rb);
            map.delete(&key);
        } else {
            let mut s = SyscallContextFull {
                inner: SyscallContext::Empty,
                ts: helpers::bpf_ktime_get_ns(),
            };
            // bpf validator forbids reading from stack uninitialized data
            // different variants of this enum has different length,
            unsafe { ptr::write_volatile(&mut s.inner, mem::zeroed()) };
            s.inner = self;
            map.set(&key, &s);
        }
    }

    #[inline(always)]
    pub fn pop_with<F>(regs: &Registers, map: &mut HashMap<SyscallContextKey, SyscallContextFull>, rb: &mut RingBuffer, f: F)
    where
        F: FnOnce(Self, u64),
    {
        let _ = (regs, rb);
        let id = helpers::bpf_get_current_pid_tgid();
        let key = SyscallContextKey {
            pid: (id & 0xffffffff) as u32,
        };
        match map.get(&key) {
            Some(context) => {
                f(context.inner.clone(), context.ts.clone());
                map.delete(&key);
            },
            None => (),
        }
    }
}
