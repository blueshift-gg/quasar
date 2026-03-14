# CLI

The `quasar` binary is a project-aware build tool for Solana programs. It wraps `cargo build-sbf` / `cargo build-bpf`, runs tests, generates IDLs, profiles compute-unit usage, and dumps sBPF assembly.

## Install

```bash
cargo install --path cli
```

## Commands

### `quasar init [name]`

Scaffold a new Quasar project. Launches an interactive wizard that prompts for:

- **Project name** — becomes the crate name and `Quasar.toml` project name
- **Toolchain** — `solana` (cargo build-sbf) or `upstream` (cargo +nightly build-bpf)
- **Testing framework** — None, Mollusk, QuasarSVM/Web3.js, or QuasarSVM/Kit
- **Template** — Minimal (single instruction) or Full (state, events, instruction files)

The wizard generates a complete project directory with `Cargo.toml`, `Quasar.toml`, source files, test scaffolding, and a program keypair. Preferences are saved to `~/.quasar/config.toml` so subsequent `init` runs use the same defaults.

```bash
quasar init my-program
```

### `quasar build [--debug] [--watch]`

Compile the on-chain program. Reads `Quasar.toml` to determine which toolchain to use and automatically generates the IDL before building.

| Flag | Effect |
|------|--------|
| `--debug` | Emit debug symbols (required for profiling and source-interleaved dump) |
| `--watch` | Watch `src/` for changes and rebuild automatically |

On success, prints the binary size and delta from the previous build:

```
  ✔ Build complete in 1.2s (56.6 KB, -1.2 KB)
```

### `quasar test [--debug] [--filter PATTERN]`

Run the test suite. Builds first, then runs either Rust tests (Mollusk) or TypeScript tests (Mocha) based on the `Quasar.toml` testing framework.

| Flag | Effect |
|------|--------|
| `--debug` | Build with debug symbols before testing |
| `--filter PATTERN` | Only run tests matching the pattern |

TypeScript tests are parsed from Mocha's JSON reporter for structured pass/fail output. Rust tests are parsed from `cargo test` output.

### `quasar profile [elf] [--expand] [--diff PROGRAM] [--share]`

Measure compute-unit usage by statically walking the sBPF binary's call graph. If no ELF path is given, runs a debug build automatically.

| Flag | Effect |
|------|--------|
| `--expand` | Show all functions with bar charts and per-function deltas |
| `--diff PROGRAM` | Compare against an on-chain program (starts a blocking server) |
| `--share` | Upload the profile as a public GitHub Gist |

The profiler tracks results between runs. On the first run, it shows the top 5 hottest functions. On subsequent runs, it shows the biggest regressions and improvements by magnitude:

```
  my_program  12,345 CU (+42)
     8,000 (+200)   5.0%  Initialize::verify
     4,000 (-158)   3.0%  Deposit::process

  flamegraph  http://127.0.0.1:7777/?program=my_program
```

A background HTTP server starts automatically to serve the interactive flamegraph viewer. The server shuts itself down after 30 seconds of inactivity.

### `quasar dump [elf] [--function SYMBOL] [--source]`

Dump sBPF assembly using `llvm-objdump` from Solana platform-tools. If no ELF path is given, auto-detects from `target/deploy/` or `target/profile/`.

| Flag | Effect |
|------|--------|
| `-f, --function SYMBOL` | Disassemble only the named symbol (demangled) |
| `-S, --source` | Interleave source code (requires a debug build) |

```bash
# Full disassembly
quasar dump

# Single function with source
quasar dump --function initialize -S
```

Prints an instruction count summary at the end.

### `quasar clean`

Remove build artifacts from `target/deploy/`, `target/profile/`, `target/idl/`, and `target/client/`.

### `quasar idl <path>`

Generate the IDL for a program crate. Produces JSON, TypeScript client, and Rust client module. See [IDL docs](idl.md) for details.

### `quasar config [get|set|list|reset]`

Manage global settings stored in `~/.quasar/config.toml`.

```bash
quasar config list              # Print all settings
quasar config get ui.animation  # Read a value
quasar config set ui.color false # Write a value
quasar config reset             # Restore factory defaults
```

## Configuration

### Project config (`Quasar.toml`)

Every project has a `Quasar.toml` at the root:

```toml
[project]
name = "my-program"

[toolchain]
type = "solana"        # "solana" or "upstream"

[testing]
framework = "mollusk"  # "none", "mollusk", "quasarsvm-web3js", or "quasarsvm-kit"
```

### Global config (`~/.quasar/config.toml`)

Preferences that apply across all projects:

```toml
[defaults]
toolchain = "solana"
framework = "mollusk"
template = "minimal"

[ui]
animation = true   # Animated banner on `quasar init`
color = true       # Colored terminal output
timing = true      # Show build timing
```
