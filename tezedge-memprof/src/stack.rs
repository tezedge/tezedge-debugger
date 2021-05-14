use std::{collections::{HashMap, HashSet}, sync::{Arc, RwLock, atomic::{AtomicU32, Ordering}}};
use bpf_memprof::{Hex64, Hex32};
use serde::Serialize;
use super::{memory_map::ProcessMap, table::SymbolTable};

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInfo {
    offset: Option<Hex32>,
    executable: Option<String>,
    virtual_address: Option<Hex64>,
    function_name: Option<String>,
}

#[derive(Default)]
pub struct StackResolver {
    files: HashMap<String, SymbolTable>,
    map: Option<ProcessMap>,
}

impl StackResolver {
    pub fn spawn(pid: Arc<AtomicU32>) -> Arc<RwLock<Self>> {
        use std::{time::Duration, thread};

        let resolver = Arc::new(RwLock::new(StackResolver::default()));
        let resolver_ref = resolver.clone();
        thread::spawn(move || {
            let mut last_map = None::<ProcessMap>;
            let mut files = HashSet::new();

            loop {
                let delay = Duration::from_secs(5);
                thread::sleep(delay);
    
                let pid = pid.load(Ordering::Relaxed);
                if pid != 0 {
                    match ProcessMap::new(pid) {
                        Ok(map) => {
                            if Some(&map) != last_map.as_ref() {
                                last_map = Some(map.clone());
                                for filename in map.files() {
                                    if !files.contains(&filename) {
                                        log::info!("try load symbols for: {}", filename);
                                        match SymbolTable::load(&filename) {
                                            Ok(table) => {
                                                log::info!("loaded {} symbols from: {}", table.len(), filename);
                                                let mut guard = resolver_ref.write().unwrap();
                                                guard.files.insert(filename.clone(), table);
                                                drop(guard);
                                            },
                                            Err(error) => {
                                                log::info!(
                                                    "failed to load symbols for: {}, {}",
                                                    filename,
                                                    error,
                                                );
                                            }
                                        }
                                        files.insert(filename);
                                    }
                                }
                                resolver_ref.write().unwrap().map = Some(map);
                            }
                        },
                        Err(error) => {
                            if last_map.is_none() {
                                log::error!("cannot get process map: {}", error);
                            }
                        },
                    }
                }
            }
        });

        resolver
    }

    fn try_resolve(&self, address: u64) -> Option<((usize, &str), Option<&String>)> {
        let map = self.map.as_ref()?;
        let (filename, offset) = map.find(address as usize)?;
        let table = self.files.get(&filename)?;
        Some(((offset, table.name()), table.find(offset as u64)))
    }

    pub fn resolve(&self, address: u64) -> SymbolInfo {
        match self.try_resolve(address) {
            None => SymbolInfo {
                offset: None,
                executable: None,
                virtual_address: Some(Hex64(address)),
                function_name: None,
            },
            Some(((offset, file), name)) => SymbolInfo {
                offset: Some(Hex32(offset as _)),
                executable: Some(file.to_string()),
                virtual_address: Some(Hex64(address)),
                function_name: name.cloned(),
            },
        }
    }
}
