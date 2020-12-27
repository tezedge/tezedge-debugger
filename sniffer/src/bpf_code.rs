// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub static CODE: &'static [u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/target/bpf/programs/kprobe/kprobe.elf"
));
