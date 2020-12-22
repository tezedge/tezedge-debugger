use std::{convert::TryFrom, fmt, mem};
use redbpf::{load::Loader, Module as RawModule, ringbuf::RingBuffer};
use futures::stream::StreamExt;
use super::{DataDescriptor, bpf_code::CODE};

pub struct Module(RawModule);

impl TryFrom<&[u8]> for DataDescriptor {
    type Error = ();

    // TODO: rewrite safe
    fn try_from(v: &[u8]) -> Result<Self, Self::Error> {
        if v.len() >= mem::size_of::<DataDescriptor>() {
            Ok(unsafe { std::ptr::read(v.as_ptr() as *const Self) })
        } else {
            Err(())
        }
    }
}

impl fmt::Debug for DataDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SnifferItem")
            .field("tag", &self.tag)
            .field("fd", &self.fd)
            .field("size", &self.size)
            .finish()
    }
}

pub struct Event<T> {
    pub map_name: String,
    pub items: Vec<T>,
}

impl Module {
    // TODO: handle error
    pub fn load() -> (Self, impl StreamExt<Item = Event<DataDescriptor>>) {
        let mut loaded = Loader::load(CODE).expect("Error loading BPF program");
        for probe in loaded.kprobes_mut() {
            probe
                .attach_kprobe(&probe.name(), 0)
                .expect(&format!("Error attaching kprobe program {}", probe.name()));
        }
        let events = loaded.events.map(|(map_name, items_bytes)| {
            let mut items = Vec::with_capacity(items_bytes.len());
            for bytes in items_bytes {
                match DataDescriptor::try_from(bytes.as_ref()) {
                    Ok(item) => items.push(item),
                    Err(()) => todo!("log en error"),
                }
            }
            Event {
                map_name: map_name,
                items: items,
            }
        });
        (Module(loaded.module), events)
    }

    pub fn main_buffer(&self) -> RingBuffer {
        let rb_map = self.0.maps.iter().find(|m| m.name == "main_buffer").unwrap();
        RingBuffer::from_map(&rb_map).unwrap()
    }
}
