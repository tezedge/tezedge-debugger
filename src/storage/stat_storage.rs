// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use serde::{Serialize, Deserialize};

#[derive(Clone, Default)]
/// Global statistics store
pub struct StatStore {
    captured_data: Arc<AtomicUsize>,
    deciphered_data: Arc<AtomicUsize>,
    processed_data: Arc<AtomicUsize>,
    captured_packets: Arc<AtomicUsize>,
    deciphered_packets: Arc<AtomicUsize>,
}

impl StatStore {
    /// Build new statistics store
    pub fn new() -> Self {
        Default::default()
    }

    /// Increase capture data by specific size
    pub fn capture_data(&self, data_len: usize) {
        self.captured_data.fetch_add(data_len, Ordering::SeqCst);
        self.captured_packets.fetch_add(1, Ordering::SeqCst);
    }

    /// Increase processed data size
    pub fn processed_data(&self, data_len: usize) {
        self.deciphered_packets.fetch_add(1, Ordering::SeqCst);
        self.processed_data.fetch_add(data_len, Ordering::SeqCst);
    }

    /// Increase deciphered data
    pub fn decipher_data(&self, data_len: usize) {
        self.deciphered_data.fetch_add(data_len, Ordering::SeqCst);
        self.processed_data(data_len);
    }

    /// Create statistics snapshot
    pub fn snapshot(&self) -> StatSnapshot {
        StatSnapshot {
            captured_data: self.captured_data.load(Ordering::SeqCst),
            processed_data: self.processed_data.load(Ordering::SeqCst),
            deciphered_data: self.deciphered_data.load(Ordering::SeqCst),
            captured_packets: self.captured_packets.load(Ordering::SeqCst),
            deciphered_packets: self.deciphered_data.load(Ordering::SeqCst),
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
/// Statistics snapshot sent by the statistics endpoint
pub struct StatSnapshot {
    captured_data: usize,
    processed_data: usize,
    deciphered_data: usize,
    captured_packets: usize,
    deciphered_packets: usize,
}