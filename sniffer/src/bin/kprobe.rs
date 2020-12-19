#![no_std]
#![no_main]
#![cfg(feature = "probes")]

use redbpf_probes::kprobe::prelude::*;
use redbpf_probes::helpers::gen;
use core::{mem, ptr, result::Result, slice};
use sniffer::DataDescriptor;

program!(0xFFFFFFFE, "GPL");

#[map]
static mut events: PerfMap<u32> = PerfMap::with_max_entries(1);

#[map]
static mut main_buffer: RingBuffer = RingBuffer::with_max_length(0x80000000);

const DD: usize = mem::size_of::<DataDescriptor>();

#[inline(always)]
fn send_piece(
    tag: i32,
    fd: u32,
    data: &[u8],
    size: usize,
    offset: usize,
    overall_size: usize,
) -> Result<(), ()> {
    match unsafe { main_buffer.reserve(size as u64, 0) } {
        Ok(buffer) => {
            let result = unsafe {
                let source = data.as_ptr();
                let destination = buffer.0.as_mut_ptr().offset(DD as isize);
                gen::bpf_probe_read_user(
                    destination as *mut _,
                    (size - DD) as u32,
                    source as *const _,
                )
            };
            let descriptor = DataDescriptor {
                tag: if result < 0 { result as i32 } else { tag },
                fd: fd,
                offset: offset as u32,
                size: size as u32,
                overall_size: overall_size as u32,
            };
            unsafe {
                ptr::write_unaligned(buffer.0[..DD].as_ptr() as *mut _, descriptor);
            }
            buffer.submit(0);
            if result < 0 {
                return Err(());
            }
            Ok(())
        },
        Err(()) => {
            // failed to allocate buffer, try allocate smaller buffer to report error
            match unsafe { main_buffer.reserve(DD as u64, 0) } {
                Ok(buffer) => {
                    let descriptor = DataDescriptor {
                        tag: -90,
                        fd: fd,
                        offset: offset as u32,
                        size: size as u32,
                        overall_size: overall_size as u32,
                    };
                    unsafe {
                        ptr::write_unaligned(buffer.0.as_ptr() as *mut _, descriptor);
                    }
                    buffer.submit(0);
                },
                Err(()) => {
                    // failed to allocate buffer to report error, nothing to do
                },
            }
            Err(())
        },
    }
}

macro_rules! send_piece {
    ($size:expr, $tag:expr, $fd:expr, $data:expr, $offset:expr) => {{
        send_piece($tag, $fd, &$data[$offset..], $size, $offset, $data.len())?;
        $offset += $size - DD;
        if $offset >= $data.len() {
            Err(())?
        }
    }};
}

#[inline(always)]
fn send_data(tag: i32, fd: u32, data: &[u8]) -> Result<(), ()> {
    let mut offset = 0;

    send_piece!(0x0000_0100, tag, fd, data, offset);
    send_piece!(0x0000_0100, tag, fd, data, offset);
    send_piece!(0x0000_0200, tag, fd, data, offset);
    send_piece!(0x0000_0400, tag, fd, data, offset);
    send_piece!(0x0000_0800, tag, fd, data, offset);
    send_piece!(0x0000_1000, tag, fd, data, offset);
    send_piece!(0x0000_2000, tag, fd, data, offset);
    send_piece!(0x0000_4000, tag, fd, data, offset);
    send_piece!(0x0000_8000, tag, fd, data, offset);
    send_piece!(0x0001_0000, tag, fd, data, offset);
    send_piece!(0x0002_0000, tag, fd, data, offset);
    send_piece!(0x0004_0000, tag, fd, data, offset);
    send_piece!(0x0008_0000, tag, fd, data, offset);
    send_piece!(0x0010_0000, tag, fd, data, offset);
    send_piece!(0x0020_0000, tag, fd, data, offset);
    send_piece!(0x0040_0000, tag, fd, data, offset);
    send_piece!(0x0080_0000, tag, fd, data, offset);
    send_piece!(0x0100_0000, tag, fd, data, offset);
    send_piece!(0x0200_0000, tag, fd, data, offset);
    send_piece!(0x0400_0000, tag, fd, data, offset);
    send_piece!(0x0800_0000, tag, fd, data, offset);
    send_piece!(0x1000_0000, tag, fd, data, offset);
    send_piece!(0x2000_0000, tag, fd, data, offset);

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
