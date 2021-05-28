use std::{collections::{HashMap, HashSet}, sync::{Arc, RwLock, atomic::{AtomicU32, Ordering}}};
use bpf_memprof::Hex32;
use serde::Serialize;
use super::{memory_map::ProcessMap, table::SymbolTable};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInfo {
    offset: Hex32,
    executable: String,
    function_name: Option<String>,
    function_category: String,
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

    pub fn resolve(&self, address: u64) -> Option<SymbolInfo> {
        let ((offset, filename), name) = self.try_resolve(address)?;

        fn cpp_demangle(s: &str) -> Option<String> {
            cpp_demangle::Symbol::new(s).ok()?.demangle(&Default::default()).ok()
        }

        let function_category = if filename == "light-node" {
            if name.map(|n| is_rust(n)).unwrap_or(false) {
                "nodeRust".to_string()
            } else {
                "nodeCpp".to_string()
            }
        } else {
            "systemLib".to_string()
        };

        Some(SymbolInfo {
            offset: Hex32(offset as _),
            executable: filename.to_string(),
            function_name: name
                .map(|n| {
                    if is_rust(n) {
                        rustc_demangle::demangle(n).to_string()
                    } else {
                        cpp_demangle(n).unwrap_or(n.clone())
                    }
                }),
            function_category,
        })
    }
}

fn is_rust(s: &str) -> bool {
    fn inner(s: &str) -> bool {
        let s = s.trim_end_matches('E');
        let l = s.bytes().len();
        if l < 17 {
            return false;
        }

        let h = s.as_bytes()[l - 17] == 'h' as u8;
        s.as_bytes()[(l - 16)..].iter().fold(h, |h, b| h && b.is_ascii_hexdigit())
    }

    s.split_whitespace().any(inner) || s.split(".llvm").any(inner)
}
