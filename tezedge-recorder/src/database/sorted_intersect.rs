// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

/// Module implements sorted intersection algorithm
/// Intersection is an *set* operation returning values
/// that are present in both sets
/// For sets:
/// - A = {1,2,3,4,5}
/// - B = {3,4,5,6,7}
/// Intersection of A and B is set {3,4,5}
///
/// Sorted intersect works on any sorted vectors.

// TODO: try it
/*use generic_array::{GenericArray, ArrayLength};

pub struct SortedIntersect<S>
where
    S: SortedIntersectSource,
{
    inner: S,
}

pub trait SortedIntersectSource {
    type Item;
    type Width: ArrayLength<Option<Self::Item>>;

    fn len(&self) -> usize;
    fn next_all(&mut self) -> &mut GenericArray<Option<Self::Item>, Self::Width>;
    fn next_at(&mut self, index: usize) -> Option<Self::Item>;
}

impl<S> SortedIntersect<S>
where
    S: SortedIntersectSource,
{
    pub fn new(source: S) -> Self {
        SortedIntersect {
            inner: source,
        }
    }
}

impl<S> Iterator for SortedIntersect<S>
where
    S: SortedIntersectSource,
{
    type Item = S::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let _ = &mut self.inner;
        None
    }
}*/

/// For given vector of *sorted* iterators, return new vector containing values
/// present in *every* iterator
pub fn sorted_intersect<I>(iters: &mut [I], limit: usize, forward: bool) -> Vec<I::Item>
where
    I: Iterator,
    I::Item: Ord,
{
    let mut ret = Default::default();
    if iters.is_empty() {
        return ret;
    } else if iters.len() == 1 {
        let iter = iters.iter_mut().next().unwrap();
        ret.extend(iter.take(limit));
        return ret;
    }
    let mut heap = Vec::with_capacity(iters.len());
    // Fill the heap with values
    if !fill_heap(iters.iter_mut(), &mut heap, forward) {
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
                if !fill_heap(iters.iter_mut(), &mut heap, forward) {
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
                    heapify(&mut heap, forward);
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
fn heapify<Item: Ord>(heap: &mut Vec<(Item, usize)>, forward: bool) {
    heap.sort_by(|(a, _), (b, _)| {
        if forward {
            a.cmp(b).reverse()
        } else {
            a.cmp(b)
        }
    });
}

/// Fill heap with new values
fn fill_heap<
    'a,
    Item: Ord,
    Inner: 'a + Iterator<Item = Item>,
    Outer: Iterator<Item = &'a mut Inner>,
>(
    iters: Outer,
    heap: &mut Vec<(Inner::Item, usize)>,
    forward: bool,
) -> bool {
    for (i, iter) in iters.enumerate() {
        let value = iter.next();
        if let Some(value) = value {
            heap.push((value, i))
        } else {
            return false;
        }
    }
    heapify(heap, forward);
    true
}

/// Check if top of the heap is a hit, meaning if it should be contained in the
/// resulting set
fn is_hit<Item: Ord>(heap: &[(Item, usize)]) -> bool {
    let value = heap.iter().next().map(|(value, _)| {
        heap.iter().fold((value, true), |(a, eq), (b, _)| (b, eq & a.eq(b)))
    });

    matches!(value, Some((_, true)))
}
