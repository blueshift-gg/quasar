//! Where a program's own compute units go: every instruction the escrow's
//! `make`, `take` and `refund` execute, counted by the function it was
//! compiled from. `make profile` builds the escrow with debug information,
//! which does not change its code, runs it under p-token with the runtime
//! recording each instruction, and prints the functions that cost most.
//!
//! A compute unit is one instruction, so the counts are the program's share
//! of the benchmark's ledger less its syscalls, which the runtime charges
//! apart.

#[allow(dead_code)]
#[path = "../escrow_bench.rs"]
mod escrow_bench;

use std::{collections::HashMap, path::Path};

use object::{FileFlags, Object, ObjectSection};
use quasar_test::{Test, target_dir};

/// The build `make profile` makes: under `target/profile`, with debug information.
const BUILD: &str = "profile";
/// The functions printed for each instruction.
const MOST: usize = 25;

fn main() {
    let traces = target_dir().join(BUILD).join("trace");
    // SAFETY: nothing else runs yet. The runtime reads the variable as each
    // test is made.
    unsafe { std::env::set_var("SBF_TRACE_DIR", &traces) };
    let build_dir = target_dir().join(BUILD);
    let deployed = std::fs::read(build_dir.join("deploy/escrow.so"))
        .expect("the deployed escrow ELF; run `make profile`");
    // Read the running ELF's architecture so an old build cannot supply its symbols.
    let target = match object::File::parse(&*deployed).expect("an ELF").flags() {
        FileFlags::Elf { e_flags: 0, .. } => "sbpf-solana-solana".to_owned(),
        FileFlags::Elf { e_flags, .. } => format!("sbpfv{e_flags}-solana-solana"),
        flags => panic!("unsupported SBF ELF flags: {flags:?}"),
    };
    let elf = build_dir.join(target).join("release/escrow.so");
    let symbols = Symbols::new(&elf);
    let p_token = Test::p_token_elf();
    let build = || escrow_bench::prepare_on(Test::built_in(BUILD, escrow::ID, "escrow"), &p_token);

    let mut t = build();
    let make =
        profile(&traces, &symbols, || t.run(escrow_bench::make()).succeeds().compute_units());
    let take =
        profile(&traces, &symbols, || t.run(escrow_bench::take()).succeeds().compute_units());
    let mut t = build();
    profile(&traces, &symbols, || t.run(escrow_bench::make()).succeeds().compute_units());
    let refund =
        profile(&traces, &symbols, || t.run(escrow_bench::refund()).succeeds().compute_units());
    for (name, (total, counts)) in [("make", make), ("take", take), ("refund", refund)] {
        let own: u64 = counts.values().sum();
        println!("\n## {name}: {total} CU, {own} of them the program's own instructions\n");
        println!("| function | instructions | of its own |");
        println!("|---|---:|---:|");
        let mut counts: Vec<_> = counts.into_iter().collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        for (function, count) in counts.into_iter().take(MOST) {
            println!("| `{function}` | {count} | {:.1}% |", 100.0 * count as f64 / own as f64);
        }
    }
}

/// Runs `instruction`, and counts the escrow's instructions it recorded by
/// function; with the compute units it reports.
fn profile(
    traces: &Path,
    symbols: &Symbols,
    instruction: impl FnOnce() -> u64,
) -> (u64, HashMap<String, u64>) {
    let _ = std::fs::remove_dir_all(traces);
    let total = instruction();
    let mut counts = HashMap::new();
    let own = escrow::ID.to_string();
    for entry in std::fs::read_dir(traces).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "regs") {
            continue;
        }
        let program =
            std::fs::read_to_string(path.with_extension("program_id")).unwrap_or_default();
        if program != own {
            continue;
        }
        // Twelve registers an instruction, the last the program counter.
        let registers = std::fs::read(&path).expect("a trace");
        for step in registers.chunks_exact(12 * 8) {
            let pc = u64::from_le_bytes(step[11 * 8..].try_into().expect("eight bytes"));
            *counts.entry(symbols.function(pc)).or_default() += 1;
        }
    }
    (total, counts)
}

/// The program's debug information: which function each instruction is from.
struct Symbols {
    lines: addr2line::Loader,
    text: u64,
}

impl Symbols {
    fn new(elf: &Path) -> Self {
        let bytes = std::fs::read(elf)
            .unwrap_or_else(|error| panic!("{}: {error}; run `make profile`", elf.display()));
        let file = object::File::parse(&*bytes).expect("an ELF");
        let text = file.section_by_name(".text").expect("a text section").address();
        Self { lines: addr2line::Loader::new(elf).expect("debug information"), text }
    }

    /// The function the instruction at `pc` was written in: the innermost of
    /// those inlined there, without its hash and type parameters.
    fn function(&self, pc: u64) -> String {
        let mut frames = match self.lines.find_frames(self.text + pc * 8) {
            Ok(frames) => frames,
            Err(_) => return "?".to_owned(),
        };
        let name = match frames.next() {
            Ok(Some(frame)) => {
                frame.function.and_then(|name| name.demangle().ok().map(String::from))
            }
            _ => None,
        };
        let mut name = name.unwrap_or_else(|| "?".to_owned());
        // `path::function::h0123..`: the hash says nothing here.
        if let Some(at) = name.rfind("::h") {
            name.truncate(at);
        }
        let mut plain = String::new();
        let mut depth = 0;
        for character in name.chars() {
            match character {
                '<' => depth += 1,
                '>' => depth -= 1,
                _ if depth == 0 => plain.push(character),
                _ => {}
            }
        }
        plain.replace("::::", "::")
    }
}
