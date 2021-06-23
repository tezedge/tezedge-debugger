use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock, atomic::{AtomicU32, Ordering}}, 
    path::PathBuf,
};
use bpf_memprof::Hex32;
use serde::Serialize;
use super::{memory_map::ProcessMap, table::SymbolTable};

#[derive(Serialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
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
    mock: Option<()>,
}

fn copy_binary(filename: &str) -> Result<PathBuf, ()> {
    use std::{env, process::Command, path::Path, fs};

    let node_name = match env::var("TEZEDGE_NODE_NAME") {
        Err(_) => return Ok(PathBuf::from(filename)),
        Ok(v) => v,
    };

    let res = Command::new("docker")
        .args(&["ps", "-qf"])
        .arg(format!("name={}", node_name))
        .output()
        .map_err(|_| ())?;
    if !res.status.success() {
        return Err(());
    }
    let node_image_name = String::from_utf8(res.stdout).map_err(|_| ())?;
    if node_image_name.lines().count() > 1 {
        log::error!("multiple node containers");
        Err(())
    } else {
        log::info!("copying: {}", filename);
        let path = format!("/tmp{}", filename);
        let path = Path::new(&path);
        let prefix = path.parent().ok_or(())?;
        let _ = fs::create_dir_all(prefix);
        let output = Command::new("docker").arg("cp")
            .arg(format!("{}:{}", node_image_name.trim_end_matches('\n'), filename))
            .arg(path)
            .output()
            .map_err(|_| ())?;
        if !output.status.success() {
            log::error!("{:?}", String::from_utf8(output.stderr));
        }
        Ok(path.to_path_buf())
    }
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
                                for initial_filename in map.files() {
                                    if !files.contains(&initial_filename) {
                                        let filename = match copy_binary(&initial_filename) {
                                            Err(()) => {
                                                log::error!("failed to copy fresh binary {:?} from tezedge container", initial_filename);
                                                continue;
                                            },
                                            Ok(filename) => filename,
                                        };
                                        log::info!("try load symbols for: {}", initial_filename);
                                        match SymbolTable::load(&filename) {
                                            Ok(table) => {
                                                log::info!("loaded {} symbols from: {}", table.len(), initial_filename);
                                                let mut guard = resolver_ref.write().unwrap();
                                                guard.files.insert(initial_filename.clone(), table);
                                                drop(guard);
                                            },
                                            Err(error) => {
                                                log::info!(
                                                    "failed to load symbols for: {:?}, {}",
                                                    filename,
                                                    error,
                                                );
                                            }
                                        }
                                        files.insert(initial_filename);
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

    pub fn mock() -> Self {
        StackResolver {
            files: HashMap::new(),
            map: None,
            mock: Some(()),
        }
    }

    fn try_resolve(&self, address: u64) -> Option<((usize, &str), Option<String>)> {
        let map = self.map.as_ref()?;
        let (filename, offset) = map.find(address as usize)?;
        let table = self.files.get(&filename)?;
        Some(((offset, table.name()), table.find(offset as u64)))
    }

    fn try_mock(&self, address: u64) -> Option<((usize, &str), Option<String>)> {
        self.mock.as_ref().map(|&()| ((0, "mock"), Some(format!("func_{}", address))))
    }

    pub fn resolve(&self, address: u64) -> Option<SymbolInfo> {
        let ((offset, filename), name) = self
            .try_resolve(address)
            .or_else(|| self.try_mock(address))?;

        fn cpp_demangle(s: &str) -> Option<String> {
            cpp_demangle::Symbol::new(s).ok()?.demangle(&Default::default()).ok()
        }

        let function_category = if filename == "light-node" {
            if name.as_ref().map(|n| is_rust(n)).unwrap_or(false) {
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
                    if is_rust(&n) {
                        rustc_demangle::demangle(&n).to_string()
                    } else {
                        cpp_demangle(&n).unwrap_or(n)
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
