// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![no_std]
#![no_main]
#![cfg(feature = "probes")]

use redbpf_probes::kprobe::prelude::*;
use redbpf_probes::helpers::{self, gen};
use bpf_memprof::{Event, EventKind};

program!(0xFFFFFFFE, "Dual BSD/GPL");

#[map]
static mut main_buffer: RingBuffer = RingBuffer::with_max_length(0x40000000); // 1GiB buffer

/*#[no_mangle]
#[link_section = "tracepoint/syscalls/sys_enter_execve"]
fn exec(regs: Registers) {
    let filename_ptr = regs.parm2() as *const usize;
    let mut filename = [0; 40];
    filename[0] = 4;
    unsafe {
        gen::bpf_probe_read_kernel(
            filename.as_mut_ptr().offset(4) as *mut _,
            36,
            filename_ptr as *const _,
        );
    }

    match unsafe { &mut main_buffer }.output(&filename, 0) {
        Ok(()) => (),
        Err(_c) => (),
    }
}*/

// TODO: rewrite it
fn check_name() -> bool {
    let comm = helpers::bpf_get_current_comm();

    true
    && comm[0] == 'l' as i8
    && comm[1] == 'i' as i8
    && comm[2] == 'g' as i8
    && comm[3] == 'h' as i8
    && comm[4] == 't' as i8
    && comm[5] == '-' as i8
    && comm[6] == 'n' as i8
    && comm[7] == 'o' as i8
    && comm[8] == 'd' as i8
    && comm[9] == 'e' as i8
}

fn try_send_event(regs: &Registers, event: EventKind) {
    match unsafe { &mut main_buffer }.reserve(0x428, 0) {
        Ok(mut data) => {
            data.as_mut()[..0x20].clone_from_slice(&event.to_bytes());

            let (pid, thread_id) = {
                let x = helpers::bpf_get_current_pid_tgid();
                ((x >> 32) as u32, (x & 0xffffffff) as u32)
            };
            data.as_mut()[0x20..0x28].clone_from_slice(&(pid as u64).to_ne_bytes());

            let ips = &mut data.as_mut()[0x30..];
            let r = unsafe {
                gen::bpf_get_stack(regs.ctx as _, ips.as_ptr() as _, ips.len() as _, 256)
            };
            data.as_mut()[0x28..0x30].clone_from_slice(&r.to_ne_bytes());
            data.submit(0);
        },
        Err(()) => (),
    }
}

#[kprobe("do_brk_flags")]
fn kprobe_brk(regs: Registers) {
    if !check_name() {
        return;
    }

    let addr = regs.parm1();
    try_send_event(&regs, EventKind::Brk { addr });
}

#[kprobe("ksys_mmap_pgoff")]
fn kprobe_mmap(regs: Registers) {
    if !check_name() {
        return;
    }

    let addr = regs.parm1();
    let len = regs.parm2();
    try_send_event(&regs, EventKind::MMap { addr, len });
}

#[kprobe("__vm_munmap")]
fn kprobe_munmap(regs: Registers) {
    if !check_name() {
        return;
    }

    let addr = regs.parm1();
    let len = regs.parm2();
    try_send_event(&regs, EventKind::MUnmap { addr, len });
}
