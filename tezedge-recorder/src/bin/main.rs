// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#[cfg(all(not(target_env = "msvc"), feature = "jemallocator"))]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn main() -> anyhow::Result<()> {
    use std::sync::{
        Arc,
        atomic::{Ordering, AtomicBool},
    };
    use tezedge_recorder::{System, database::rocks, main_loop};

    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()?;

    let running = Arc::new(AtomicBool::new(true));
    {
        let running = running.clone();
        ctrlc::set_handler(move || running.store(false, Ordering::Relaxed))?;
    }

    let mut system = System::<rocks::Db>::load_config()?;
    system.run_dbs(running.clone());
    main_loop::run(system, running)
}
