use std::{ops::Range, path::Path};

pub struct SymbolTable {
    inner: Vec<Symbol>,
    name: String,
    strings: Vec<u8>,
}

struct Symbol {
    range: u64, // 5 bytes offset, 3 bytes length
    name_offset: u32,
}

impl Symbol {
    fn code(&self) -> Range<u64> {
        (self.range >> 24)..(self.range & !((1 << 24) - 1))
    }
}

impl SymbolTable {
    pub fn load<P>(path: P) -> Result<Self, String>
    where
        P: AsRef<Path>,
    {
        use std::{fs, io::Read};
        use elf64::{Elf64, SectionData};

        let mut f = fs::File::open(&path).map_err(|e| e.to_string())?;
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
                    (link, SectionData::DynamicSymbolTable { table, .. }) => Some((link, table)),
                    _ => None,
                }
            });

        let mut strings = Vec::new();
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
                let range = (symbol.value << 24) + symbol.size;
                symbols.push(Symbol { range, name_offset: (strings.len() as u32) + symbol.name })
            }
            strings.extend_from_slice(strtab.as_raw());
        }
        symbols.sort_by(|a, b| a.range.cmp(&b.range));

        Ok(SymbolTable {
            inner: symbols,
            name: path.as_ref().file_name().and_then(|n| n.to_str()).unwrap_or("").to_string(),
            strings,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn find(&self, offset: u64) -> Option<String> {
        if self.is_empty() {
            return None;
        }

        let mut length = 1 << (63 - (self.len() as u64).leading_zeros() as usize);
        // pos points somewhere in the middle of symbols array, and is power of two
        // if 4 <= symbols.len() < 8 => pos == 4
        // do binary search in symbols
        let mut pos = length;
        while length > 0 {
            length >>= 1;
            if pos >= self.len() {
                pos -= length;
            } else {
                let symbol = &self.inner[pos];
                let code = symbol.code();
                if code.contains(&offset) {
                    let strtab = elf64::StringTable::new(&self.strings);
                    return match strtab.pick(symbol.name_offset as usize) {
                        Ok(name) => Some(name.to_string()),
                        Err(e) => Some(format!("\"string table error {:?}\"", e)),
                    };
                } else if code.start > offset {
                    pos -= length;
                } else {
                    pos += length;
                }
            }
        }
        None
    }
}
