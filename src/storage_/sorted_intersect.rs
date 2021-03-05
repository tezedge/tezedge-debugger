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

// TODO: turn to work with iterator if possible
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
