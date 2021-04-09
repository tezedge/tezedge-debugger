// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![no_std]
#![no_main]
#![cfg(feature = "probes")]

use redbpf_probes::kprobe::prelude::*;

program!(0xFFFFFFFE, "Dual BSD/GPL");

#[map]
static mut main_buffer: RingBuffer = RingBuffer::with_max_length(0x40000000); // 1GiB buffer

#[kprobe("kprobe/do_brk_flags")]
fn kprobe_brk(regs: Registers) {
    let addr = regs.parm1();
    match unsafe { &mut main_buffer }.reserve(8, 0) {
        Ok(mut data) => data.as_mut().copy_from_slice(&addr.to_be_bytes()),
        Err(()) => (),
    }
}

#[kprobe("kprobe/ksys_mmap_pgoff")]
fn kprobe_mmap(regs: Registers) {
    let _ = regs;
}

#[kprobe("kprobe/__vm_munmap")]
fn kprobe_munmap(regs: Registers) {
    let _ = regs;
}
