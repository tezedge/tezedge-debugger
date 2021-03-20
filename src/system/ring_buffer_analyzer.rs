use std::{fs::File, io::{self, Write}, path::Path};
use bpf_common::RingBufferObserver;

pub fn dump<P, Q>(json_path: P, bin_path: Q, observer: &RingBufferObserver) -> io::Result<()>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let it = RingBufferIterator::new(observer);
    let descriptors = it.collect::<Vec<_>>();
    let json = serde_json::to_string(&descriptors)?;
    File::create(json_path)?.write_all(json.as_bytes())?;

    let len = observer.len() / 2;
    let content = &observer.as_ref()[..len];
    File::create(bin_path)?.write_all(content)?;

    Ok(())
}

pub struct RingBufferIterator<'a> {
    pos: usize,
    observer: &'a RingBufferObserver,
}

#[derive(serde::Serialize)]
pub struct RingBufferSliceDescriptor {
    pid: u32,
    fd: u32,
    ts: u64,
    tag: String,
    pos: u32,
    size: u32,
}

impl<'a> RingBufferIterator<'a> {
    fn new(observer: &'a RingBufferObserver) -> Self {
        let producer_pos = observer.producer_pos();
        let content = observer.as_ref();
        if producer_pos <= content.len() / 2 {
            // first loop ever, simple case
            // let's traversal the ring buffer from 0 to 1GiB
            RingBufferIterator {
                pos: 0,
                observer,
            }
        } else {
            // need to determine where start to read, not simple case
            //
            RingBufferIterator {
                pos: 0, // TODO:
                observer,
            }
        }
    }
}

impl<'a> Iterator for RingBufferIterator<'a> {
    type Item = RingBufferSliceDescriptor;

    fn next(&mut self) -> Option<Self::Item> {
        //unimplemented!()
        let _ = (&self.pos, &self.observer);
        None
    }
}
