// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::{mem, ptr};
use typenum::{Unsigned, Bit, Shleft};
use redbpf_probes::{maps::RingBuffer, helpers::gen};
use super::data_descriptor::{EventId, DataDescriptor, DataTag};

#[inline(always)]
pub fn sized<S, K>(id: EventId, tag: DataTag, data: &[u8], rb: &mut RingBuffer)
where
    S: Unsigned,
    K: Bit,
{
    if let Ok(buffer) = rb.reserve(S::U64, 0) {
        let to_copy = (S::USIZE - mem::size_of::<DataDescriptor>()).min(data.len());
        let result = if to_copy > 0 {
            let source = data.as_ptr();
            let offset = mem::size_of::<DataDescriptor>() as isize;
            unsafe {
                let destination = buffer.0.as_mut_ptr().offset(offset);
                if K::BOOL {
                    gen::bpf_probe_read_kernel(
                        destination as *mut _,
                        to_copy as u32,
                        source as *const _,
                    )
                } else {
                    gen::bpf_probe_read_user(
                        destination as *mut _,
                        to_copy as u32,
                        source as *const _,
                    )
                }
            }
        } else {
            0
        };

        let size = if result == 0 {
            to_copy as i32
        } else {
            result as i32
        };
        let descriptor = DataDescriptor { id, tag, size };
        unsafe {
            ptr::write(buffer.0.as_ptr() as *mut _, descriptor);
        }
        buffer.submit(0);
        return;
    }

    // failed to allocate buffer, try allocate smaller buffer to report error
    if let Ok(buffer) = rb.reserve(mem::size_of::<DataDescriptor>() as u64, 0) {
        let descriptor = DataDescriptor { id, tag, size: -90 };
        unsafe {
            ptr::write(buffer.0.as_ptr() as *mut _, descriptor);
        }
        buffer.submit(0);
    }
}

#[inline(always)]
pub fn dyn_sized<K>(id: EventId, tag: DataTag, data: &[u8], rb: &mut RingBuffer)
where
    K: Bit,
{
    let length_to_send = data.len() + mem::size_of::<DataDescriptor>();
    if length_to_send <= Shleft::<typenum::U1, typenum::U8>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U8>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U9>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U9>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U10>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U10>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U11>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U11>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U12>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U12>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U13>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U13>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U14>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U14>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U15>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U15>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U16>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U16>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U17>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U17>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U18>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U18>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U19>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U19>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U20>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U20>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U21>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U21>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U22>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U22>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U23>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U23>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U24>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U24>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U25>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U25>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U26>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U26>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U27>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U27>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U28>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U28>, K>(id, tag, data, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U29>::USIZE {
        sized::<Shleft<typenum::U1, typenum::U29>, K>(id, tag, data, rb)
    }
}
