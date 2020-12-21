#![no_std]
#![no_main]
#![cfg(feature = "probes")]

use redbpf_probes::kprobe::prelude::*;
use redbpf_probes::helpers::gen;
use typenum::Unsigned;
use core::{mem, ptr, result::Result, slice, ops::Sub};
use sniffer::DataDescriptor;

program!(0xFFFFFFFE, "GPL");

#[map]
static mut events: PerfMap<u32> = PerfMap::with_max_entries(1);

#[map]
static mut main_buffer: RingBuffer = RingBuffer::with_max_length(0x40000000);

const DD: usize = mem::size_of::<DataDescriptor>();

#[inline(always)]
fn send_piece<S, C>(
    tag: i32,
    data: &[u8],
    header_ctor: C,
) -> Result<(), ()>
where
    // Sub<typenum::U20>, 20 should be equal size_of DataDescriptor
    S: Unsigned + Sub<typenum::U20>,
    <S as Sub<typenum::U20>>::Output: Unsigned,
    C: FnOnce(i32) -> DataDescriptor,
{
    match unsafe { main_buffer.reserve(S::U64, 0) } {
        Ok(buffer) => {
            let result = unsafe {
                let source = data.as_ptr();
                let destination = buffer.0.as_mut_ptr().offset(DD as isize);
                gen::bpf_probe_read_user(
                    destination as *mut _,
                    <<S as Sub<typenum::U20>>::Output as Unsigned>::U32,
                    source as *const _,
                )
            };
            let descriptor = header_ctor(if result < 0 { result as i32 } else { tag });
            unsafe {
                ptr::write(buffer.0[..DD].as_ptr() as *mut _, descriptor);
            }
            buffer.submit(0);
            if result < 0 {
                Err(())
            } else {
                Ok(())
            }
        },
        Err(()) => {
            // failed to allocate buffer, try allocate smaller buffer to report error
            let buffer = unsafe { main_buffer.reserve(DD as u64, 0) }?;
            let descriptor = header_ctor(-90);
            unsafe {
                ptr::write(buffer.0.as_ptr() as *mut _, descriptor);
            }
            buffer.submit(0);
            Err(())
        },
    }
}

macro_rules! send_piece {
    ($size:ty, $tag:expr, $fd:expr, $data:expr, $offset:expr) => {{
        send_piece::<$size, _>($tag, &$data[$offset..], |tag| {
            DataDescriptor {
                tag: tag,
                fd: $fd,
                offset: $offset as u32,
                size: <$size>::U32,
                overall_size: $data.len() as u32,
            }
        })?;
        $offset += <$size as Unsigned>::USIZE - DD;
        if $offset >= $data.len() {
            Err(())?
        }
    }};
}

#[inline(always)]
fn send_data(tag: i32, fd: u32, data: &[u8]) -> Result<(), ()> {
    let mut offset = 0;

    send_piece!(typenum::U256, tag, fd, data, offset);
    send_piece!(typenum::U256, tag, fd, data, offset);
    send_piece!(typenum::U512, tag, fd, data, offset);
    send_piece!(typenum::U1024, tag, fd, data, offset);
    send_piece!(typenum::U2048, tag, fd, data, offset);
    send_piece!(typenum::U4096, tag, fd, data, offset);
    send_piece!(typenum::U8192, tag, fd, data, offset);
    send_piece!(typenum::U16384, tag, fd, data, offset);
    send_piece!(typenum::U32768, tag, fd, data, offset);
    send_piece!(typenum::U65536, tag, fd, data, offset);
    send_piece!(typenum::U131072, tag, fd, data, offset);
    send_piece!(typenum::U262144, tag, fd, data, offset);
    send_piece!(typenum::U524288, tag, fd, data, offset);
    send_piece!(typenum::U1048576, tag, fd, data, offset);
    send_piece!(typenum::U2097152, tag, fd, data, offset);
    send_piece!(typenum::U4194304, tag, fd, data, offset);
    send_piece!(typenum::U8388608, tag, fd, data, offset);
    send_piece!(typenum::U16777216, tag, fd, data, offset);
    send_piece!(typenum::U33554432, tag, fd, data, offset);
    send_piece!(typenum::U67108864, tag, fd, data, offset);
    send_piece!(typenum::U134217728, tag, fd, data, offset);
    send_piece!(typenum::U268435456, tag, fd, data, offset);
    send_piece!(typenum::U536870912, tag, fd, data, offset);

    Ok(())
}

#[kprobe("__sys_sendto")]
fn kprobe_sendto(regs: Registers) {
    let fd = regs.parm1() as u32;
    let buf = regs.parm2() as *const u8;
    let size = regs.parm3() as usize;
    let _flags = regs.parm4();

    let data = unsafe { slice::from_raw_parts(buf, size) };
    match send_data(1, fd, data) {
        Ok(()) => (),
        Err(()) => (),
    }
}
