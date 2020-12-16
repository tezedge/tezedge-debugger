#![no_std]
#![no_main]
#![cfg(feature = "probes")]

use redbpf_probes::kprobe::prelude::*;
use sniffer::SnifferItem;

program!(0xFFFFFFFE, "GPL");

#[map]
static mut events: PerfMap<SnifferItem> = PerfMap::with_max_entries(0x100);

#[map]
static mut sockets_ipv4: HashMap<u32, [u8; 4]> = HashMap::with_max_entries(0x400);

#[map]
static mut sockets_ipv6: HashMap<u32, [u8; 16]> = HashMap::with_max_entries(0x400);

#[kprobe("__sys_sendto")]
fn kprobe_sendto(regs: Registers) {
    let fd = regs.parm1() as u32;
    let buf = regs.parm2() as usize;
    let len0 = regs.parm3() as usize;
    let _flags = regs.parm4();

    if buf == 0 {
        return;
    }

    let mut item = SnifferItem {
        tag: 1,
        fd: fd,
        offset: 0,
        size: len0 as u32,
        data: [0; SnifferItem::SIZE],
    };
    let p_data = item.data.as_mut_ptr() as *mut _;

    for i in 0..0xc00 {
        let offset = item.offset as usize;
        let len = usize::min(len0 - offset, SnifferItem::SIZE) as u32;

        let p_buf = (buf + offset) as *const _;
        let result = unsafe { bpf_probe_read_user(p_data, len, p_buf) };
        if result < 0 {
            item.tag = result;
        }
        unsafe { events.insert(regs.ctx, &item) };

        item.offset += len;
        if item.offset == len0 as u32 {
            return;
        }
    }

    if item.offset < len0 as u32 {
        item.tag = -90; // EMSGSIZE 90 Message too long
        unsafe { events.insert(regs.ctx, &item) };
    }
}

#[inline(always)]
pub unsafe extern "C" fn bpf_probe_read_user(dst: *mut cty::c_void, size: u32, src: *const cty::c_void) -> cty::c_int {
    let f: unsafe extern "C" fn (dst: *mut cty::c_void, size: u32, src: *const cty::c_void) -> cty::c_int = core::mem::transmute(112usize);
    f(dst, size, src)
}

#[inline(always)]
pub unsafe extern "C" fn bpf_probe_read_kernel(dst: *mut cty::c_void, size: u32, src: *const cty::c_void) -> cty::c_int {
    let f: unsafe extern "C" fn (dst: *mut cty::c_void, size: u32, src: *const cty::c_void) -> cty::c_int = core::mem::transmute(113usize);
    f(dst, size, src)
}
