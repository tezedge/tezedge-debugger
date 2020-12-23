#![no_std]
#![no_main]
#![cfg(feature = "probes")]

use redbpf_probes::kprobe::prelude::*;
use redbpf_probes::helpers::{gen, bpf_get_current_pid_tgid};
use typenum::{Unsigned, Shleft};
use core::{mem, ptr, slice, convert::TryFrom};
use sniffer::{DataDescriptor, DataTag, Address, SyscallRelevantContext};

program!(0xFFFFFFFE, "GPL");

const DD: usize = mem::size_of::<DataDescriptor>();
type Dd = typenum::U12;

#[map]
static mut main_buffer: RingBuffer = RingBuffer::with_max_length(0x40000000); // 1GiB buffer

// the key is (pid concat tgid) it identifies the single thread
// one thread can do no more then one syscall simultaneously
// max entries is the maximal number of processors on target machine, let's define 64
#[map]
static mut syscall_contexts: HashMap<u64, SyscallRelevantContext> = HashMap::with_max_entries(64);

// connected socket ipv6 ocaml
#[map]
static mut outgoing_connections: HashMap<u32, [u8; Address::RAW_SIZE]> = HashMap::with_max_entries(0x1000);

#[inline(always)]
fn send_sized<S, C>(data: &[u8], header_ctor: C)
where
    S: Unsigned,
    C: FnOnce(i32) -> DataDescriptor,
{
    match unsafe { main_buffer.reserve(S::U64, 0) } {
        Ok(buffer) => {
            let to_copy = (S::USIZE - DD).min(data.len());
            let result = if to_copy > 0 {
                unsafe {
                    let source = data.as_ptr();
                    let destination = buffer.0.as_mut_ptr().offset(DD as isize);
                    gen::bpf_probe_read_user(
                        destination as *mut _,
                        to_copy as u32,
                        source as *const _,
                    )
                }
            } else {
                0
            };
            
            let copied = if result == 0 {
                to_copy as i32
            } else {
                result as i32
            };
            let descriptor = header_ctor(copied);
            unsafe {
                ptr::write(buffer.0[..DD].as_ptr() as *mut _, descriptor);
            }
            buffer.submit(0);
        },
        Err(()) => {
            // failed to allocate buffer, try allocate smaller buffer to report error
            if let Ok(buffer) = unsafe { main_buffer.reserve(DD as u64, 0) } {
                let descriptor = header_ctor(-90);
                unsafe {
                    ptr::write(buffer.0.as_ptr() as *mut _, descriptor);
                }
                buffer.submit(0);
            }
        },
    }
}

#[inline(always)]
fn send_data<C>(data: &[u8], header_ctor: C)
where
    C: FnOnce(i32) -> DataDescriptor,
{
    let length_to_send = data.len() + DD;
    if length_to_send <= Shleft::<typenum::U1, typenum::U8>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U8>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U9>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U9>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U10>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U10>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U11>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U11>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U12>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U12>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U13>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U13>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U14>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U14>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U15>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U15>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U16>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U16>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U17>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U17>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U18>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U18>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U19>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U19>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U20>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U20>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U21>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U21>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U22>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U22>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U23>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U23>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U24>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U24>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U25>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U25>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U26>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U26>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U27>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U27>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U28>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U28>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U29>::USIZE {
        send_sized::<Shleft<typenum::U1, typenum::U29>, _>(data, header_ctor)
    }
}

#[kprobe("ksys_write")]
fn kprobe_write(regs: Registers) {
    let fd = regs.parm1() as u32;
    let buf = regs.parm2() as *const u8;
    let size = regs.parm3() as usize;

    if unsafe { outgoing_connections.get(&fd).is_none() } {
        return;
    }

    let data = unsafe { slice::from_raw_parts(buf, size) };

    let id = bpf_get_current_pid_tgid();
    let mut context = SyscallRelevantContext::empty();
    context = SyscallRelevantContext::Write {
        fd: fd,
        data: data,
    };
    unsafe { syscall_contexts.set(&id, &context) }
}

#[kretprobe("ksys_write")]
fn kretprobe_write(regs: Registers) {
    if !regs.is_syscall_success() {
        return;
    }

    let id = bpf_get_current_pid_tgid();
    match unsafe { syscall_contexts.get(&id) } {
        Some(SyscallRelevantContext::Write { ref fd, ref data }) => {
            let fd = fd.clone();
            let data = data.clone();

            send_data(data, |size| DataDescriptor { tag: DataTag::Write, fd, size })
        },
        _ => (),
    }
}

#[kprobe("__sys_sendto")]
fn kprobe_sendto(regs: Registers) {
    let fd = regs.parm1() as u32;
    let buf = regs.parm2() as *const u8;
    let size = regs.parm3() as usize;

    if unsafe { outgoing_connections.get(&fd).is_none() } {
        return;
    }

    let data = unsafe { slice::from_raw_parts(buf, size) };
    send_data(data, |size| DataDescriptor { tag: DataTag::SendTo, fd, size })
}

#[repr(C)]
struct UserMessageHeader {
    msg_name: *mut cty::c_void,
    msg_name_len: cty::c_int,
    msg_iov: *mut IoVec,
    msg_iov_len: cty::c_long,
    msg_control: *mut cty::c_void,
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
    let message_header = regs.parm2() as *const cty::c_void;

    if unsafe { outgoing_connections.get(&fd).is_none() } {
        return;
    }

    let mut message_header_ = UserMessageHeader {
        msg_name: ptr::null_mut(),
        msg_name_len: 0,
        msg_iov: ptr::null_mut(),
        msg_iov_len: 0,
        msg_control: ptr::null_mut(),
        msg_control_len: 0,
        msg_flags: 0,
    };
    let _ = unsafe {
        gen::bpf_probe_read_user(
            &mut message_header_ as *mut UserMessageHeader as *mut _,
            mem::size_of::<UserMessageHeader>() as u32,
            message_header,
        )
    };

    let data = unsafe { slice::from_raw_parts(message_header_.msg_control as *const u8, message_header_.msg_control_len as usize) };
    send_data(data, |size| DataDescriptor { tag: DataTag::SendMsgAncillary, fd, size });

    let mut io_vec = IoVec {
        iov_base: ptr::null(),
        iov_len: 0,
    };
    for i in 0..4 {
        if i >= message_header_.msg_iov_len {
            break;
        }

        let _ = unsafe {
            gen::bpf_probe_read_user(
                &mut io_vec as *mut IoVec as *mut _,
                mem::size_of::<IoVec>() as u32,
                ((message_header_.msg_iov as usize) + mem::size_of::<IoVec>() * (i as usize)) as *mut _,
            )
        };

        let data = unsafe { slice::from_raw_parts(io_vec.iov_base as *const u8, io_vec.iov_len) };
        send_data(data, |size| DataDescriptor { tag: DataTag::SendMsg, fd, size })
    }
}

#[kprobe("__sys_connect")]
fn kprobe_connect(regs: Registers) {
    let fd = regs.parm1() as u32;
    let buf = regs.parm2() as *const u8;
    let size = regs.parm3() as usize;

    let address = unsafe { slice::from_raw_parts(buf, size) };

    let id = bpf_get_current_pid_tgid();
    let mut context = SyscallRelevantContext::empty();
    context = SyscallRelevantContext::Connect {
        fd: fd,
        address: address,
    };
    unsafe { syscall_contexts.set(&id, &context) }
}

#[kretprobe("__sys_connect")]
fn kretprobe_connect(regs: Registers) {
    if !regs.is_syscall_success() {
        return;
    }

    let id = bpf_get_current_pid_tgid();
    match unsafe { syscall_contexts.get(&id) } {
        Some(&SyscallRelevantContext::Connect { ref fd, ref address }) => {
            let fd = fd.clone();
            let address = address.clone();

            let mut tmp = [0; Address::RAW_SIZE];
            let read = unsafe { 
                gen::bpf_probe_read_user(
                    tmp.as_mut_ptr() as _,
                    tmp.len().min(address.len()) as u32,
                    address.as_ptr() as _,
                )
            };

            if let Ok(_) = Address::try_from(tmp.as_ref()) {
                unsafe { outgoing_connections.set(&fd, &tmp) };
                // Address::RAW_SIZE + DD == 40
                send_sized::<typenum::U40, _>(address.as_ref(), |size| DataDescriptor { tag: DataTag::Connect, fd, size })
            } else {
                // ignore connection to other type of address
                // track only ipv4 (af_inet) and ipv6 (af_inet6)
            }
        },
        _ => (),
    }
}

#[kprobe("__close_fd")]
fn kprobe_close(regs: Registers) {
    let fd = regs.parm1() as u32;

    if unsafe { outgoing_connections.get(&fd).is_none() } {
        return;
    }

    unsafe { outgoing_connections.delete(&fd) };
    send_sized::<Dd, _>(&[], |size| DataDescriptor { tag: DataTag::Close, fd, size })
}
