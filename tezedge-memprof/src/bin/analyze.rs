// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs::File, io::Write};
    use bpf_memprof::{EventKind, Hex64};

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let f = File::open("target/report.bin")?;
    let history = bincode::deserialize_from::<_, Vec<EventKind>>(f)?;

    let mut f = File::create("target/cache.out")?;
    let mut g = File::create("target/cache_free.out")?;
    for event in history {
        match event {
            EventKind::CacheAlloc(v) => {
                let r = v.ptr..Hex64(v.ptr.0 + v.bytes_alloc.0);
                f.write_fmt(format_args!("{:?}\n", r))?;
            },
            EventKind::CacheAllocNode(v) => {
                let r = v.ptr..Hex64(v.ptr.0 + v.bytes_alloc.0);
                f.write_fmt(format_args!("{:?}\n", r))?;
            },
            EventKind::CacheFree(v) => {
                g.write_fmt(format_args!("{:?}\n", v.ptr))?;
            },
            _ => (),
        }
    }

    Ok(())
}
