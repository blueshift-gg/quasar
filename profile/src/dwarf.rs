use {
    crate::elf::Symbol,
    addr2line::{gimli, LookupResult},
    object::{Object, ObjectSection},
};

pub enum Resolver<'a> {
    Dwarf(DwarfResolver<'a>, SymbolResolver),
    Symbol(SymbolResolver),
}

impl Resolver<'_> {
    pub fn resolve(&self, addr: u64) -> Vec<String> {
        match self {
            Resolver::Dwarf(dwarf, sym_fallback) => {
                let stack = dwarf.resolve(addr);
                if stack.first().map(|s| s.as_str()) == Some("[unknown]") {
                    sym_fallback.resolve(addr)
                } else {
                    stack
                }
            }
            Resolver::Symbol(r) => r.resolve(addr),
        }
    }
}

// --- DWARF resolver (full inline frame support) ---

pub struct DwarfResolver<'a> {
    ctx: addr2line::Context<gimli::EndianSlice<'a, gimli::RunTimeEndian>>,
}

impl<'a> DwarfResolver<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        let obj = object::File::parse(data).expect("failed to parse object file for DWARF");

        let endian = if obj.is_little_endian() {
            gimli::RunTimeEndian::Little
        } else {
            gimli::RunTimeEndian::Big
        };

        let dwarf = gimli::Dwarf::load(|section_id| -> Result<_, gimli::Error> {
            Ok(match obj.section_by_name(section_id.name()) {
                Some(section) => {
                    let data = section.data().unwrap_or(&[]);
                    gimli::EndianSlice::new(data, endian)
                }
                None => gimli::EndianSlice::new(&[], endian),
            })
        })
        .expect("failed to load DWARF sections");

        let ctx =
            addr2line::Context::from_dwarf(dwarf).expect("failed to create addr2line context");

        Self { ctx }
    }

    pub fn resolve(&self, addr: u64) -> Vec<String> {
        let frames_result = match self.ctx.find_frames(addr) {
            LookupResult::Output(result) => result,
            LookupResult::Load { .. } => return vec!["[unknown]".into()],
        };

        let mut frames_iter = match frames_result {
            Ok(iter) => iter,
            Err(_) => return vec!["[unknown]".into()],
        };

        let mut stack = Vec::new();
        loop {
            match frames_iter.next() {
                Ok(Some(frame)) => {
                    let name = match &frame.function {
                        Some(f) => match f.demangle() {
                            Ok(cow) => cow.into_owned(),
                            Err(_) => f
                                .raw_name()
                                .map(|n| n.to_string())
                                .unwrap_or_else(|_| "[unknown]".into()),
                        },
                        None => "[unknown]".into(),
                    };
                    stack.push(name);
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }

        if stack.is_empty() {
            vec!["[unknown]".into()]
        } else {
            stack
        }
    }
}

// --- Symbol-only resolver (fallback, no inline info) ---

pub struct SymbolResolver {
    /// Sorted by address
    symbols: Vec<(u64, u64, String)>,
}

impl SymbolResolver {
    pub fn new(symbols: &[Symbol]) -> Self {
        let entries = symbols
            .iter()
            .map(|s| (s.addr, s.size, s.name.clone()))
            .collect();
        Self { symbols: entries }
    }

    pub fn resolve(&self, addr: u64) -> Vec<String> {
        let idx = self.symbols.partition_point(|&(start, _, _)| start <= addr);

        if idx > 0 {
            let (start, size, ref name) = self.symbols[idx - 1];
            if addr < start + size {
                return vec![name.clone()];
            }
        }

        vec!["[unknown]".into()]
    }
}
