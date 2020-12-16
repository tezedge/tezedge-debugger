use std::{convert::TryFrom, fmt};
use redbpf::{load::Loader, Module as RawModule};
use futures::stream::StreamExt;
use super::SnifferItem;

pub struct Module(RawModule);

impl TryFrom<&[u8]> for SnifferItem {
    type Error = ();

    // TODO: rewrite safe
    fn try_from(v: &[u8]) -> Result<Self, Self::Error> {
        if v.len() >= Self::SIZE + 8 {
            Ok(unsafe { std::ptr::read(v.as_ptr() as *const Self) })
        } else {
            Err(())
        }
    }
}

impl fmt::Debug for SnifferItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SnifferItem")
            .field("tag", &self.tag)
            .field("fd", &self.fd)
            .field("offset", &self.offset)
            .field("size", &self.size)
            .field("data", &hex::encode(self.data.as_ref()))
            .finish()
    }
}

pub struct Event<T> {
    pub map_name: String,
    pub items: Vec<T>,
}

impl Module {
    // TODO: handle error
    pub fn load() -> (Self, impl StreamExt<Item = Event<SnifferItem>>) {
        let code = include_bytes!(concat!(env!("OUT_DIR"), "/target/bpf/programs/kprobe/kprobe.elf"));
        let mut loaded = Loader::load(code).expect("Error loading BPF program");
        for probe in loaded.kprobes_mut() {
            probe.attach_kprobe(&probe.name(), 0)
                .expect(&format!("Error attaching xdp program {}", probe.name()));
        }
        let events = loaded.events.map(|(map_name, items_bytes)| {
            let mut items = Vec::with_capacity(items_bytes.len());
            for bytes in items_bytes {
                match SnifferItem::try_from(bytes.as_ref()) {
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
}
