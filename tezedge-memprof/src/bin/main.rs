// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::HashMap, sync::{Arc, Mutex, atomic::{Ordering, AtomicU32}}};
use bpf_memprof::{Client, ClientCallback, Event, EventKind};
use tezedge_memprof::{AtomicState, Page, History, EventLast};

#[derive(Default)]
struct MemprofClient {
    pid: Arc<AtomicU32>,
    state: Arc<AtomicState>,
    allocations: HashMap<u64, u64>,
    history: Arc<Mutex<History<EventLast>>>,
    last: Option<EventKind>,
}

impl ClientCallback for MemprofClient {
    fn arrive(&mut self, client: &mut Client, data: &[u8]) {
        let _ = client;
        let event = match Event::from_slice(data) {
            Ok(v) => v,
            Err(error) => {
                log::error!("failed to read ring buffer slice: {}", error);
                return;
            }
        };

        self.pid.store(event.pid, Ordering::Relaxed);

        if let Some(last) = &self.last {
            if last.eq(&event.event) {
                log::debug!("repeat");
                return;
            }
        }
        self.state.process_event(&mut self.allocations, &event.event);
        match &event.event {
            &EventKind::PageAlloc(ref v) if v.pfn.0 != 0 =>
                self.history.lock().unwrap().track_alloc(Page::new(v.pfn, v.order), &event.stack, v.gfp_flags),
            &EventKind::PageFree(ref v) if v.pfn.0 != 0 =>
                self.history.lock().unwrap().track_free(Page::new(v.pfn, v.order)),
            _ => (),
        }
        self.last = Some(event.event);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{thread, time::Duration, io};
    use tezedge_memprof::{Reporter, StackResolver};
    use tokio::runtime::Runtime;

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let cli = MemprofClient::default();

    {
        let state = cli.state.clone();
        ctrlc::set_handler(move || state.stop())?;
    }

    let thread = {
        let mut state = Reporter::new(cli.state.clone());
        let history = cli.history.clone();
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
                log::info!("{}", history.lock().unwrap().short_report());
            }
        })
    };

    let resolver = StackResolver::spawn(cli.pid.clone());

    let runtime = Runtime::new().unwrap();
    let server = runtime.spawn(warp::serve(server::routes(cli.history.clone(), resolver)).run(([0, 0, 0, 0], 17832)));

    let state = cli.state.clone();
    let history = cli.history.clone();
    let rb = Client::connect("/tmp/bpf-memprof.sock", cli)?;
    while state.running() {
        match rb.poll(Duration::from_secs(1)) {
            Ok(_) => (),
            Err(c) if c == -4 => break,
            Err(c) => {
                log::error!("code: {}, error: {}", c, io::Error::last_os_error());
                break;
            }
        }
    }

    thread.join().unwrap();

    let history = history.lock().unwrap();
    serde_json::to_writer(std::fs::File::create("target/history.json")?, &*history)?;

    let _ = (server, runtime);

    Ok(())
}

mod server {
    use std::sync::{Arc, Mutex, RwLock};
    use warp::{
        Filter, Rejection, Reply,
        reply::{WithStatus, Json, self},
        http::StatusCode,
    };
    use serde::Deserialize;
    use tezedge_memprof::{History, EventLast, StackResolver};

    pub fn routes(
        history: Arc<Mutex<History<EventLast>>>,
        resolver: Arc<RwLock<StackResolver>>,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone + Sync + Send + 'static {
        use warp::reply::with;
    
        warp::get()
            .and(tree(history, resolver))
            .with(with::header("Content-Type", "application/json"))
            .with(with::header("Access-Control-Allow-Origin", "*"))
    }

    fn tree(
        history: Arc<Mutex<History<EventLast>>>,
        resolver: Arc<RwLock<StackResolver>>,
    ) -> impl Filter<Extract = (WithStatus<Json>,), Error = Rejection> + Clone + Sync + Send + 'static {
        #[derive(Deserialize)]
        struct Params {
            threshold: Option<u64>,
        }

        warp::path!("v1" / "tree")
            .and(warp::query::query())
            .map(move |params: Params| -> WithStatus<Json> {
                let resolver = resolver.read().unwrap();
                let report = history.lock()
                    .unwrap()
                    .tree_report(resolver, params.threshold.unwrap_or(512));
                reply::with_status(reply::json(&report), StatusCode::OK)
            })
    }
}
