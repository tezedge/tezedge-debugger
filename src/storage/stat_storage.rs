// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use serde::{Serialize, Deserialize};

#[derive(Clone, Default)]
pub struct StatStore {
    captured_data: Arc<AtomicUsize>,
    deciphered_data: Arc<AtomicUsize>,
    captured_packets: Arc<AtomicUsize>,
    deciphered_packets: Arc<AtomicUsize>,
}

impl StatStore {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn capture_data(&self, data_len: usize) {
        self.captured_data.fetch_add(data_len, Ordering::SeqCst);
        self.captured_packets.fetch_add(1, Ordering::SeqCst);
    }

    pub fn decipher_data(&self, data_len: usize) {
        self.deciphered_data.fetch_add(data_len, Ordering::SeqCst);
        self.deciphered_packets.fetch_add(1, Ordering::SeqCst);
    }

    pub fn snapshot(&self) -> StatSnapshot {
        StatSnapshot {
            captured_data: self.captured_data.load(Ordering::SeqCst),
            deciphered_data: self.deciphered_data.load(Ordering::SeqCst),
            captured_packets: self.captured_packets.load(Ordering::SeqCst),
            deciphered_packets: self.deciphered_data.load(Ordering::SeqCst),
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct StatSnapshot {
    captured_data: usize,
    deciphered_data: usize,
    captured_packets: usize,
    deciphered_packets: usize,
}