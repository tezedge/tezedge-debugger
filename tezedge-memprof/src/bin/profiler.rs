// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    sync::{Arc, Mutex, atomic::{Ordering, AtomicBool, AtomicU32}},
    io,
    time::Duration,
    thread,
    env,
    process::Command,
};
use bpf_memprof::{EventKind, Event, ClientCallback, Client};
use tezedge_memprof::{Collector, StackResolver, server};

#[derive(Default)]
struct MemprofClient {
    pid_local: u32,
    pid: Arc<AtomicU32>,
    collector: Arc<Mutex<Collector>>,
    last: Option<EventKind>,
    overall_counter: u64,
}

impl ClientCallback for MemprofClient {
    fn arrive(&mut self, client: &mut Client, data: &[u8]) {
        let event = match Event::from_slice(data) {
            Ok(v) => v,
            Err(error) => {
                log::error!("failed to read ring buffer slice: {}", error);
                return;
            }
        };

        if let Some(last) = &self.last {
            if last.eq(&event.event) {
                log::trace!("repeat");
                return;
            }
        }
        match &event.event {
            &EventKind::PageAlloc(ref v) if v.pfn.0 != 0 => {
                self.pid_local = event.pid;
                self.pid.store(event.pid, Ordering::SeqCst);
                self.collector.lock().unwrap().track_alloc(v.pfn.0 as u32, v.order as u8, &event.stack);
            }
            &EventKind::PageFree(ref v) if v.pfn.0 != 0 && self.pid_local != 0 => {
                self.collector.lock().unwrap().track_free(v.pfn.0 as u32);
            },
            &EventKind::AddToPageCache(ref v) if v.pfn.0 != 0 && self.pid_local != 0 => {
                self.collector.lock().unwrap().mark_cache(v.pfn.0 as u32, true);
            },
            &EventKind::RemoveFromPageCache(ref v) if v.pfn.0 != 0 && self.pid_local != 0 => {
                self.collector.lock().unwrap().mark_cache(v.pfn.0 as u32, false);
            },
            _ => (),
        }
        self.last = Some(event.event);

        self.overall_counter += 1;
        if self.overall_counter & 0xffff == 0 {
            client.send_command("check").unwrap();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let bpf = if env::args().find(|a| a == "--run-bpf").is_some() {
        let h = Command::new("bpf-memprof-user").spawn().expect("cannot run bpf");
        thread::sleep(Duration::from_millis(500));
        Some(h)
    } else {
        None
    };

    let running = Arc::new(AtomicBool::new(true));
    let cli = MemprofClient::default();
    let history = cli.collector.clone();

    // spawn a thread monitoring process map from `/proc/<pid>/maps` and loading symbol tables
    let resolver = StackResolver::spawn(cli.pid.clone());

    // spawn a thread-pool serving http requests, using tokio
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = runtime
        .spawn(warp::serve(server::routes(history.clone(), resolver, cli.pid.clone())).run(([0, 0, 0, 0], 17832)));

    // spawn a thread listening ctrl+c
    {
        let running = running.clone();
        ctrlc::set_handler(move || running.store(false, Ordering::Relaxed))?;
    }

    // polling ebpf events
    let rb = Client::connect("/tmp/bpf-memprof.sock", cli)?;
    while running.load(Ordering::Relaxed) {
        match rb.poll(Duration::from_secs(1)) {
            Ok(_) => (),
            Err(c) if c == -4 => break,
            Err(c) => {
                log::error!("code: {}, error: {}", c, io::Error::last_os_error());
                break;
            }
        }
    }

    let _ = server;
    if let Some(mut bpf) = bpf {
        let _ = bpf.wait()?;
    }

    Ok(())
}
