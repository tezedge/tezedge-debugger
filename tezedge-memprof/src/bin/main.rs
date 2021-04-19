// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{
        sync::{Arc, atomic::{Ordering, AtomicBool, AtomicU64}},
        fs::File,
        io::Write,
        thread,
        time::Duration,
    };
    use serde::{Serialize, ser};
    use bpf_memprof::{Client, Event, EventKind};
    use tezedge_memprof::ProcessMap;

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    #[derive(Serialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum StackEntry {
        Unknown,
        Symbol {
            filename: String,
            address: Address,
        },
    }

    #[derive(Serialize)]
    struct Record {
        event: EventKind,
        stack: Vec<StackEntry>,
    }

    struct Address(usize);

    impl Serialize for Address {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: ser::Serializer,
        {
            serializer.serialize_str(&format!("{:016x}", self.0))
        }
    }

    let running = Arc::new(AtomicBool::new(true));
    {
        let running = running.clone();
        ctrlc::set_handler(move || running.store(false, Ordering::Relaxed))?;
    }

    let (mut client, mut rb) = Client::new("/tmp/bpf-memprof.sock")?;
    client.send_command("dummy command")?;

    let allocated = Arc::new(AtomicU64::new(0));
    let allocated_phys = Arc::new(AtomicU64::new(0));
    let maps_size = Arc::new(AtomicU64::new(0));
    {
        let running = running.clone();
        let allocated = allocated.clone();
        let allocated_phys = allocated_phys.clone();
        let maps_size = maps_size.clone();
        thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_secs(5));
                let bytes = allocated.load(Ordering::Relaxed);
                let bytes_phys = allocated_phys.load(Ordering::Relaxed);
                let bytes_maps = maps_size.load(Ordering::Relaxed);
                log::info!("allocated: {} {}, phys: {}", bytes, bytes_maps, bytes_phys);
            }
        });
    }

    //let mut cached_map = None;
    let mut history = vec![];
    let mut last_data_addr = 0;
    while running.load(Ordering::Relaxed) {
        let events = rb.read_blocking::<Event>(&running)?;
        for event in events {
            let map = match ProcessMap::new(event.pid) {
                Ok(map) => map,
                Err(_) => break,
            };
            maps_size.store(map.size() as u64, Ordering::Relaxed);

            let mut record = Record {
                event: event.kind,
                stack: vec![],
            };

            match &record.event {
                &EventKind::Brk { addr } => {
                    if last_data_addr != 0 {
                        debug_assert!(addr > last_data_addr);
                        allocated.fetch_add(addr - last_data_addr, Ordering::Relaxed);
                    }
                    last_data_addr = addr;
                },
                &EventKind::MMap { len, .. } => {
                    allocated.fetch_add(len, Ordering::Relaxed);
                },
                &EventKind::MUnmap { len, .. } => {
                    allocated.fetch_sub(len, Ordering::Relaxed);
                },
                &EventKind::PageAlloc { order } => {
                    allocated_phys.fetch_add(4096 << order, Ordering::Relaxed);
                },
            }

            match event.stack {
                Ok(stack) => {
                    for ip in stack.ips() {
                        match map.find(*ip) {
                            None => {
                                record.stack.push(StackEntry::Unknown);
                            },
                            Some((path, addr)) => {
                                let entry = StackEntry::Symbol {
                                    filename: format!("{:?}", path),
                                    address: Address(addr),
                                };
                                record.stack.push(entry);
                            },
                        }
                    }
                },
                Err(code) => {
                    log::error!("failed to receive stack, error code: {}", code);
                },
            }

            history.push(record);
            if history.len() & 0xfff == 0 {
                log::info!("processed: {} events", history.len());
            }
        }
    }

    let history = serde_json::to_string(&history)?;
    File::create("target/report.json")?.write_all(history.as_bytes())?;

    Ok(())
}
