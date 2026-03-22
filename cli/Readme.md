# CLI

The `quasar` binary is a project-aware build tool for Solana programs. It wraps `cargo build-sbf` / `cargo build-bpf`, runs tests, generates IDLs, profiles compute-unit usage, and dumps sBPF assembly.

## Install

```bash
cargo install --path cli
```

## Commands

### `quasar init [name] [--yes] [--no-git]`

Scaffold a new Quasar project. When a name is provided, uses saved defaults and skips prompts. Without a name, launches an interactive wizard that prompts for:

- **Project name** — becomes the crate name and `Quasar.toml` project name
- **Toolchain** — `solana` (cargo build-sbf) or `upstream` (cargo +nightly build-bpf)
- **Testing framework** — None, Mollusk, QuasarSVM/Rust, QuasarSVM/Web3.js, or QuasarSVM/Kit
- **Template** — Minimal (single instruction) or Full (state, events, instruction files)
- **Git setup** — Initialize + Commit, Initialize, or Skip

The wizard generates a complete project directory with `Cargo.toml`, `Quasar.toml`, source files, test scaffolding, a program keypair, and optional git setup. Preferences are saved to `~/.quasar/config.toml` and used as defaults for future runs.

| Flag | Effect |
|------|--------|
| `-y, --yes` | Explicitly skip prompts (same as providing a name) |
| `--no-git` | Skip git repo setup |

```bash
quasar init                  # Interactive wizard
quasar init my-program       # Use saved defaults, no prompts
quasar init .                # Scaffold into current directory
```

### `quasar build [--debug] [--watch] [--features FEATURES]`

Compile the on-chain program. Reads `Quasar.toml` to determine which toolchain to use and automatically generates the IDL before building.

| Flag | Effect |
|------|--------|
| `--debug` | Emit debug symbols (required for profiling and source-interleaved dump) |
| `--watch` | Watch `src/` for changes and rebuild automatically |
| `--features FEATURES` | Cargo features to enable (passed through to the build command) |

On success, prints the binary size and delta from the previous build:

```
  ✔ Build complete in 1.2s (56.6 KB, -1.2 KB)
```

### `quasar test [--debug] [--filter PATTERN] [--watch]`

Run the test suite. Builds first, then runs either Rust tests (Mollusk/QuasarSVM) or TypeScript tests (Mocha) based on the `Quasar.toml` testing framework.

| Flag | Effect |
|------|--------|
| `--debug` | Build with debug symbols before testing |
| `-f, --filter PATTERN` | Only run tests matching the pattern |
| `-w, --watch` | Watch `src/` for changes and re-run tests automatically |
| `--no-build` | Skip the build step (use existing binary) |

TypeScript tests are parsed from Mocha's JSON reporter for structured pass/fail output. Rust tests are parsed from `cargo test` output.

### `quasar profile [elf] [--expand] [--diff PROGRAM] [--share] [--watch]`

Measure compute-unit usage by statically walking the sBPF binary's call graph. If no ELF path is given, runs a debug build automatically.

| Flag | Effect |
|------|--------|
| `--expand` | Show all functions with bar charts and per-function deltas |
| `--diff PROGRAM` | Compare against an on-chain program (starts a blocking server) |
| `--share` | Upload the profile as a public GitHub Gist |
| `-w, --watch` | Watch `src/` for changes and re-profile automatically |

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

### `quasar deploy [--program-keypair KEYPAIR] [--upgrade-authority KEYPAIR]`

Deploy the program to a Solana cluster using the Solana CLI. Uses the cluster and wallet from your Solana CLI config (`solana config get`).

The program binary is auto-detected from `target/deploy/`. The program keypair defaults to `target/deploy/<name>-keypair.json` (generated during `quasar build`).

| Flag | Effect |
|------|--------|
| `--program-keypair KEYPAIR` | Path to the program keypair (overrides auto-detection) |
| `--upgrade-authority KEYPAIR` | Upgrade authority keypair (defaults to Solana CLI default) |

```bash
quasar build && quasar deploy          # Build and deploy
quasar deploy                          # Deploy existing binary
quasar deploy --program-keypair ./my-key.json
```

### `quasar clean`

Remove build artifacts from `target/deploy/`, `target/profile/`, `target/idl/`, and `target/client/`.

### `quasar idl <path>`

Generate the IDL for a program crate. Produces JSON, TypeScript client, and Rust client module. See [IDL docs](idl.md) for details.

### `quasar completions <shell>`

Generate shell completions for your shell. Supported shells: `bash`, `zsh`, `fish`, `elvish`, `powershell`.

```bash
# Bash
quasar completions bash >> ~/.bashrc

# Zsh
quasar completions zsh >> ~/.zshrc

# Fish
quasar completions fish > ~/.config/fish/completions/quasar.fish
```

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
framework = "mollusk"  # "none", "mollusk", "quasarsvm-rust", "quasarsvm-web3js", or "quasarsvm-kit"
```

### Global config (`~/.quasar/config.toml`)

Preferences that apply across all projects:

```toml
[defaults]
toolchain = "solana"
framework = "mollusk"
template = "minimal"
git = "commit"      # "commit", "init", or "skip"

[ui]
animation = true   # Animated banner on `quasar init`
color = true       # Colored terminal output
```
