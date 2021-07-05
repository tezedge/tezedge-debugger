// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redbpf::{load::Loader, Module, HashMap, Map};
use bpf_sniffer_common::SocketId;

pub struct BpfModule {
    inner: Module,
    counter_map_index: usize,
}

#[repr(C)]
struct AlignedTo<A, B>
where
    B: ?Sized,
{
    _align: [A; 0],
    bytes: B,
}

pub static CODE: &'static [u8] = {
    static _ALIGNED: &'static AlignedTo<u64, [u8]> = &AlignedTo {
        _align: [],
        bytes: *include_bytes!(concat!(env!("OUT_DIR"), "/target/bpf/programs/kprobe/kprobe.elf")),
    };
    &_ALIGNED.bytes
};

impl BpfModule {
    // TODO: handle error
    pub fn load() -> Self {
        let mut loaded = Loader::load(CODE).expect("Error loading BPF program");
        for probe in loaded.kprobes_mut() {
            // try to detach the kprobe, if previous run of the sniffer did not cleanup
            let _ = probe
                .detach_kprobe_namespace("default", &probe.name());
            probe
                .attach_kprobe_namespace("default", &probe.name(), 0)
                .expect(&format!("Error attaching kprobe program {}", probe.name()));
        }
        let i = loaded.module.maps.iter().enumerate().find(|&(_, m)| m.name == "overall_counter").unwrap().0;
        BpfModule {
            inner: loaded.module,
            counter_map_index: i,
        }
    }

    pub fn main_buffer_map(&self) -> &Map {
        self
            .inner
            .maps
            .iter()
            .find(|m| m.name == "main_buffer")
            .unwrap()
    }

    fn connections_map(&self) -> HashMap<SocketId, u32> {
        let map = self
            .inner
            .maps
            .iter()
            .find(|m| m.name == "connections")
            .unwrap();
        HashMap::new(map).unwrap()
    }

    pub fn ignore(&self, id: SocketId) {
        self.connections_map().delete(id);
    }

    fn ports_to_watch_map(&self) -> HashMap<u16, u32> {
        let map = self
            .inner
            .maps
            .iter()
            .find(|m| m.name == "ports")
            .unwrap();
        HashMap::new(map).unwrap()
    }

    pub fn watch_port(&self, port: u16) {
        self.ports_to_watch_map().set(port, 1)
    }

    pub fn get_counter(&self) -> u64 {
        HashMap::new(&self.inner.maps[self.counter_map_index]).unwrap().get(0 as u32).unwrap_or(0)
    }
}
