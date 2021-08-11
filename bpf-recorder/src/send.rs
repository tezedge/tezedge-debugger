// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::{mem, ptr, ops::Sub};
use typenum::{Unsigned, Bit, Shleft};
use ebpf_kern::{RingBufferRef, helpers};
use bpf_recorder::{EventId, DataDescriptor, DataTag};

#[inline(always)]
pub fn sized<S, K>(id: EventId, tag: DataTag, error: i16, data: *const u8, len: usize, rb: &mut RingBufferRef)
where
    S: Unsigned,
    K: Bit,
{
    if let Ok(mut buffer) = rb.reserve(S::U64 as usize + mem::size_of::<DataDescriptor>()) {
        let p_buffer = buffer.as_mut().as_mut_ptr() as *mut DataDescriptor;

        let to_copy = S::USIZE.min(len);

        let result = if to_copy > 0 {
            unsafe {
                if K::BOOL {
                    helpers::probe_read_kernel(
                        p_buffer.offset(1) as *mut _,
                        to_copy as u32,
                        data as *const _,
                    )
                } else {
                    helpers::probe_read_user(
                        p_buffer.offset(1) as *mut _,
                        to_copy as u32,
                        data as *const _,
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
        let descriptor = DataDescriptor { id, tag, error, size };
        unsafe {
            ptr::write(p_buffer, descriptor);
        }

        buffer.submit();
        return;
    }

    // failed to allocate buffer, try allocate smaller buffer to report error
    if let Ok(mut buffer) = rb.reserve(mem::size_of::<DataDescriptor>()) {
        let descriptor = DataDescriptor { id, tag, error, size: -90 };
        unsafe {
            ptr::write(buffer.as_mut().as_mut_ptr() as *mut _, descriptor);
        }
        buffer.submit();
    }
}

type SizeOfDataDescriptor = typenum::U24;
type DecByDataDescriptor<S> = <S as Sub<SizeOfDataDescriptor>>::Output;

#[inline(always)]
fn sized_inner<S, K>(id: EventId, tag: DataTag, error: i16, data: *const u8, len: usize, rb: &mut RingBufferRef)
where
    S: Unsigned + Sub<SizeOfDataDescriptor>,
    DecByDataDescriptor<S>: Unsigned,
    K: Bit,
{
    sized::<DecByDataDescriptor<S>, K>(id, tag, error, data, len, rb)
}

#[inline(always)]
pub fn dyn_sized<K>(id: EventId, tag: DataTag, error: i16, data: *const u8, len: usize, rb: &mut RingBufferRef)
where
    K: Bit,
{
    // data len 124 happens often, let's have special case 148 = 124 + sizeof DataDescriptor
    let length_to_send = len + mem::size_of::<DataDescriptor>();
    if length_to_send <= typenum::U148::USIZE {
        sized_inner::<typenum::U148, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U8>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U8>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U9>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U9>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U10>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U10>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U11>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U11>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U12>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U12>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U13>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U13>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U14>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U14>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U15>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U15>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U16>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U16>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U17>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U17>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U18>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U18>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U19>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U19>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U20>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U20>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U21>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U21>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U22>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U22>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U23>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U23>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U24>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U24>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U25>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U25>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U26>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U26>, K>(id, tag, error, data, len, rb)
    } else if length_to_send <= Shleft::<typenum::U1, typenum::U27>::USIZE {
        sized_inner::<Shleft<typenum::U1, typenum::U27>, K>(id, tag, error, data, len, rb)
    }
}
