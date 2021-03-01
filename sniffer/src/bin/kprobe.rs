// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![no_std]
#![no_main]
#![cfg(feature = "probes")]

use redbpf_probes::kprobe::prelude::*;
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

// connected socket
#[map]
static mut connections: HashSet<SocketId> = HashSet::with_max_entries(0x8000);

#[map]
static mut ports_to_watch: HashSet<u16> = HashSet::with_max_entries(0x100);

#[map]
static mut process_ids: HashSet<u16> = HashSet::with_max_entries(0x100);

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
fn reg_connection(id: &SocketId, incoming: bool) {
    unsafe {
        connections.delete(id);
        let v = if incoming { 2 } else { 1 };
        connections.set(id, &v)
    };
}

#[inline(always)]
fn is_connection(id: &SocketId) -> bool {
    if let Some(c) = unsafe { connections.get(id) } {
        *c == 1 || *c == 2
    } else {
        false
    }
}

#[inline(always)]
fn forget_connection(id: &SocketId) {
    unsafe { connections.delete(id) };
}

#[kprobe("ksys_write")]
fn kprobe_write(regs: Registers) {
    let fd = regs.parm1() as u32;
    let data_ptr = regs.parm2() as usize;

    if !is_connection(&SocketId::this(fd)) {
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
                let id = EventId::now(fd, ts);
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

    if !is_connection(&SocketId::this(fd)) {
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
                let id = EventId::now(fd, ts);
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

    if !is_connection(&SocketId::this(fd)) {
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
                let id = EventId::now(fd, ts);
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

    if !is_connection(&SocketId::this(fd)) {
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
                let id = EventId::now(fd, ts);
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
            if regs.is_syscall_success() {
                let mut tmp = [0xff; Address::RAW_SIZE];
                unsafe {
                    gen::bpf_probe_read_user(
                        tmp.as_mut_ptr() as _,
                        tmp.len().min(address.len()) as u32,
                        address.as_ptr() as _,
                    )
                };

                if let Ok(_) = Address::try_from(tmp.as_ref()) {
                    let id = EventId::now(fd, ts);
                    reg_connection(&id.socket_id, false);
                    send::sized::<typenum::U28, typenum::B0>(id, DataTag::Connect, address, rb())
                } else {
                    // AF_UNSPEC
                    if tmp[0] == 0 && tmp[1] == 0 {
                        forget_connection(&SocketId::this(fd));
                        send::sized::<typenum::U0, typenum::B0>(EventId::now(fd, ts), DataTag::Close, &[], rb());
                    }
                    // ignore connection to other type of address
                    // track only ipv4 (af_inet) and ipv6 (af_inet6)
                }
            }
        },
        _ => (),
    });
}

#[kprobe("__sys_bind")]
fn kprobe_bind(regs: Registers) {
    let fd = regs.parm1() as u32;
    let buf = regs.parm2() as *const u8;
    let size = regs.parm3() as usize;

    let address = unsafe { slice::from_raw_parts(buf, size) };

    let context = SyscallContext::Bind { fd, address };
    context.push(&regs, syscall_contexts_map(), rb());
}

#[kretprobe("__sys_bind")]
fn kretprobe_bind(regs: Registers) {
    SyscallContext::pop_with(&regs, syscall_contexts_map(), rb(), |s, ts| match s {
        SyscallContext::Bind { fd, address } => {
            if regs.is_syscall_success() {
                let mut tmp = [0xff; Address::RAW_SIZE];
                unsafe {
                    gen::bpf_probe_read_user(
                        tmp.as_mut_ptr() as _,
                        tmp.len().min(address.len()) as u32,
                        address.as_ptr() as _,
                    )
                };

                if let Ok(_) = Address::try_from(tmp.as_ref()) {
                    let id = EventId::now(fd, ts);
                    reg_connection(&id.socket_id, false);
                    send::sized::<typenum::U28, typenum::B0>(id, DataTag::Bind, address, rb())
                } else {
                    // AF_UNSPEC
                    if tmp[0] == 0 && tmp[1] == 0 {
                        forget_connection(&SocketId::this(fd));
                        send::sized::<typenum::U0, typenum::B0>(EventId::now(fd, ts), DataTag::Close, &[], rb());
                    }
                    // ignore connection to other type of address
                    // track only ipv4 (af_inet) and ipv6 (af_inet6)
                }
            }
        },
        _ => (),
    });
}

#[kprobe("__sys_listen")]
fn kprobe_listen(regs: Registers) {
    let fd = regs.parm1() as u32;

    let context = SyscallContext::Listen { fd, unused: 0 };
    context.push(&regs, syscall_contexts_map(), rb());
}

#[kretprobe("__sys_listen")]
fn kretprobe_listen(regs: Registers) {
    SyscallContext::pop_with(&regs, syscall_contexts_map(), rb(), |s, ts| match s {
        SyscallContext::Listen { fd, unused: _ } => {
            if regs.is_syscall_success() {
                send::sized::<typenum::U0, typenum::B0>(EventId::now(fd, ts), DataTag::Listen, &[], rb());
            }
        },
        _ => (),
    });
}

#[kprobe("__sys_accept4")]
fn kprobe_accept(regs: Registers) {
    let listen_on_fd = regs.parm1() as u32;
    let buf = regs.parm2() as *const u8;
    let size = regs.parm3() as usize;

    let address = unsafe { slice::from_raw_parts(buf, size) };

    let context = SyscallContext::Accept { listen_on_fd, address };
    context.push(&regs, syscall_contexts_map(), rb());
}

#[kretprobe("__sys_accept4")]
fn kretprobe_accept(regs: Registers) {
    SyscallContext::pop_with(&regs, syscall_contexts_map(), rb(), |s, ts| match s {
        SyscallContext::Accept { listen_on_fd, address } => {
            if regs.is_syscall_success() && regs.rc() as i64 > 0 {
                let fd = regs.rc() as u32;

                let mut tmp = [0xff; Address::RAW_SIZE + 4];
                unsafe {
                    gen::bpf_probe_read_user(
                        tmp[4..].as_mut_ptr() as _,
                        Address::RAW_SIZE.min(address.len()) as u32,
                        address.as_ptr() as _,
                    )
                };

                tmp[0..4].clone_from_slice(listen_on_fd.to_le_bytes().as_ref());

                if let Ok(_) = Address::try_from(&tmp[4..]) {
                    let id = EventId::now(fd, ts);
                    reg_connection(&id.socket_id, true);
                    send::sized::<typenum::U32, typenum::B1>(id, DataTag::Accept, tmp.as_ref(), rb())
                } else {
                    // ignore connection to other type of address
                    // track only ipv4 (af_inet) and ipv6 (af_inet6)
                }
            }
        },
        _ => (),
    });
}

#[kprobe("__close_fd")]
fn kprobe_close(regs: Registers) {
    let fd = regs.parm1() as u32;

    let socket_id = SocketId::this(fd);
    if is_connection(&socket_id) {
        forget_connection(&socket_id);

        send::sized::<typenum::U0, typenum::B0>(EventId::now(fd, 0), DataTag::Close, &[], rb());
    }
}
