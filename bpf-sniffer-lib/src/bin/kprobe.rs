// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![no_std]
#![no_main]
#![cfg(feature = "probes")]

use redbpf_probes::kprobe::prelude::*;
use core::{mem, ptr};
use bpf_common::{DataTag, SocketId, EventId};
use bpf_sniffer_lib::{SyscallContext, SyscallContextFull, send, AppIo, AppProbes};

program!(0xFFFFFFFE, "GPL");

// HashMap should store something even if it is void, let's store 1u32
type HashSet<T> = HashMap<T, u32>;

#[map]
static mut main_buffer: RingBuffer = RingBuffer::with_max_length(0x40000000); // 1GiB buffer

#[map]
static mut ports: HashSet<u16> = HashSet::with_max_entries(64);

#[map]
static mut processes: HashMap<u32, u16> = HashMap::with_max_entries(64);

// connected socket
#[map]
static mut connections: HashSet<SocketId> = HashSet::with_max_entries(8192);

// the key is (pid concat tgid) it identifies the single thread
// one thread can do no more then one syscall simultaneously
// max entries is the maximal number of processors on target machine, let's define 256
#[map]
static mut syscall_contexts: HashMap<u32, SyscallContextFull> = HashMap::with_max_entries(256);

#[map]
static mut overall_counter: HashMap<u32, u64> = HashMap::with_max_entries(1);

struct App;

impl AppIo for App {
    fn rb(&mut self) -> &mut RingBuffer {
        unsafe { &mut main_buffer }
    }

    fn is_interesting_port(&self, port: u16) -> bool {
        unsafe { ports.get(&port) }.is_some()
    }

    fn reg_process(&mut self, pid: u32, port: u16) {
        unsafe { processes.set(&pid, &port) }
    }

    fn is_process(&mut self, pid: u32) -> bool {
        unsafe { processes.get(&pid) }.is_some()
    }

    fn reg_connection(&mut self, pid: u32, fd: u32, incoming: bool) {
        let socket_id = SocketId { pid, fd };
        unsafe {
            connections.delete(&socket_id);
            let v = if incoming { 2 } else { 1 };
            connections.set(&socket_id, &v)
        };    
    }

    fn is_connection(&self, pid: u32, fd: u32) -> bool {
        let socket_id = SocketId { pid, fd };
        if let Some(c) = unsafe { connections.get(&socket_id) } {
            *c == 1 || *c == 2
        } else {
            false
        }    
    }

    fn forget_connection(&mut self, pid: u32, fd: u32) {
        let socket_id = SocketId { pid, fd };
        unsafe { connections.delete(&socket_id) };
    }

    fn push_context(&mut self, thread_id: u32, pid: u32, ts: u64, context: SyscallContext) {
        let map = unsafe { &mut syscall_contexts };
        if map.get(&thread_id).is_some() {
            let id = EventId::new(SocketId { pid, fd: 0 }, ts, ts);
            send::sized::<typenum::U8, typenum::B1>(id, DataTag::Debug, 0xdeadbeef_u64.to_be_bytes().as_ref(), self.rb());
            map.delete(&thread_id);
        } else {
            let mut s = SyscallContextFull {
                inner: SyscallContext::Empty,
                ts,
            };
            // bpf validator forbids reading from stack uninitialized data
            // different variants of this enum has different length,
            unsafe { ptr::write_volatile(&mut s.inner, mem::zeroed()) };
            s.inner = context;
            map.set(&thread_id, &s);
        }
    }

    fn pop_context<H: FnOnce(&mut Self, SyscallContext, u64)>(&mut self, thread_id: u32, handler: H) {
        let map = unsafe { &mut syscall_contexts };
        match map.get(&thread_id) {
            Some(context) => {
                handler(self, context.inner.clone(), context.ts.clone());
                map.delete(&thread_id);
            },
            None => (),
        }
    }

    fn inc_counter(&mut self) {
        unsafe {
            let c = overall_counter.get(&0).cloned().unwrap_or(0) + 1;
            overall_counter.set(&0, &c);
        }
    }
}

#[kprobe("ksys_write")]
fn kprobe_write(regs: Registers) {
    App.on_data(&regs, false, false)
}

#[kretprobe("ksys_write")]
fn kretprobe_write(regs: Registers) {
    App.on_ret(&regs)
}

#[kprobe("ksys_read")]
fn kprobe_read(regs: Registers) {
    App.on_data(&regs, true, false)
}

#[kretprobe("ksys_read")]
fn kretprobe_read(regs: Registers) {
    App.on_ret(&regs)
}

#[kprobe("__sys_sendto")]
fn kprobe_sendto(regs: Registers) {
    App.on_data(&regs, false, true)
}

#[kretprobe("__sys_sendto")]
fn kretprobe_sendto(regs: Registers) {
    App.on_ret(&regs)
}

#[kprobe("__sys_recvfrom")]
fn kprobe_recvfrom(regs: Registers) {
    App.on_data(&regs, true, true)
}

#[kretprobe("__sys_recvfrom")]
fn kretprobe_recvfrom(regs: Registers) {
    App.on_ret(&regs)
}

#[kprobe("__sys_bind")]
fn kprobe_bind(regs: Registers) {
    App.on_bind(&regs)
}

#[kretprobe("__sys_bind")]
fn kretprobe_bind(regs: Registers) {
    App.on_ret(&regs)
}

#[kprobe("__sys_listen")]
fn kprobe_listen(regs: Registers) {
    App.on_listen(&regs)
}

#[kretprobe("__sys_listen")]
fn kretprobe_listen(regs: Registers) {
    App.on_ret(&regs)
}

#[kprobe("__sys_connect")]
fn kprobe_connect(regs: Registers) {
    App.on_connect(&regs, false)
}

#[kretprobe("__sys_connect")]
fn kretprobe_connect(regs: Registers) {
    App.on_ret(&regs)
}

#[kprobe("__sys_accept4")]
fn kprobe_accept(regs: Registers) {
    App.on_connect(&regs, true)
}

#[kretprobe("__sys_accept4")]
fn kretprobe_accept(regs: Registers) {
    App.on_ret(&regs)
}

#[kprobe("__close_fd")]
fn kprobe_close(regs: Registers) {
    App.on_close(&regs)
}
