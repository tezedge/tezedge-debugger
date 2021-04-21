// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{thread, time::Duration, collections::HashMap};
    use bpf_memprof::{Client, Event, EventKind};
    use tezedge_memprof::AtomicState;

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let state = AtomicState::new();
    {
        let state = state.clone();
        ctrlc::set_handler(move || state.stop())?;
    }

    {
        let state = state.clone();
        thread::spawn(move || {
            let delay = Duration::from_secs(1);
            while state.running() {
                thread::sleep(delay);
                log::info!("{}", state.observe(delay));
            }
        });
    }

    //let mut history = Vec::new();
    let mut alloc = HashMap::new();

    let (mut client, mut rb) = Client::new("/tmp/bpf-memprof.sock")?;
    client.send_command("dummy command")?;
    while state.running() {
        let events = rb.read_blocking::<Event>(&state.running)?;
        for event in events {
            match &event.event {
                &EventKind::KFree(ref v) => {
                    match alloc.remove(&v.ptr.0) {
                        Some(len) => {
                            state.count_physical_free(len);
                            state.count_free_event(true);
                        },
                        None => state.count_free_event(false),
                    }
                },
                &EventKind::KMAlloc(ref v) => {
                    alloc.insert(v.ptr.0, v.bytes_alloc.0);
                    state.count_physical_alloc(v.bytes_alloc.0);
                }
                &EventKind::KMAllocNode(ref v) => {
                    alloc.insert(v.ptr.0, v.bytes_alloc.0);
                    state.count_physical_alloc(v.bytes_alloc.0);
                }
                &EventKind::CacheAlloc(ref v) => {
                    alloc.insert(v.ptr.0, v.bytes_alloc.0);
                    state.count_physical_alloc(v.bytes_alloc.0);
                },
                &EventKind::CacheAllocNode(ref v) => {
                    alloc.insert(v.ptr.0, v.bytes_alloc.0);
                    state.count_physical_alloc(v.bytes_alloc.0);
                },
                &EventKind::CacheFree(ref v) => {
                    match alloc.remove(&v.ptr.0) {
                        Some(len) => {
                            state.count_physical_free(len);
                            state.count_free_event(true);
                        },
                        None => state.count_free_event(false),
                    }
                },
                &EventKind::PageAlloc(ref v) => {
                    state.count_physical_alloc(0x1000 << (v.order as u64));
                },
                &EventKind::PageAllocExtFrag(ref v) => {
                    state.count_physical_alloc(0x1000 << (v.alloc_order as u64));
                },
                &EventKind::PageAllocZoneLocked(ref v) => {
                    state.count_physical_alloc(0x1000 << (v.order as u64));
                },
                &EventKind::PageFree(ref v) => {
                    state.count_physical_free(0x1000 << (v.order as u64));
                },
                &EventKind::PageFreeBatched(_) => {
                    state.count_physical_free(0x1000);
                },
                &EventKind::PagePcpuDrain(ref v) => {
                    state.count_physical_free(0x1000 << (v.order as u64));
                },
                &EventKind::PageFaultUser(_) => {
                    state.count_page_fault();
                }
            }
            //history.push(event);
        }
    }

    //serde_json::to_writer(std::fs::File::create("target/report.json")?, &history)?;

    Ok(())
}
