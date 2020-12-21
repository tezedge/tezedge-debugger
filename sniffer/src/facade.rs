use std::{convert::TryFrom, fmt, mem};
use redbpf::{load::Loader, Module as RawModule, ringbuf::{RingBufferManager, RingBufferItem}};
use futures::stream::StreamExt;
use super::DataDescriptor;

pub struct Module(RawModule, RingBufferManager);

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

impl RingBufferItem for DataDescriptor {
    fn consume(slice: &[u8]) -> Self {
        Self::try_from(slice).unwrap()
    }
}

impl fmt::Debug for DataDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SnifferItem")
            .field("tag", &self.tag)
            .field("fd", &self.fd)
            .field("offset", &self.offset)
            .field("size", &self.size)
            .field("overall_size", &self.overall_size)
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
        let code = include_bytes!(concat!(
            env!("OUT_DIR"),
            "/target/bpf/programs/kprobe/kprobe.elf"
        ));
        let mut loaded = Loader::load(code).expect("Error loading BPF program");
        for probe in loaded.kprobes_mut() {
            probe
                .attach_kprobe(&probe.name(), 0)
                .expect(&format!("Error attaching xdp program {}", probe.name()));
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
        (Module(loaded.module, RingBufferManager::new()), events)
    }

    pub fn rb_events(&mut self) -> impl StreamExt<Item = Event<DataDescriptor>> + '_ {
        let rb_map = self.0.maps.iter().find(|m| m.kind == 27).unwrap();
        let rb = self.1.add(rb_map.fd).unwrap();
        rb.map(move |items| {
            Event {
                map_name: String::new(),
                items: items,
            }
        })
    }
}
