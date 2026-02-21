mod aggregate;
mod dwarf;
mod elf;
mod output;
mod walk;

use std::path::PathBuf;

use elf::DebugLevel;
use memmap2::Mmap;

enum OutputMode {
    Svg,
    Folded,
    Text,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: quasar-profile <path-to-elf.so> [-o output.svg] [--folded] [--text]");
        std::process::exit(1);
    }

    let elf_path = PathBuf::from(&args[1]);
    let default_output = elf_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|name| format!("{}.profile.svg", name))
        .unwrap_or_else(|| "profile.svg".to_string());
    let mut output_path = PathBuf::from(default_output);
    let mut mode = OutputMode::Svg;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                output_path = PathBuf::from(
                    args.get(i).expect("-o requires an output path argument"),
                );
            }
            "--folded" => mode = OutputMode::Folded,
            "--text" => mode = OutputMode::Text,
            other => {
                eprintln!("Unknown option: {}", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    if !elf_path.exists() {
        eprintln!("Error: file not found: {}", elf_path.display());
        std::process::exit(1);
    }

    let file = std::fs::File::open(&elf_path).unwrap_or_else(|e| {
        eprintln!("Error: failed to open {}: {}", elf_path.display(), e);
        std::process::exit(1);
    });

    let mmap = unsafe { Mmap::map(&file) }.unwrap_or_else(|e| {
        eprintln!("Error: failed to mmap {}: {}", elf_path.display(), e);
        std::process::exit(1);
    });

    let info = elf::load(&mmap, &elf_path);

    eprintln!("quasar-profile: {}", elf_path.display());

    let resolver = match info.debug_level {
        DebugLevel::Dwarf => {
            eprintln!("DWARF debug info: yes");
            dwarf::Resolver::Dwarf(
                dwarf::DwarfResolver::new(&mmap),
                dwarf::SymbolResolver::new(&info.symbols),
            )
        }
        DebugLevel::SymbolsOnly => {
            eprintln!("DWARF debug info: no (symbol table only)");
            eprintln!(
                "Warning: inline functions will not be resolved. \
                 Rebuild with debug info for full resolution."
            );
            dwarf::Resolver::Symbol(dwarf::SymbolResolver::new(&info.symbols))
        }
        DebugLevel::Stripped => {
            eprintln!(
                "Error: binary is fully stripped. Use the unstripped binary from \
                 target/sbf-solana-solana/release/ instead of target/deploy/"
            );
            std::process::exit(1);
        }
    };

    let program_name = elf_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let result = aggregate::profile(&mmap, &info, &resolver);

    output::print_summary(&result);

    match mode {
        OutputMode::Svg => {
            output::write_svg(&result.folded_stacks, &output_path, program_name);
            eprintln!("Flame graph written to: {}", output_path.display());
        }
        OutputMode::Folded => {
            print!("{}", result.folded_stacks);
        }
        OutputMode::Text => {
            // Summary already printed above
        }
    }
}
