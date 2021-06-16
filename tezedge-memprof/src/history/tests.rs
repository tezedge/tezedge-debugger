use bpf_memprof::{Stack, Hex64, Hex32};
use super::{History, Page, EventLast};

fn allocate_sequence<I, F>(pages: I, stack: F) -> History<EventLast>
where
    I: Iterator<Item = u64>,
    F: Fn(u64) -> u64,
{
    let history = History::<EventLast>::default();

    pages.fold(history, |mut h, i| {
        let stack = Stack::from_frames(&[stack(i)]);
        let page = Page::new(Hex64(i), 0);
        h.track_alloc(page, &stack, Hex32(0));
        h
    })
}

fn deallocate_sequence<I>(pages: I, history: History<EventLast>) -> History<EventLast>
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
    let history = allocate_sequence(0..0x1000, |_| 1);
    let (value, cache) = history.short_report();
    assert_eq!(value, 0x1000 * 4);
    assert_eq!(cache, 0);
}

#[test]
fn alloc_free() {
    let history = allocate_sequence(0..0x1000, |_| 1);
    let history = deallocate_sequence(0x600..0xa00, history);
    let (value, cache) = history.short_report();
    assert_eq!(value, 0xc00 * 4);
    assert_eq!(cache, 0);
}

#[test]
fn free_without_alloc() {
    let history = allocate_sequence(0..0x1000, |_| 1);
    let history = deallocate_sequence(0xa00..0x1100, history);
    let (value, cache) = history.short_report();
    assert_eq!(value, 0xa00 * 4);
    assert_eq!(cache, 0);
}
