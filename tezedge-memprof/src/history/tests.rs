use std::collections::HashSet;
use bpf_memprof::{Stack, Hex64, Hex32};
use super::{History, Page, EventLast};
use crate::StackResolver;

fn allocate_sequence<I, F>(history: History<EventLast>, pages: I, stack: F) -> History<EventLast>
where
    I: Iterator<Item = u64>,
    F: Fn(u64) -> u64,
{
    pages.fold(history, |mut h, i| {
        let stack = Stack::from_frames(&[stack(i)]);
        let page = Page::new(Hex64(i), 0);
        h.track_alloc(page, &stack, Hex32(0));
        h
    })
}

fn deallocate_sequence<I>(history: History<EventLast>, pages: I) -> History<EventLast>
where
    I: Iterator<Item = u64>,
{
    pages.fold(history, |mut h, i| {
        let page = Page::new(Hex64(i), 0);
        h.track_free(page, 0);
        h
    })
}

#[test]
fn alloc() {
    let history = allocate_sequence(History::default(), 0..0x1000, |_| 1);
    let (value, cache) = history.short_report();
    assert_eq!(value, 0x1000 * 4);
    assert_eq!(cache, 0);
}

#[test]
fn alloc_free() {
    let history = allocate_sequence(History::default(), 0..0x1000, |_| 1);
    let history = deallocate_sequence(history, 0x600..0xa00);
    let (value, cache) = history.short_report();
    assert_eq!(value, 0xc00 * 4);
    assert_eq!(cache, 0);
}

#[test]
fn free_without_alloc() {
    let history = allocate_sequence(History::default(), 0..0x1000, |_| 1);
    let history = deallocate_sequence(history, 0xa00..0x1100);
    let (value, cache) = history.short_report();
    assert_eq!(value, 0xa00 * 4);
    assert_eq!(cache, 0);
}

#[test]
fn double_alloc() {
    let history = allocate_sequence(History::default(), 0..0x1000, |_| 1);
    let history = allocate_sequence(history, 0x100..0x1100, |_| 1);
    let (value, cache) = history.short_report();
    assert_eq!(value, 0x1100 * 4);
    assert_eq!(cache, 0);
}

#[test]
fn alloc_random() {
    let mut pages = HashSet::<u64>::new();
    let mut history = History::<EventLast>::default();
    let stack = Stack::from_frames(&[]);
    for _ in 0..0x1000 {
        let page_i = rand::random::<u64>() % 0x1000;
        pages.insert(page_i);
        let page = Page::new(Hex64(page_i), 0);
        history.track_alloc(page, &stack, Hex32(0));
    }

    let (value, cache) = history.short_report();
    assert_eq!(value, pages.len() as u64 * 4);
    assert_eq!(cache, 0);
}

#[test]
fn free_random() {
    let mut pages = HashSet::<u64>::new();
    let mut history = allocate_sequence(History::default(), 0..0x1000, |_| 1);
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

#[test]
fn alloc_free_random() {
    let mut pages = HashSet::<u64>::new();
    let mut history = History::<EventLast>::default();
    let stack = Stack::from_frames(&[]);
    for _ in 0..0x1000 {
        let page_i = rand::random::<u64>() % 0x1000;
        pages.insert(page_i);
        let page = Page::new(Hex64(page_i), 0);
        history.track_alloc(page, &stack, Hex32(0));
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

#[test]
fn alloc_cache_random() {
    let mut pages = HashSet::<u64>::new();
    let mut cache_pages = HashSet::<u64>::new();
    let mut history = History::<EventLast>::default();
    let stack = Stack::from_frames(&[]);
    for _ in 0..0x1000 {
        let page_i = rand::random::<u64>() % 0x1000;
        pages.insert(page_i);
        let page = Page::new(Hex64(page_i), 0);
        history.track_alloc(page, &stack, Hex32(0));
        if rand::random::<bool>() {
            cache_pages.insert(page_i);
            history.mark_page_cache(page, true);
        }
    }

    let (value, cache) = history.short_report();
    assert_eq!(value, pages.len() as u64 * 4);
    assert_eq!(cache, cache_pages.len() as u64 * 4);
}

#[test]
fn free_cache_random() {
    let mut pages = HashSet::<u64>::new();
    let mut cache_pages = HashSet::<u64>::new();
    let mut history = allocate_sequence(History::<EventLast>::default(), 0..0x1000, |_| 1);
    for page_i in 0..0x1000 {
        let page = Page::new(Hex64(page_i), 0);
        if rand::random::<bool>() {
            cache_pages.insert(page_i);
            history.mark_page_cache(page, true);
        }
    }

    for _ in 0..0x1000 {
        let page_i = rand::random::<u64>() % 0x1000;
        pages.insert(page_i);
        cache_pages.remove(&page_i);
        let page = Page::new(Hex64(page_i), 0);
        history.track_free(page, 0);
    }

    let (value, cache) = history.short_report();
    assert_eq!(value, (0x1000 - pages.len() as u64) * 4);
    assert_eq!(cache, cache_pages.len() as u64 * 4);
}

#[test]
fn alloc_free_cache_random() {
    let mut pages = HashSet::<u64>::new();
    let mut cache_pages = HashSet::<u64>::new();
    let mut history = History::<EventLast>::default();
    let stack = Stack::from_frames(&[]);
    for _ in 0..0x1000 {
        let page_i = rand::random::<u64>() % 0x1000;
        pages.insert(page_i);
        let page = Page::new(Hex64(page_i), 0);
        history.track_alloc(page, &stack, Hex32(0));
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
        pages.remove(&page_i);
        cache_pages.remove(&page_i);
        let page = Page::new(Hex64(page_i), 0);
        history.track_free(page, 0);
    }

    let (value, cache) = history.short_report();
    assert_eq!(value, pages.len() as u64 * 4);
    assert_eq!(cache, cache_pages.len() as u64 * 4);
}

#[test]
fn alloc_in_different_stacks() {
    let history = allocate_sequence(History::default(), 0..0x1000, |i| (i * 7) % 0x100);
    let resolver = StackResolver::mock();

    let tree = history.tree_report(&resolver, 0, false);
    assert_eq!(tree.value(), 0x1000 * 4);
    assert_eq!(tree.cache_value(), 0);
    let _ = serde_json::to_string_pretty(&tree).unwrap();
}
