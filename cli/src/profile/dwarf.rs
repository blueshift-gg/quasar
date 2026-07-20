//! DWARF and symbol resolution for the CLI profiler.

use {
    super::elf::Symbol,
    addr2line::{gimli, LookupResult},
    object::{Object, ObjectSection},
};

pub(crate) enum Resolver<'a> {
    Dwarf(DwarfResolver<'a>, SymbolResolver),
    Symbol(SymbolResolver),
}

impl Resolver<'_> {
    pub(crate) fn resolve(&self, addr: u64) -> Vec<String> {
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

pub(crate) struct DwarfResolver<'a> {
    ctx: addr2line::Context<gimli::EndianSlice<'a, gimli::RunTimeEndian>>,
}

impl<'a> DwarfResolver<'a> {
    pub(crate) fn try_new(data: &'a [u8]) -> Option<Self> {
        let obj = object::File::parse(data).ok()?;

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
        .ok()?;

        let ctx = addr2line::Context::from_dwarf(dwarf).ok()?;

        Some(Self { ctx })
    }

    pub(crate) fn resolve(&self, addr: u64) -> Vec<String> {
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

pub(crate) struct SymbolResolver {
    /// Sorted by address
    symbols: Vec<(u64, u64, String)>,
}

impl SymbolResolver {
    pub(crate) fn new(symbols: &[Symbol]) -> Self {
        let entries = symbols
            .iter()
            .map(|s| (s.addr, s.size, s.name.clone()))
            .collect();
        Self { symbols: entries }
    }

    pub(crate) fn resolve(&self, addr: u64) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::DwarfResolver;

    #[test]
    fn malformed_dwarf_input_returns_none() {
        assert!(DwarfResolver::try_new(b"not an elf").is_none());
    }
}
