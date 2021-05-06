// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{thread, time::Duration, collections::HashMap, sync::{Arc, Mutex}, fs::File};
    use bpf_memprof::{Client, Event, EventKind};
    use tezedge_memprof::{AtomicState, Reporter, Page, PageHistory};

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let state = AtomicState::new();
    {
        let state = state.clone();
        ctrlc::set_handler(move || state.stop())?;
    }

    let thread = {
        let mut state = Reporter::new(state.clone());
        thread::spawn(move || {
            let delay = Duration::from_secs(4);
            let mut cnt = 2;
            loop {
                if !state.running() {
                    if cnt <= 0 {
                        break;
                    } else {
                        cnt -= 1;
                    }
                }
                thread::sleep(delay);
                log::info!("{}", state.report(delay));
            }
        })
    };

    let history = Arc::new(Mutex::new(PageHistory::default()));
    let mut allocations = HashMap::new();
    let mut last = None::<EventKind>;

    let (mut client, mut rb) = Client::new("/tmp/bpf-memprof.sock")?;
    client.send_command("dummy command")?;
    while state.running() {
        let events = rb.read_blocking::<Event>(state.running_ref())?;
        for event in events {
            if let Some(last) = &last {
                if last.eq(&event.event) {
                    log::debug!("repeat");
                    continue;
                }
            }
            state.process_event(&mut allocations, &event.event);
            match &event.event {
                &EventKind::PageAlloc(ref v) if v.pfn.0 != 0 =>
                    history.lock().unwrap().process(Page::new(v.pfn, v.order), Some(&event.stack)),
                &EventKind::PageFree(ref v) if v.pfn.0 != 0 =>
                    history.lock().unwrap().process(Page::new(v.pfn, v.order), None),
                _ => (),
            }
            last = Some(event.event);
        }
    }

    thread.join().unwrap();

    let history = history.lock().unwrap();
    serde_json::to_writer(File::create("target/history.json")?, &*history)?;
    serde_json::to_writer(File::create("target/tree.json")?, &history.report(&|_| true))?;

    Ok(())
}
