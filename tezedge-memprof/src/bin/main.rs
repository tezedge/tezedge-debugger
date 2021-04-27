// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{thread, time::Duration, collections::HashMap};
    use bpf_memprof::{Client, Event, EventKind};
    use tezedge_memprof::{AtomicState, Reporter};

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let state = AtomicState::new();
    {
        let state = state.clone();
        ctrlc::set_handler(move || state.stop())?;
    }

    {
        let mut state = Reporter::new(state.clone());
        thread::spawn(move || {
            let delay = Duration::from_secs(4);
            while state.running() {
                thread::sleep(delay);
                log::info!("{}", state.report(delay));
            }
        });
    }

    let mut history = Vec::new();
    let mut alloc = HashMap::new();

    let (mut client, mut rb) = Client::new("/tmp/bpf-memprof.sock")?;
    client.send_command("dummy command")?;
    while state.running() {
        let events = rb.read_blocking::<Event>(state.running_ref())?;
        for event in events {
            match &event.event {
                &EventKind::KFree(ref v) => {
                    match alloc.get(&v.ptr.0) {
                        Some(&len) => state.slab_unknown_free(len, true),
                        None => state.slab_unknown_free(0, false),
                    }
                },
                &EventKind::KMAlloc(ref v) => {
                    alloc.insert(v.ptr.0, v.bytes_alloc.0);
                    state.slab_unknown_alloc(v.bytes_alloc.0);
                },
                &EventKind::KMAllocNode(ref v) => {
                    alloc.insert(v.ptr.0, v.bytes_alloc.0);
                    state.slab_unknown_alloc(v.bytes_alloc.0);
                },
                &EventKind::CacheAlloc(ref v) => {
                    alloc.insert(v.ptr.0, v.bytes_alloc.0);
                    state.slab_known_alloc(v.bytes_alloc.0);
                },
                &EventKind::CacheAllocNode(ref v) => {
                    alloc.insert(v.ptr.0, v.bytes_alloc.0);
                    state.slab_known_alloc(v.bytes_alloc.0);
                },
                &EventKind::CacheFree(ref v) => {
                    match alloc.get(&v.ptr.0) {
                        Some(&len) => state.slab_known_free(len, true),
                        None => state.slab_known_free(0, false),
                    }
                },
                &EventKind::PageAlloc(ref v) => {
                    state.page_alloc(0x1000 << (v.order as u64));
                },
                &EventKind::PageAllocExtFrag(ref v) => {
                    state.page_alloc(0x1000 << (v.alloc_order as u64));
                },
                &EventKind::PageAllocZoneLocked(ref v) => {
                    state.page_alloc(0x1000 << (v.order as u64));
                },
                &EventKind::PageFree(ref v) => {
                    state.page_free(0x1000 << (v.order as u64));
                },
                &EventKind::PageFreeBatched(_) => {
                    state.page_free(0x1000);
                },
                &EventKind::PagePcpuDrain(ref v) => {
                    state.page_free(0x1000 << (v.order as u64));
                },
                &EventKind::PageFaultUser(_) => {
                    state.page_fault();
                },
                &EventKind::RssStat(ref v) => {
                    state.rss_stat(v.size, v.member);
                    history.push(event.event);
                },
            }
            //log::info!("{:?}", event.stack);
            //history.push(event.event);
        }
    }

    serde_json::to_writer(std::fs::File::create("target/report.json")?, &history)?;
    //bincode::serialize_into(std::fs::File::create("target/report.bin")?, &history)?;

    Ok(())
}
