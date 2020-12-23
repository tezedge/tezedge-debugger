pub static CODE: &'static [u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/target/bpf/programs/kprobe/kprobe.elf"
));
