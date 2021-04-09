// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT


fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::{Arc, atomic::{Ordering, AtomicBool}};
    use bpf_memprof::{Client, Event};

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let running = Arc::new(AtomicBool::new(true));
    {
        let running = running.clone();
        ctrlc::set_handler(move || running.store(false, Ordering::Relaxed))?;
    }

    let (mut client, mut rb) = Client::new("/tmp/bpf-memprof.sock")?;
    client.send_command("dummy command")?;

    while running.load(Ordering::Relaxed) {
        let events = rb.read_blocking::<Event>(&running)?;
        for event in events {
            log::info!("{:?}", event);
        }
    }

    Ok(())
}
