// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#[repr(C)]
struct AlignedTo<A, B>
where
    B: ?Sized,
{
    _align: [A; 0],
    bytes: B,
}

pub static CODE: &'static [u8] = {
    static _ALIGNED: &'static AlignedTo<u64, [u8]> = &AlignedTo {
        _align: [],
        bytes: *include_bytes!(concat!(env!("OUT_DIR"), "/target/bpf/programs/kprobe/kprobe.elf")),
    };
    &_ALIGNED.bytes
};
