
use std::{ops::Range, str::FromStr, io::{self, Read}, fs::File, path::PathBuf};

pub struct ProcessMap(Vec<MemoryMapEntry>);

impl ProcessMap {
    pub fn new(pid: u32) -> io::Result<Self> {
        MemoryMapEntry::new(pid).map(ProcessMap)
    }

    pub fn find(&self, ip: usize) -> Option<(PathBuf, usize)> {
        self.0.iter()
            .find_map(|entry| {
                if !entry.range.contains(&ip) {
                    return None;
                }
                let filename = entry.relevant()?;

                if !entry.exec() {
                    log::warn!("have non exec pointer in stacktrace {:016x}@{:?}", ip, filename);
                }

                Some((filename, entry.offset + ip - entry.range.start))
            })
    }
}

struct MemoryMapEntry {
    range: Range<usize>,
    flags: String,
    offset: usize,
    name: EntryName,
}

enum EntryName {
    Nothing,
    FileName(PathBuf),
    Remark(String),
}

impl MemoryMapEntry {
    fn new(pid: u32) -> Result<Vec<Self>, io::Error> {
        let mut entries = String::new();
        File::open(&format!("/proc/{}/maps", pid))?
            .read_to_string(&mut entries)?;

        let mut map = vec![];
        for line in entries.lines() {
            map.push(line.parse()?);
        }
        Ok(map)
    }

    fn exec(&self) -> bool {
        self.flags.contains('x')
    }

    fn relevant(&self) -> Option<PathBuf> {
        match &self.name {
            &EntryName::FileName(ref filename) => Some(filename.clone()),
            _ => None,
        }
    }
}

impl FromStr for MemoryMapEntry {
    // TODO: proper error
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut columns = s.split_ascii_whitespace();

        let range_str = columns.next().ok_or(io::ErrorKind::Other)?;
        let range = {
            let mut range_items = range_str.split('-');
            let range_start = range_items.next().ok_or(io::ErrorKind::Other)?;
            let range_end = range_items.next().ok_or(io::ErrorKind::Other)?;
            let start = usize::from_str_radix(range_start, 16)
                .map_err(|_| io::ErrorKind::Other)?;
            let end = usize::from_str_radix(range_end, 16)
                .map_err(|_| io::ErrorKind::Other)?;
            start..end
        };

        let flags = columns.next().ok_or(io::ErrorKind::Other)?.to_string();

        let offset_str = columns.next().ok_or(io::ErrorKind::Other)?;
        let offset = usize::from_str_radix(offset_str, 16)
            .map_err(|_| io::ErrorKind::Other)?;

        let _ = columns.next().ok_or(io::ErrorKind::Other)?;
        let _ = columns.next().ok_or(io::ErrorKind::Other)?;

        let name = match columns.next() {
            None => EntryName::Nothing,
            Some(name) => {
                if name.is_empty() {
                    EntryName::Nothing
                } else {
                    match PathBuf::from_str(name) {
                        Ok(path) => EntryName::FileName(path),
                        Err(_) => EntryName::Remark(name.to_string()),
                    }
                }
            },
        };

        Ok(MemoryMapEntry { range, flags, offset, name })
    }
}
