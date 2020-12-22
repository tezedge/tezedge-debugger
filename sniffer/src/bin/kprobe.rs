#![no_std]
#![no_main]
#![cfg(feature = "probes")]

use redbpf_probes::kprobe::prelude::*;
use redbpf_probes::helpers::gen;
use typenum::{Unsigned, Shleft};
use core::{mem, ptr, slice};
use sniffer::DataDescriptor;

program!(0xFFFFFFFE, "GPL");

#[map]
static mut events: PerfMap<u32> = PerfMap::with_max_entries(1);

#[map]
static mut main_buffer: RingBuffer = RingBuffer::with_max_length(0x40000000);

const DD: usize = mem::size_of::<DataDescriptor>();

#[inline(always)]
fn send_piece<S, C>(data: &[u8], header_ctor: C)
where
    S: Unsigned,
    C: FnOnce(i32) -> DataDescriptor,
{
    match unsafe { main_buffer.reserve(S::U64, 0) } {
        Ok(buffer) => {
            let to_copy = (S::USIZE - DD).min(data.len());
            let result = unsafe {
                let source = data.as_ptr();
                let destination = buffer.0.as_mut_ptr().offset(DD as isize);
                gen::bpf_probe_read_user(
                    destination as *mut _,
                    to_copy as u32,
                    source as *const _,
                )
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
fn send_data(tag: u32, fd: u32, data: &[u8]) {
    let header_ctor = |size| DataDescriptor { tag, fd, size };
    let length_to_send = data.len() + DD;
    if length_to_send <= Shleft::<typenum::U1, typenum::U8>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U8>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U9>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U9>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U10>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U10>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U11>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U11>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U12>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U12>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U13>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U13>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U14>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U14>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U15>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U15>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U16>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U16>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U17>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U17>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U18>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U18>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U19>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U19>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U20>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U20>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U21>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U21>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U22>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U22>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U23>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U23>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U24>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U24>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U25>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U25>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U26>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U26>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U27>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U27>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U28>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U28>, _>(data, header_ctor)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U29>::USIZE {
        send_piece::<Shleft<typenum::U1, typenum::U29>, _>(data, header_ctor)
    }
}

#[kprobe("__sys_sendto")]
fn kprobe_sendto(regs: Registers) {
    let fd = regs.parm1() as u32;
    let buf = regs.parm2() as *const u8;
    let size = regs.parm3() as usize;
    let _flags = regs.parm4();

    let data = unsafe { slice::from_raw_parts(buf, size) };
    send_data(1, fd, data)
}
