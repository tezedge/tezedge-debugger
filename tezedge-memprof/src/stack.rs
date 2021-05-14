use std::{collections::{HashMap, HashSet}, ops::Range, path::Path, sync::{Arc, RwLock, atomic::{AtomicU32, Ordering}}};
use super::memory_map::ProcessMap;

#[derive(Default)]
pub struct StackResolver {
    files: HashMap<String, Vec<Symbol>>,
    map: Option<ProcessMap>,
}

struct Symbol {
    code: Range<u64>,
    name: String,
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
                                        match Symbol::load(&filename) {
                                            Ok(symbols) => {
                                                log::info!("loaded {} symbols from: {}", symbols.len(), filename);
                                                let mut guard = resolver_ref.write().unwrap();
                                                guard.files.insert(filename.clone(), symbols);
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

    fn try_resolve(&self, address: u64) -> Option<&String> {
        let map = self.map.as_ref()?;
        let (filename, offset) = map.find(address as usize)?;
        let symbols = self.files.get(&filename)?;
        let offset = offset as u64;
        let mut length = 1 << (63 - (symbols.len() as u64).leading_zeros() as usize);
        // pos points somewhere in the middle of symbols array, and is power of two
        // if 4 <= symbols.len() < 8 => pos == 4
        // do binary search in symbols
        let mut pos = length;
        while length > 0 {
            length >>= 1;
            if pos >= symbols.len() {
                pos -= length;
            } else {
                let symbol = &symbols[pos];
                if symbol.code.contains(&offset) {
                    return Some(&symbol.name);
                } else if symbol.code.start > offset {
                    pos -= length;
                } else {
                    pos += length;
                }
            }
        }
        None
    }

    pub fn resolve(&self, address: u64) -> String {
        if let Some(s) = self.try_resolve(address) {
            format!("{:016x} - \'{}\'", address, s)
        } else {
            format!("{:016x} - unknown", address)
        }
    }
}

impl Symbol {
    pub fn load<P>(path: P) -> Result<Vec<Self>, String>
    where
        P: AsRef<Path>,
    {
        use std::{fs, io::Read};
        use elf64::{Elf64, SectionData};

        let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut data = Vec::new();
        f.read_to_end(&mut data).map_err(|e| e.to_string())?;

        let mut symbols = Vec::new();

        let elf = Elf64::new(&data).map_err(|e| format!("{:?}", e))?;
        let s = elf.section_number();
        let symbol_tables = (0..s)
            .filter_map(|i| {
                let section = elf.section(i).ok()??;
                match (section.link, section.data) {
                    (link, SectionData::SymbolTable { table, .. }) => Some((link, table)),
                    _ => None,
                }
            });

        for (link, symtab) in symbol_tables {
            let index = u16::from(link) as usize;
            if index >= elf.section_number() {
                log::warn!("no strtab table corresponding to symtab");
            }
            let strtab = if let Ok(Some(section)) = elf.section(index) {
                if let SectionData::StringTable(table) = section.data {
                    table
                } else {
                    log::warn!("symtab linked to bad strtab {}", index);
                    continue;
                }
            } else {
                log::warn!("symtab linked to bad strtab {}", index);
                continue;
            };
            for i in 0..symtab.length() {
                let symbol = symtab.pick(i).map_err(|e| format!("{:?}", e))?;
                let name = strtab.pick(symbol.name as usize)
                    .map_err(|e| format!("{:?}", e))?
                    .to_string();
                symbols.push(Symbol {
                    code: symbol.value..(symbol.value + symbol.size),
                    name,
                })
            }
        }
        symbols.sort_by(|a, b| a.code.start.cmp(&b.code.start));

        Ok(symbols)
    }
}
