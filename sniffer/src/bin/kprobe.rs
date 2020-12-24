#![no_std]
#![no_main]
#![cfg(feature = "probes")]

use redbpf_probes::kprobe::prelude::*;
use redbpf_probes::helpers;
use core::{mem, ptr, slice, convert::TryFrom};
use sniffer::{DataDescriptor, DataTag, Address, SyscallContext, send};

program!(0xFFFFFFFE, "GPL");

const DD: usize = mem::size_of::<DataDescriptor>();

// HashMap need to store something
type HashSet<T> = HashMap<T, u32>;

#[map]
static mut main_buffer: RingBuffer = RingBuffer::with_max_length(0x40000000); // 1GiB buffer

// the key is (pid concat tgid) it identifies the single thread
// one thread can do no more then one syscall simultaneously
// max entries is the maximal number of processors on target machine, let's define 64
#[map]
static mut syscall_contexts: HashMap<u64, SyscallContext> = HashMap::with_max_entries(64);

// connected socket ipv6 ocaml
#[map]
static mut outgoing_connections: HashSet<u32> = HashSet::with_max_entries(0x1000);

// each bpf map is safe to access from multiple threads
fn syscall_contexts_map() -> &'static mut HashMap<u64, SyscallContext> {
    unsafe { &mut syscall_contexts }
}

fn rb() -> &'static mut RingBuffer {
    unsafe { &mut main_buffer }
}

fn reg_outgoing(fd: &u32) {
    unsafe { outgoing_connections.set(fd, &1) };
}

fn is_outgoing(fd: &u32) -> bool {
    if let Some(c) = unsafe { outgoing_connections.get(fd) } {
        *c == 1
    } else {
        false
    }
}

fn forget_outgoing(fd: &u32) {
    unsafe { outgoing_connections.delete(fd) };
}

#[kprobe("ksys_write")]
fn kprobe_write(regs: Registers) {
    let fd = regs.parm1() as u32;
    let data_ptr = regs.parm2() as usize;

    if !is_outgoing(&fd) {
        return;
    }

    let mut context = SyscallContext::empty();
    context = SyscallContext::Write { fd, data_ptr };
    context.push(syscall_contexts_map())
}

#[kretprobe("ksys_write")]
fn kretprobe_write(regs: Registers) {
    SyscallContext::pop_with(syscall_contexts_map(), |s| match s {
        SyscallContext::Write { fd, data_ptr } => {
            let written = regs.rc();
            if regs.is_syscall_success() && written as i64 > 0 {
                let data = unsafe { slice::from_raw_parts(data_ptr as *mut u8, written as usize) };
                send::dyn_sized::<typenum::B0>(DataTag::Write, fd, data, rb())
            }
        },
        _ => (),
    });
}

#[kprobe("ksys_read")]
fn kprobe_read(regs: Registers) {
    let fd = regs.parm1() as u32;
    let data_ptr = regs.parm2() as usize;

    if !is_outgoing(&fd) {
        return;
    }

    let mut context = SyscallContext::empty();
    context = SyscallContext::Read { fd, data_ptr };
    context.push(syscall_contexts_map())
}

#[kretprobe("ksys_read")]
fn kretprobe_read(regs: Registers) {
    SyscallContext::pop_with(syscall_contexts_map(), |s| match s {
        SyscallContext::Read { fd, data_ptr } => {
            let read = regs.rc();
            if regs.is_syscall_success() && read as i64 > 0 {
                let data = unsafe { slice::from_raw_parts(data_ptr as *mut u8, read as usize) };
                send::dyn_sized::<typenum::B0>(DataTag::Read, fd, data, rb())
            }
        },
        _ => (),
    });
}

#[kprobe("__sys_sendto")]
fn kprobe_sendto(regs: Registers) {
    let fd = regs.parm1() as u32;
    let data_ptr = regs.parm2() as usize;

    if !is_outgoing(&fd) {
        return;
    }

    let mut context = SyscallContext::empty();
    context = SyscallContext::SendTo { fd, data_ptr };
    context.push(syscall_contexts_map())
}

#[kretprobe("__sys_sendto")]
fn kretprobe_sendto(regs: Registers) {
    SyscallContext::pop_with(syscall_contexts_map(), |s| match s {
        SyscallContext::SendTo { fd, data_ptr } => {
            let written = regs.rc();
            if regs.is_syscall_success() && written as i64 > 0 {
                let data = unsafe { slice::from_raw_parts(data_ptr as *mut u8, written as usize) };
                send::dyn_sized::<typenum::B0>(DataTag::SendTo, fd, data, rb())
            }
        },
        _ => (),
    });
}

#[kprobe("__sys_recvfrom")]
fn kprobe_recvfrom(regs: Registers) {
    let fd = regs.parm1() as u32;
    let data_ptr = regs.parm2() as usize;

    if !is_outgoing(&fd) {
        return;
    }

    let mut context = SyscallContext::empty();
    context = SyscallContext::RecvFrom { fd, data_ptr };
    context.push(syscall_contexts_map())
}

#[kretprobe("__sys_recvfrom")]
fn kretprobe_recvfrom(regs: Registers) {
    SyscallContext::pop_with(syscall_contexts_map(), |s| match s {
        SyscallContext::RecvFrom { fd, data_ptr } => {
            let read = regs.rc();
            if regs.is_syscall_success() && read as i64 > 0 {
                let data = unsafe { slice::from_raw_parts(data_ptr as *mut u8, read as usize) };
                send::dyn_sized::<typenum::B0>(DataTag::RecvFrom, fd, data, rb())
            }
        },
        _ => (),
    });
}

#[repr(C)]
struct UserMessageHeader {
    msg_name: *const cty::c_void,
    msg_name_len: cty::c_int,
    msg_iov: *const IoVec,
    msg_iov_len: cty::c_long,
    msg_control: *const cty::c_void,
    msg_control_len: cty::c_long,
    msg_flags: cty::c_int,
}

#[repr(C)]
struct IoVec {
    iov_base: *const cty::c_void,
    iov_len: cty::size_t,
}

#[kprobe("__sys_sendmsg")]
fn kprobe_sendmsg(regs: Registers) {
    let fd = regs.parm1() as u32;
    let header = regs.parm2() as *const UserMessageHeader;

    if !is_outgoing(&fd) {
        return;
    }

    let header = match unsafe { helpers::bpf_probe_read_user(header) } {
        Ok(header) => header,
        Err(_) => return,
    };

    /*
    let data = unsafe {
        slice::from_raw_parts(
            header.msg_control as *const u8,
            header.msg_control_len as usize,
        )
    };
    // send it if needed
    */

    let mut io_vec = IoVec {
        iov_base: ptr::null(),
        iov_len: 0,
    };
    for i in 0..4 {
        if i >= header.msg_iov_len as isize {
            break;
        }

        let data = match unsafe { helpers::bpf_probe_read_user(header.msg_iov.offset(i)) } {
            Ok(v) => unsafe { slice::from_raw_parts(v.iov_base as *const u8, v.iov_len) },
            Err(_) => return,
        };

        send::dyn_sized::<typenum::B0>(DataTag::SendMsg, fd, data, rb())
    }
}

#[kprobe("__sys_connect")]
fn kprobe_connect(regs: Registers) {
    let fd = regs.parm1() as u32;
    let buf = regs.parm2() as *const u8;
    let size = regs.parm3() as usize;

    let address = unsafe { slice::from_raw_parts(buf, size) };

    let mut context = SyscallContext::empty();
    context = SyscallContext::Connect { fd, address };
    context.push(syscall_contexts_map())
}

#[kretprobe("__sys_connect")]
fn kretprobe_connect(regs: Registers) {
    SyscallContext::pop_with(syscall_contexts_map(), |s| match s {
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
                    reg_outgoing(&fd);
                    // Address::RAW_SIZE + size of DataDescriptor == 40
                    send::sized::<typenum::U40, typenum::B0>(DataTag::Connect, fd, address, rb())
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

    if is_outgoing(&fd) {
        forget_outgoing(&fd);
        send::sized::<typenum::U12, typenum::B0>(DataTag::Close, fd, &[], rb())
    }
}
