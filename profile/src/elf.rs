use {goblin::elf::Elf, memmap2::Mmap, std::path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugLevel {
    Dwarf,
    SymbolsOnly,
    Stripped,
}

pub struct Symbol {
    pub addr: u64,
    pub size: u64,
    pub name: String,
}

pub struct ElfInfo {
    pub text_offset: usize,
    pub text_size: usize,
    pub text_base_addr: u64,
    pub symbols: Vec<Symbol>,
    pub debug_level: DebugLevel,
}

const EM_BPF: u16 = 247;
const EM_SBF: u16 = 263;

pub fn load(mmap: &Mmap, path: &Path) -> ElfInfo {
    let elf = Elf::parse(mmap).unwrap_or_else(|e| {
        eprintln!("Error: failed to parse ELF file {}: {}", path.display(), e);
        std::process::exit(1);
    });

    if elf.header.e_machine != EM_BPF && elf.header.e_machine != EM_SBF {
        eprintln!(
            "Error: not an SBF binary (e_machine = {}, expected {} or {})",
            elf.header.e_machine, EM_BPF, EM_SBF,
        );
        std::process::exit(1);
    }

    let text_sh = elf
        .section_headers
        .iter()
        .find(|sh| elf.shdr_strtab.get_at(sh.sh_name) == Some(".text"))
        .unwrap_or_else(|| {
            eprintln!("Error: no .text section found in ELF");
            std::process::exit(1);
        });

    let text_offset = text_sh.sh_offset as usize;
    let text_size = text_sh.sh_size as usize;
    let text_base_addr = text_sh.sh_addr;

    if text_size == 0 {
        eprintln!("Error: .text section is empty");
        std::process::exit(1);
    }

    let has_debug_info = elf
        .section_headers
        .iter()
        .any(|sh| elf.shdr_strtab.get_at(sh.sh_name) == Some(".debug_info"));

    let has_symtab = elf
        .section_headers
        .iter()
        .any(|sh| sh.sh_type == goblin::elf::section_header::SHT_SYMTAB);

    let has_dynsym = !elf.dynsyms.is_empty();

    let debug_level = match (has_debug_info, has_symtab, has_dynsym) {
        (true, _, _) => DebugLevel::Dwarf,
        (false, true, _) => DebugLevel::SymbolsOnly,
        (false, false, true) => DebugLevel::SymbolsOnly,
        (false, false, false) => DebugLevel::Stripped,
    };

    // Collect from .symtab first, fall back to .dynsym
    let mut symbols: Vec<Symbol> = elf
        .syms
        .iter()
        .filter(|sym| sym.st_type() == goblin::elf::sym::STT_FUNC && sym.st_size > 0)
        .filter_map(|sym| {
            let name = elf.strtab.get_at(sym.st_name)?;
            let demangled = rustc_demangle::demangle(name).to_string();
            Some(Symbol {
                addr: sym.st_value,
                size: sym.st_size,
                name: demangled,
            })
        })
        .collect();

    if symbols.is_empty() {
        symbols = elf
            .dynsyms
            .iter()
            .filter(|sym| sym.st_type() == goblin::elf::sym::STT_FUNC && sym.st_size > 0)
            .filter_map(|sym| {
                let name = elf.dynstrtab.get_at(sym.st_name)?;
                let demangled = rustc_demangle::demangle(name).to_string();
                Some(Symbol {
                    addr: sym.st_value,
                    size: sym.st_size,
                    name: demangled,
                })
            })
            .collect();
    }

    symbols.sort_by_key(|s| s.addr);

    ElfInfo {
        text_offset,
        text_size,
        text_base_addr,
        symbols,
        debug_level,
    }
}
