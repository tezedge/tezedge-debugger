// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{thread, time::Duration, collections::HashMap, sync::{Arc, Mutex}};
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

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = runtime.spawn(warp::serve(server::routes(history.clone())).run(([0, 0, 0, 0], 17832)));

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

    //let history = history.lock().unwrap();
    //serde_json::to_writer(std::fs::File::create("target/history.json")?, &*history)?;

    let _ = (server, runtime);

    Ok(())
}

mod server {
    use std::sync::{Arc, Mutex};
    use warp::{
        Filter, Rejection, Reply,
        reply::{WithStatus, Json, self},
        http::StatusCode,
    };
    use tezedge_memprof::PageHistory;

    pub fn routes(
        history: Arc<Mutex<PageHistory>>,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone + Sync + Send + 'static {
        use warp::reply::with;
    
        warp::get()
            .and(
                tree(history)
            )
            .with(with::header("Content-Type", "application/json"))
            .with(with::header("Access-Control-Allow-Origin", "*"))
    }

    fn tree(
        history: Arc<Mutex<PageHistory>>,
    ) -> impl Filter<Extract = (WithStatus<Json>,), Error = Rejection> + Clone + Sync + Send + 'static {
        warp::path!("v1" / "tree")
            .and(warp::query::query())
            .map(move |()| -> WithStatus<Json> {
                let report = history.lock()
                    .unwrap()
                    .report(&|ranges| ranges.last().unwrap_or(&(0..0)).end == u64::MAX);
                reply::with_status(reply::json(&report), StatusCode::OK)
            })
    }
}
