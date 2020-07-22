// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod p2p_storage;
mod log_storage;
mod rpc_storage;
mod stat_storage;
mod metric_storage;
mod secondary_index;

pub use p2p_storage::{P2pStore, P2pFilters};
pub use log_storage::{LogStore, LogFilters};
pub use rpc_storage::{RpcStore, RpcFilters};
pub use metric_storage::{MetricStore, MetricFilters};
pub(crate) use p2p_storage::secondary_indexes as p2p_indexes;
pub(crate) use log_storage::secondary_indexes as log_indexes;
pub(crate) use rpc_storage::secondary_indexes as rpc_indexes;

use rocksdb::{DB, ColumnFamilyDescriptor};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
    net::IpAddr,
};
use storage::persistent::KeyValueSchema;
use crate::storage::stat_storage::StatStore;

#[derive(Clone)]
/// Basic store for all captured messages
pub struct MessageStore {
    p2p_db: P2pStore,
    log_db: LogStore,
    rpc_db: RpcStore,
    stat_db: Arc<StatStore>,
    metric_db: MetricStore,
    raw_db: Arc<DB>,
    max_db_size: Option<u64>,
}

impl MessageStore {
    /// Create new store onto given RocksDB database
    pub fn new(db: Arc<DB>) -> Self {
        Self {
            p2p_db: P2pStore::new(db.clone()),
            log_db: LogStore::new(db.clone()),
            rpc_db: RpcStore::new(db.clone()),
            stat_db: Arc::new(StatStore::new()),
            metric_db: MetricStore::new(db.clone()),
            raw_db: db,
            max_db_size: None,
        }
    }

    /// Get p2p message store
    pub fn p2p(&self) -> &P2pStore {
        &self.p2p_db
    }

    /// Get log message store
    pub fn log(&self) -> &LogStore {
        &self.log_db
    }

    /// Get rpc message store
    pub fn rpc(&self) -> &RpcStore {
        &self.rpc_db
    }

    /// Get statistics store
    pub fn stat(&self) -> &StatStore {
        &self.stat_db
    }

    pub fn metric(&self) -> &MetricStore {
        &self.metric_db
    }
}

/// Create list of all Column Family descriptors required for Message store
pub fn cfs() -> Vec<ColumnFamilyDescriptor> {
    vec![
        P2pStore::descriptor(),
        LogStore::descriptor(),
        RpcStore::descriptor(),
        MetricStore::descriptor(),
        p2p_indexes::RemoteAddrIndex::descriptor(),
        p2p_indexes::TypeIndex::descriptor(),
        p2p_indexes::IncomingIndex::descriptor(),
        p2p_indexes::SourceTypeIndex::descriptor(),
        log_indexes::LevelIndex::descriptor(),
        log_indexes::TimestampIndex::descriptor(),
        rpc_indexes::RemoteAddrIndex::descriptor(),
    ]
}

/// Create new UNIX timestamp
pub fn get_ts() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
}

/// Encoding for IPv4/IPv6 address. Both are represented as
/// u128 value. IPv4 is prefixed with 96 zeroed bits
fn encode_address(addr: &IpAddr) -> u128 {
    match addr {
        &IpAddr::V6(addr) => u128::from(addr),
        &IpAddr::V4(addr) => u32::from(addr) as u128,
    }
}

#[allow(dead_code)]
/// Decode some u128 as some valid IP address
fn decode_address(value: u128) -> IpAddr {
    use std::net::{Ipv4Addr, Ipv6Addr};
    if value & 0xFFFFFFFFFFFFFFFFFFFFFFFF00000000 == 0 {
        IpAddr::V4(Ipv4Addr::from(value as u32))
    } else {
        IpAddr::V6(Ipv6Addr::from(value))
    }
}

/// Dissect given number as vector of it binary parts.
/// It is always true, that sum of the parts is equal to the original number
/// For example: number 11 is dissected as values 8, 2 and 1
/// (because: 1011 == 1000 + 10 + 1)
pub fn dissect(mut number: u32) -> Vec<u32> {
    let mut ret: Vec<u32> = Vec::with_capacity(number.count_ones() as usize);
    while number != 0 {
        let value = 0x1 << number.trailing_zeros();
        number = !value & number;
        ret.push(value);
    }
    ret
}

pub mod sorted_intersect {
    /// Module implements sorted intersection algorithm
    /// Intersection is an *set* operation returning values
    /// that are present in both sets
    /// For sets:
    /// - A = {1,2,3,4,5}
    /// - B = {3,4,5,6,7}
    /// Intersection of A and B is set {3,4,5}
    ///
    /// Sorted intersect works on any sorted vectors.
    use std::cmp::Ordering;

    /// For given vector of *sorted* iterators, return new vector containing values
    /// present in *every* iterator
    pub fn sorted_intersect<I>(mut iters: Vec<I>, limit: usize) -> Vec<I::Item>
        where
            I: Iterator,
            I::Item: Ord,
    {
        let mut ret = Default::default();
        if iters.len() == 0 {
            return ret;
        } else if iters.len() == 1 {
            let iter = iters.iter_mut().next().unwrap();
            ret.extend(iter.take(limit));
            return ret;
        }
        let mut heap = Vec::with_capacity(iters.len());
        // Fill the heap with values
        if !fill_heap(iters.iter_mut(), &mut heap) {
            // Hit an exhausted iterator, finish
            return ret;
        }

        while ret.len() < limit {
            if is_hit(&heap) {
                // We hit intersected item
                if let Some((item, _)) = heap.pop() {
                    // Push it into the intersect values
                    ret.push(item);
                    // Clear the rest of the heap
                    heap.clear();
                    // Build a new heap from new values
                    if !fill_heap(iters.iter_mut(), &mut heap) {
                        // Hit an exhausted iterator, finish
                        return ret;
                    }
                } else {
                    // Hit an exhausted iterator, finish
                    return ret;
                }
            } else {
                // Remove max element from the heap
                if let Some((_, iter_num)) = heap.pop() {
                    if let Some(item) = iters[iter_num].next() {
                        // Insert replacement from the corresponding iterator to heap
                        heap.push((item, iter_num));
                        heapify(&mut heap);
                    } else {
                        // Hit an exhausted iterator, finish
                        return ret;
                    }
                } else {
                    // Hit an exhausted iterator, finish
                    return ret;
                }
            }
        }

        ret
    }

    /// Create heap out of vector
    fn heapify<Item: Ord>(heap: &mut Vec<(Item, usize)>) {
        heap.sort_by(|(a, _), (b, _)| a.cmp(b));
    }

    /// Fill heap with new values
    fn fill_heap<'a, Item: Ord, Inner: 'a + Iterator<Item=Item>, Outer: Iterator<Item=&'a mut Inner>>(iters: Outer, heap: &mut Vec<(Inner::Item, usize)>) -> bool {
        for (i, iter) in iters.enumerate() {
            let value = iter.next();
            if let Some(value) = value {
                heap.push((value, i))
            } else {
                return false;
            }
        }
        heapify(heap);
        true
    }

    /// Check if top of the heap is a hit, meaning if it should be contained in the
    /// resulting set
    fn is_hit<Item: Ord>(heap: &Vec<(Item, usize)>) -> bool {
        let value = heap.iter().next().map(|(value, _)|
            heap.iter().fold((value, true), |(a, eq), (b, _)| {
                (b, eq & (a.cmp(b) == Ordering::Equal))
            })
        );

        if let Some((_, true)) = value {
            true
        } else {
            false
        }
    }
}