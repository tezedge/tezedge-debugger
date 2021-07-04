// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::ops::Deref;
use std::sync::{Arc, Mutex, atomic::{Ordering, AtomicU32}};
use bpf_memprof_common::{EventKind, Event};
use super::{Reporter, StackResolver, FrameReport, aggregator::Aggregator};

impl Reporter for Aggregator {
    fn short_report(&self) -> (u64, u64) {
        let (mut value, mut cache_value) = (0, 0);
        for (v, c, _) in self.report() {
            value += v;
            cache_value += c;
        }

        (value, cache_value)
    }

    fn tree_report<R>(&self, resolver: R, threshold: u64, reverse: bool) -> FrameReport<R>
    where
        R: Deref<Target = StackResolver>,
    {
        let mut report = FrameReport::new(resolver);
        for (value, cache_value, stack) in self.report() {
            if reverse {
                report.inner.insert(stack.iter().rev(), value, cache_value);
            } else {
                report.inner.insert(stack.iter(), value, cache_value);
            }
        }
        report.inner.strip(threshold);

        report

    }
}

#[derive(Default)]
pub struct Consumer {
    has_pid: bool,
    pid: Arc<AtomicU32>,
    aggregator: Arc<Mutex<Aggregator>>,
    last: Option<EventKind>,
}

impl Consumer {
    pub fn reporter(&self) -> Arc<Mutex<Aggregator>> {
        self.aggregator.clone()
    }

    pub fn pid(&self) -> Arc<AtomicU32> {
        self.pid.clone()
    }
}

impl Consumer {
    pub fn arrive(&mut self, data: &[u8]) {
        let event = match Event::from_slice(data) {
            Ok(v) => v,
            Err(error) => {
                log::error!("failed to read slice from kernel: {}", error);
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
                self.has_pid = true;
                self.pid.store(event.pid, Ordering::SeqCst);
                self.aggregator.lock().unwrap().track_alloc(v.pfn.0 as u32, v.order as u8, &event.stack, event.pid);
            }
            &EventKind::PageFree(ref v) if v.pfn.0 != 0 && self.has_pid => {
                self.aggregator.lock().unwrap().track_free(v.pfn.0 as u32, v.order as u8, event.pid);
            },
            &EventKind::AddToPageCache(ref v) if v.pfn.0 != 0 && self.has_pid => {
                self.aggregator.lock().unwrap().mark_cache(v.pfn.0 as u32, true, event.pid);
            },
            &EventKind::RemoveFromPageCache(ref v) if v.pfn.0 != 0 && self.has_pid => {
                self.aggregator.lock().unwrap().mark_cache(v.pfn.0 as u32, false, event.pid);
            },
            &EventKind::RssStat(ref v) if v.member == 1 && self.has_pid => {
                self.aggregator.lock().unwrap().track_rss_anon(v.size as _);
            },
            &EventKind::MigratePages(ref v) => {
                log::warn!("{:?}", v);
            },
            _ => (),
        }
        self.last = Some(event.event);
    }
}
