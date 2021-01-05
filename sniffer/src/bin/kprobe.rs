// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![no_std]
#![no_main]
#![cfg(feature = "probes")]

use redbpf_probes::kprobe::prelude::*;
use redbpf_probes::helpers;
use core::{slice, convert::TryFrom};
use sniffer::{SocketId, EventId, DataTag, Address, SyscallContext, SyscallContextFull, SyscallContextKey, send};

program!(0xFFFFFFFE, "GPL");

// HashMap should store something even if it is void, let's store 1u32
type HashSet<T> = HashMap<T, u32>;

#[map]
static mut main_buffer: RingBuffer = RingBuffer::with_max_length(0x40000000); // 1GiB buffer

// the key is (pid concat tgid) it identifies the single thread
// one thread can do no more then one syscall simultaneously
// max entries is the maximal number of processors on target machine, let's define 256
#[map]
static mut syscall_contexts: HashMap<SyscallContextKey, SyscallContextFull> = HashMap::with_max_entries(256);

// connected socket ipv6 ocaml
#[map]
static mut outgoing_connections: HashSet<SocketId> = HashSet::with_max_entries(0x1000);

// each bpf map is safe to access from multiple threads
#[inline(always)]
fn syscall_contexts_map() -> &'static mut HashMap<SyscallContextKey, SyscallContextFull> {
    unsafe { &mut syscall_contexts }
}

#[inline(always)]
fn rb() -> &'static mut RingBuffer {
    unsafe { &mut main_buffer }
}

#[inline(always)]
fn reg_outgoing(id: &SocketId) {
    unsafe {
        outgoing_connections.delete(id);
        outgoing_connections.set(id, &1)
    };
}

#[inline(always)]
fn is_outgoing(id: &SocketId) -> bool {
    if let Some(c) = unsafe { outgoing_connections.get(id) } {
        *c == 1
    } else {
        false
    }
}

#[inline(always)]
fn forget_outgoing(id: &SocketId) {
    unsafe { outgoing_connections.delete(id) };
}

#[inline(always)]
fn socket_id(fd: u32) -> SocketId {
    let id = helpers::bpf_get_current_pid_tgid();

    SocketId {
        pid: (id >> 32) as u32,
        fd: fd,
    }
}

#[inline(always)]
fn event_id(fd: u32, ts0: u64) -> EventId {
    EventId::new(socket_id(fd), ts0, helpers::bpf_ktime_get_ns())
}

#[kprobe("ksys_write")]
fn kprobe_write(regs: Registers) {
    let fd = regs.parm1() as u32;
    let data_ptr = regs.parm2() as usize;

    if !is_outgoing(&socket_id(fd)) {
        return;
    }

    let context = SyscallContext::Write { fd, data_ptr };
    context.push(&regs, syscall_contexts_map(), rb());
}

#[kretprobe("ksys_write")]
fn kretprobe_write(regs: Registers) {
    SyscallContext::pop_with(&regs, syscall_contexts_map(), rb(), |s, ts| match s {
        SyscallContext::Write { fd, data_ptr } => {
            let written = regs.rc();
            if regs.is_syscall_success() && written as i64 > 0 {
                let data = unsafe { slice::from_raw_parts(data_ptr as *mut u8, written as usize) };
                let id = event_id(fd, ts);
                send::dyn_sized::<typenum::B0>(id, DataTag::Write, data, rb())
            }
        },
        _ => (),
    });
}

#[kprobe("ksys_read")]
fn kprobe_read(regs: Registers) {
    let fd = regs.parm1() as u32;
    let data_ptr = regs.parm2() as usize;

    if !is_outgoing(&socket_id(fd)) {
        return;
    }

    let context = SyscallContext::Read { fd, data_ptr };
    context.push(&regs, syscall_contexts_map(), rb());
}

#[kretprobe("ksys_read")]
fn kretprobe_read(regs: Registers) {
    SyscallContext::pop_with(&regs, syscall_contexts_map(), rb(), |s, ts| match s {
        SyscallContext::Read { fd, data_ptr } => {
            let read = regs.rc();
            if regs.is_syscall_success() && read as i64 > 0 {
                let data = unsafe { slice::from_raw_parts(data_ptr as *mut u8, read as usize) };
                let id = event_id(fd, ts);
                send::dyn_sized::<typenum::B0>(id, DataTag::Read, data, rb())
            }
        },
        _ => (),
    });
}

#[kprobe("__sys_sendto")]
fn kprobe_sendto(regs: Registers) {
    let fd = regs.parm1() as u32;
    let data_ptr = regs.parm2() as usize;

    if !is_outgoing(&socket_id(fd)) {
        return;
    }

    let context = SyscallContext::SendTo { fd, data_ptr };
    context.push(&regs, syscall_contexts_map(), rb());
}

#[kretprobe("__sys_sendto")]
fn kretprobe_sendto(regs: Registers) {
    SyscallContext::pop_with(&regs, syscall_contexts_map(), rb(), |s, ts| match s {
        SyscallContext::SendTo { fd, data_ptr } => {
            let written = regs.rc();
            if regs.is_syscall_success() && written as i64 > 0 {
                let data = unsafe { slice::from_raw_parts(data_ptr as *mut u8, written as usize) };
                let id = event_id(fd, ts);
                send::dyn_sized::<typenum::B0>(id, DataTag::SendTo, data, rb())
            }
        },
        _ => (),
    });
}

#[kprobe("__sys_recvfrom")]
fn kprobe_recvfrom(regs: Registers) {
    let fd = regs.parm1() as u32;
    let data_ptr = regs.parm2() as usize;

    if !is_outgoing(&socket_id(fd)) {
        return;
    }

    let context = SyscallContext::RecvFrom { fd, data_ptr };
    context.push(&regs, syscall_contexts_map(), rb());
}

#[kretprobe("__sys_recvfrom")]
fn kretprobe_recvfrom(regs: Registers) {
    SyscallContext::pop_with(&regs, syscall_contexts_map(), rb(), |s, ts| match s {
        SyscallContext::RecvFrom { fd, data_ptr } => {
            let read = regs.rc();
            if regs.is_syscall_success() && read as i64 > 0 {
                let data = unsafe { slice::from_raw_parts(data_ptr as *mut u8, read as usize) };
                let id = event_id(fd, ts);
                send::dyn_sized::<typenum::B0>(id, DataTag::RecvFrom, data, rb())
            }
        },
        _ => (),
    });
}

#[kprobe("__sys_connect")]
fn kprobe_connect(regs: Registers) {
    let fd = regs.parm1() as u32;
    let buf = regs.parm2() as *const u8;
    let size = regs.parm3() as usize;

    let address = unsafe { slice::from_raw_parts(buf, size) };

    let context = SyscallContext::Connect { fd, address };
    context.push(&regs, syscall_contexts_map(), rb());
}

#[kretprobe("__sys_connect")]
fn kretprobe_connect(regs: Registers) {
    SyscallContext::pop_with(&regs, syscall_contexts_map(), rb(), |s, ts| match s {
        SyscallContext::Connect { fd, address } => {
            if regs.is_syscall_success() && regs.rc() as i64 > 0 {
                let mut tmp = [0xff; Address::RAW_SIZE];
                unsafe {
                    gen::bpf_probe_read_user(
                        tmp.as_mut_ptr() as _,
                        tmp.len().min(address.len()) as u32,
                        address.as_ptr() as _,
                    )
                };

                if let Ok(_) = Address::try_from(tmp.as_ref()) {
                    let id = event_id(fd, ts);
                    reg_outgoing(&id.socket_id);
                    send::sized::<typenum::U28, typenum::B0>(id, DataTag::Connect, address, rb())
                } else {
                    // AF_UNSPEC
                    if tmp[0] == 0 && tmp[1] == 0 {
                        forget_outgoing(&socket_id(fd));
                        send::sized::<typenum::U0, typenum::B0>(event_id(fd, 0), DataTag::Close, &[], rb());
                    }
                    // ignore connection to other type of address
                    // track only ipv4 (af_inet) and ipv6 (af_inet6)
                }
            }
        },
        _ => (),
    });
}

// TODO: kretprobe
#[kprobe("__sys_listen")]
fn kprobe_listen(regs: Registers) {
    let fd = regs.parm1() as u32;

    send::sized::<typenum::U0, typenum::B0>(event_id(fd, 0), DataTag::Listen, &[], rb());
}

// TODO: kretprobe
#[kprobe("__sys_accept4")]
fn kprobe_accept4(regs: Registers) {
    let fd = regs.parm1() as u32;

    send::sized::<typenum::U0, typenum::B0>(event_id(fd, 0), DataTag::Accept, &[], rb());
}

#[kprobe("__close_fd")]
fn kprobe_close(regs: Registers) {
    let fd = regs.parm1() as u32;

    if is_outgoing(&socket_id(fd)) {
        forget_outgoing(&socket_id(fd));

        send::sized::<typenum::U0, typenum::B0>(event_id(fd, 0), DataTag::Close, &[], rb());
    }
}
