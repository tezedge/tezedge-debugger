// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs::File, io::Write, collections::HashMap};
    //use bincode::Options;
    use bpf_memprof::Hex64;
    use tezedge_memprof::{History, PageEvent};

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    #[derive(Debug)]
    enum Anomaly {
        DoesNotAlloc(Hex64),
        DoesNotFree(Hex64),
    }

    let mut history = serde_json::from_reader::<_, History>(File::open("target/report.json")?)?;
    //let opts = bincode::DefaultOptions::default().with_native_endian();
    //let mut history = opts.deserialize_from::<_, History>(File::open("target/report.bin")?)?;
    let mut anomalies = File::create("target/anomalies.log")?;
    let mut state = HashMap::new();
    let mut dna = 0;
    let mut dnf = 0;
    let mut total_a = 0;
    let mut total_f = 0;
    history.reorder(2);
    for PageEvent { pfn, pages, stack, flavour } in history {
        if flavour != 0 && flavour != 1 && flavour != 3 && flavour != 4 {
            continue;
        }
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
        anomalies.write_fmt(format_args!("{:?}\n", Anomaly::DoesNotFree(Hex64(pfn))))?;
        if pages > 0 {
            dnf += pages;
        }
    }
    log::info!("does not alloc = {}, does not free = {}, total free = {}, total alloc = {}", -dna, dnf, -total_f, total_a);

    Ok(())
}
