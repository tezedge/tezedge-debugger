// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use bpf_memprof_common::{Stack, Hex64, Hex32};
use super::{Page, AllocationState, History, EventLast, Tracker, Reporter};
use crate::{StackResolver, Aggregator};

fn allocate_sequence<T, I, F>(history: T, pages: I, stack: F) -> T
where
    T: Tracker + Reporter,
    I: Iterator<Item = u64>,
    F: Fn(u64) -> u64,
{
    pages.fold(history, |mut h, i| {
        let stack = Stack::from_frames(&[stack(i)]);
        let page = Page::new(Hex64(i), 0);
        h.track_alloc(page, &stack, Hex32(0), 0);
        h
    })
}

fn deallocate_sequence<T, I>(history: T, pages: I) -> T
where
    T: Tracker + Reporter,
    I: Iterator<Item = u64>,
{
    pages.fold(history, |mut h, i| {
        let page = Page::new(Hex64(i), 0);
        h.track_free(page, 0);
        h
    })
}

fn alloc<T>()
where
    T: Default + Tracker + Reporter,
{
    let history = allocate_sequence(T::default(), 0..0x1000, |_| 1);
    let (value, cache) = history.short_report();
    assert_eq!(value, 0x1000 * 4);
    assert_eq!(cache, 0);
}

fn alloc_free<T>()
where
    T: Default + Tracker + Reporter,
{
    let history = allocate_sequence(T::default(), 0..0x1000, |_| 1);
    let history = deallocate_sequence(history, 0x600..0xa00);
    let (value, cache) = history.short_report();
    assert_eq!(value, 0xc00 * 4);
    assert_eq!(cache, 0);
}

fn free_without_alloc<T>()
where
    T: Default + Tracker + Reporter,
{
    let history = allocate_sequence(T::default(), 0..0x1000, |_| 1);
    let history = deallocate_sequence(history, 0xa00..0x1100);
    let (value, cache) = history.short_report();
    assert_eq!(value, 0xa00 * 4);
    assert_eq!(cache, 0);
}

fn double_alloc<T>()
where
    T: Default + Tracker + Reporter,
{
    let history = allocate_sequence(T::default(), 0..0x1000, |_| 1);
    let history = allocate_sequence(history, 0x100..0x1100, |_| 1);
    let (value, cache) = history.short_report();
    assert_eq!(value, 0x1100 * 4);
    assert_eq!(cache, 0);
}

fn alloc_random<T>()
where
    T: Default + Tracker + Reporter,
{
    let mut pages = HashSet::<u64>::new();
    let mut history = T::default();
    let stack = Stack::from_frames(&[]);
    for _ in 0..0x1000 {
        let page_i = rand::random::<u64>() % 0x1000;
        pages.insert(page_i);
        let page = Page::new(Hex64(page_i), 0);
        history.track_alloc(page, &stack, Hex32(0), 0);
    }

    let (value, cache) = history.short_report();
    assert_eq!(value, pages.len() as u64 * 4);
    assert_eq!(cache, 0);
}

fn free_random<T>()
where
    T: Default + Tracker + Reporter,
{
    let mut pages = HashSet::<u64>::new();
    let mut history = allocate_sequence(T::default(), 0..0x1000, |_| 1);
    for _ in 0..0x1000 {
        let page_i = rand::random::<u64>() % 0x1000;
        pages.insert(page_i);
        let page = Page::new(Hex64(page_i), 0);
        history.track_free(page, 0);
    }

    let (value, cache) = history.short_report();
    assert_eq!(value, (0x1000 - pages.len() as u64) * 4);
    assert_eq!(cache, 0);
}

fn alloc_free_random<T>()
where
    T: Default + Tracker + Reporter,
{
    let mut pages = HashSet::<u64>::new();
    let mut history = T::default();
    let stack = Stack::from_frames(&[]);
    for _ in 0..0x1000 {
        let page_i = rand::random::<u64>() % 0x1000;
        pages.insert(page_i);
        let page = Page::new(Hex64(page_i), 0);
        history.track_alloc(page, &stack, Hex32(0), 0);
    }

    let (value, cache) = history.short_report();
    assert_eq!(value, pages.len() as u64 * 4);
    assert_eq!(cache, 0);

    for _ in 0..0x1000 {
        let page_i = rand::random::<u64>() % 0x1000;
        pages.remove(&page_i);
        let page = Page::new(Hex64(page_i), 0);
        history.track_free(page, 0);
    }

    let (value, cache) = history.short_report();
    assert_eq!(value, pages.len() as u64 * 4);
    assert_eq!(cache, 0);
}

fn alloc_cache_random<T>()
where
    T: Default + Tracker + Reporter,
{
    let mut pages = HashSet::<u64>::new();
    let mut cache_pages = HashSet::<u64>::new();
    let mut history = T::default();
    let stack = Stack::from_frames(&[]);
    for _ in 0..0x1000 {
        let page_i = rand::random::<u64>() % 0x1000;
        pages.insert(page_i);
        let page = Page::new(Hex64(page_i), 0);
        history.track_alloc(page, &stack, Hex32(0), 0);
        if rand::random::<bool>() {
            cache_pages.insert(page_i);
            history.mark_page_cache(page, true);
        }
    }

    let (value, cache) = history.short_report();
    println!("{}, {}", value, cache);
    assert_eq!(value, pages.len() as u64 * 4);
    assert_eq!(cache, cache_pages.len() as u64 * 4);
}

fn free_cache_random<T>()
where
    T: Default + Tracker + Reporter,
{
    let mut pages = HashSet::<u64>::new();
    let mut cache_pages = HashSet::<u64>::new();
    let mut history = allocate_sequence(T::default(), 0..0x1000, |_| 1);
    for page_i in 0..0x1000 {
        let page = Page::new(Hex64(page_i), 0);
        if rand::random::<bool>() {
            cache_pages.insert(page_i);
            history.mark_page_cache(page, true);
        }
    }

    for _ in 0..0x1000 {
        let page_i = rand::random::<u64>() % 0x1000;
        let page = Page::new(Hex64(page_i), 0);

        pages.insert(page_i);
        cache_pages.remove(&page_i);
        history.track_free(page, 0);
    }

    let (value, cache) = history.short_report();
    assert_eq!(value, (0x1000 - pages.len() as u64) * 4);
    assert_eq!(cache, cache_pages.len() as u64 * 4);
}

fn alloc_free_cache_random<T>()
where
    T: Default + Tracker + Reporter,
{
    let mut pages = HashSet::<u64>::new();
    let mut cache_pages = HashSet::<u64>::new();
    let mut history = AllocationState::default();
    let stack = Stack::from_frames(&[]);
    for _ in 0..0x1000 {
        let page_i = rand::random::<u64>() % 0x1000;
        pages.insert(page_i);
        let page = Page::new(Hex64(page_i), 0);
        history.track_alloc(page, &stack, Hex32(0), 0);
        if rand::random::<bool>() {
            cache_pages.insert(page_i);
            history.mark_page_cache(page, true);
        }
    }

    let (value, cache) = history.short_report();
    assert_eq!(value, pages.len() as u64 * 4);
    assert_eq!(cache, cache_pages.len() as u64 * 4);

    for _ in 0..0x1000 {
        let page_i = rand::random::<u64>() % 0x1000;
        let page = Page::new(Hex64(page_i), 0);

        pages.remove(&page_i);
        cache_pages.remove(&page_i);
        history.track_free(page, 0);
    }

    let (value, cache) = history.short_report();
    assert_eq!(value, pages.len() as u64 * 4);
    assert_eq!(cache, cache_pages.len() as u64 * 4);
}

fn alloc_in_different_stacks<T>()
where
    T: Default + Tracker + Reporter,
{
    let history = allocate_sequence(T::default(), 0..0x1000, |i| (i * 7) % 0x100);
    let resolver = StackResolver::mock();

    let tree = history.tree_report(&resolver, 0, false);
    assert_eq!(tree.value(), 0x1000 * 4);
    assert_eq!(tree.cache_value(), 0);
    let _ = serde_json::to_string_pretty(&tree).unwrap();
}

#[test]
fn alloc_simple() {
    alloc::<AllocationState>()
}

#[test]
fn alloc_history() {
    alloc::<History<EventLast>>()
}

#[test]
fn alloc_aggregator() {
    alloc::<Aggregator>()
}

#[test]
fn alloc_free_simple() {
    alloc_free::<AllocationState>()
}

#[test]
fn alloc_free_history() {
    alloc_free::<History<EventLast>>()
}

#[test]
fn alloc_free_aggregator() {
    alloc_free::<Aggregator>()
}

#[test]
fn free_without_alloc_simple() {
    free_without_alloc::<AllocationState>()
}

#[test]
fn free_without_alloc_history() {
    free_without_alloc::<History<EventLast>>()
}

#[test]
fn free_without_alloc_aggregator() {
    free_without_alloc::<Aggregator>()
}

#[test]
fn double_alloc_simple() {
    double_alloc::<AllocationState>()
}

#[test]
fn double_alloc_history() {
    double_alloc::<History<EventLast>>()
}

#[test]
fn alloc_random_simple() {
    alloc_random::<AllocationState>()
}

#[test]
fn alloc_random_history() {
    alloc_random::<History<EventLast>>()
}

#[test]
fn free_random_simple() {
    free_random::<AllocationState>()
}

#[test]
fn free_random_history() {
    free_random::<History<EventLast>>()
}

#[test]
fn free_random_aggregator() {
    free_random::<Aggregator>()
}

#[test]
fn alloc_free_random_simple() {
    alloc_free_random::<AllocationState>()
}

#[test]
fn alloc_free_random_history() {
    alloc_free_random::<History<EventLast>>()
}

#[test]
fn alloc_cache_random_simple() {
    alloc_cache_random::<AllocationState>()
}

#[test]
fn alloc_cache_random_history() {
    alloc_cache_random::<History<EventLast>>()
}

#[test]
fn free_cache_random_simple() {
    free_cache_random::<AllocationState>()
}

#[test]
fn free_cache_random_history() {
    free_cache_random::<History<EventLast>>()
}

#[test]
fn free_cache_random_aggregator() {
    free_cache_random::<Aggregator>()
}

#[test]
fn alloc_free_cache_random_simple() {
    alloc_free_cache_random::<AllocationState>()
}

#[test]
fn alloc_free_cache_random_history() {
    alloc_free_cache_random::<History<EventLast>>()
}

#[test]
fn alloc_free_cache_random_aggregator() {
    alloc_free_cache_random::<Aggregator>()
}

#[test]
fn alloc_in_different_stacks_simple() {
    alloc_in_different_stacks::<AllocationState>()
}

#[test]
fn alloc_in_different_stacks_history() {
    alloc_in_different_stacks::<History<EventLast>>()
}

#[test]
fn alloc_in_different_stacks_aggregator() {
    alloc_in_different_stacks::<Aggregator>()
}
