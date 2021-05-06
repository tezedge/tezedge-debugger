// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs::File, io::Write, collections::HashMap};
    //use bincode::Options;
    use bpf_memprof::Hex64;
    use tezedge_memprof::{History, PageEvent, Frame};

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    #[derive(Debug)]
    enum Anomaly {
        DoesNotAlloc(Hex64),
        DoesNotFree(Hex64),
        Leak(Hex64),
    }

    let mut history = serde_json::from_reader::<_, History>(File::open("target/report.json")?)?;
    history.reorder(0);

    let mut anomalies = File::create("target/anomalies.log")?;
    let mut state = HashMap::new();
    let mut dna = 0;
    let mut dnf = 0;
    let mut leak = 0;
    let mut total_a = 0;
    let mut total_f = 0;
    for &PageEvent { pfn, pages, ref stack, .. } in history.iter() {
        let _ = stack;
        if pages > 0 {
            total_a += pages;
            if state.insert(pfn.0, pages).is_some() {
                anomalies.write_fmt(format_args!("{:?}\n", Anomaly::DoesNotFree(pfn)))?;
                dnf += pages;
            }
        } else if pages < 0 {
            total_f += pages;
            if state.remove(&pfn.0).is_none() {
                anomalies.write_fmt(format_args!("{:?}\n", Anomaly::DoesNotAlloc(pfn)))?;
                dna += pages;
            }
        }
    }
    for (pfn, pages) in state {
        anomalies.write_fmt(format_args!("{:?}\n", Anomaly::Leak(Hex64(pfn))))?;
        if pages > 0 {
            leak += pages;
        }
    }
    log::info!("does not alloc = {}, does not free = {}, leak = {}, total free = {}, total alloc = {}", -dna, dnf, leak, -total_f, total_a);

    let mut frame = Frame::empty();
    for event in history {
        frame.insert(&event);
    }
    frame.strip();
    serde_json::to_writer(File::create("target/tree.json")?, &frame)?;

    Ok(())
}
