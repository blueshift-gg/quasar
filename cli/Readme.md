# CLI

The `quasar` binary is a project-aware build tool for Solana programs. It wraps `cargo build-sbf` / `cargo build-bpf`, runs tests, generates IDLs, profiles compute-unit usage, and dumps sBPF assembly.

## Install

```bash
cargo install --path cli --locked
```

## Quasar 0.2 compatibility

New projects use the typed-core API, explicit instruction discriminators and
published ZeroPod 0.4.0 through `quasar-lang`. The Solana build uses
platform-tools v1.56 and the v3 architecture by default, matching the repository
gate. The optional upstream build uses `cargo +nightly`, `rust-src`, and
`sbpf-linker`; its target config selects the runtime's on-chain implementation
and SBPFv3 output.

Rust scaffolds use Mollusk. TypeScript scaffolds use LiteSVM with the public
Kit or Web3.js SDK. Both run SBPFv3 programs. The legacy
`--rust-framework quasar-svm` spelling is accepted as an alias for `mollusk`.

`quasar add --instruction settle` allocates an unused discriminator of the
existing width. It preserves existing instruction numbers. State and error
modules added to a minimal project are included in the crate automatically.

`quasar idl` runs the host-only emission test and adapts `idl().to_json()`
to the public client schema. `quasar client` also accepts that runtime JSON
directly, alongside existing complete schema IDLs. Rust clients are standalone
and use the runtime's Borsh-compatible encoding. Fixed-capacity `ArrayString`
and `ArrayVec` values expose their wire representation as `length` plus
`storage`; unused slots must be padded.

The runtime IDL currently describes instructions, account flags and account
fields. It does not describe PDA resolvers, events, errors or custom nested
type definitions. The adapter does not invent that metadata: callers supply
account addresses, and an unknown field encoding produces an explicit error.
Linting still checks the reported interface and compares `quasar.lock.json`;
its scope is the metadata present in the IDL.

Before 0.2.0 is published, replace the generated `quasar-lang` dependency with
a path to this checkout's `lang` directory. No local CLI installation publishes
crates or deploys a program.

## Commands

### `quasar init [name] [--yes] [--no-git] [--verbose]`

Scaffold a new Quasar project. Without `--yes`, this launches an interactive wizard. If a name is provided, it pre-fills the project name prompt. With `--yes`, the command uses saved defaults or explicit flags and requires a project name.

- **Project name**: becomes the crate name and `Quasar.toml` project name
- **Toolchain**: `solana` (cargo build-sbf) or `upstream` (cargo +nightly build-bpf)
- **Testing framework**: None, Mollusk/Rust, LiteSVM/Web3.js, or LiteSVM/Kit
- **Template**: Minimal (single instruction) or Full (state, events, instruction files)
- **Git setup**: Initialize + Commit, Initialize, or Skip

The wizard generates a complete project directory with `Cargo.toml`, `Quasar.toml`, source files, test scaffolding, a program keypair, and optional git setup. Preferences are saved to `~/.quasar/config.toml` and used as defaults for future runs.

| Flag | Effect |
|------|--------|
| `-y, --yes` | Explicitly skip prompts and use saved defaults |
| `--no-git` | Skip git repo setup |
| `--test-language` | Set `none`, `rust`, or `typescript` |
| `--rust-framework` | Set `mollusk` for Rust tests |
| `--ts-sdk` | Set `kit` or `web3.js` for TypeScript tests |
| `--template` | Set `minimal` or `full` |
| `--toolchain` | Set `solana` or `upstream` |
| `--verbose` | Print each scaffold step as it runs |

```bash
quasar init                  # Interactive wizard
quasar init my-program       # Interactive, with the name pre-filled
quasar init my-program -y    # Use saved defaults, no prompts
quasar init .                # Scaffold into current directory
quasar init my-program -y --verbose
```

### `quasar build [--debug] [--verbose] [--watch] [--features FEATURES]`

Compile the on-chain program. Reads `Quasar.toml` to determine which toolchain to use and automatically generates the IDL before building. By default, Quasar shows phase progress and keeps child-process output quiet unless the build fails. With `--verbose`, it streams the underlying build command output directly.

| Flag | Effect |
|------|--------|
| `--debug` | Emit debug symbols (required for profiling and source-interleaved dump) |
| `--verbose` | Stream the underlying build command output directly |
| `--watch` | Watch `src/` for changes and rebuild automatically |
| `--features FEATURES` | Cargo features to enable (passed through to the build command) |

On success, prints the binary size and delta from the previous build:

```
  [ok] Build complete in 1.2s (56.6 KB, -1.2 KB)
```

### `quasar test [--debug] [--show-output] [--filter PATTERN] [--watch] [--no-build] [--features FEATURES] [--verbose]`

Run the test suite. Builds first, then runs either Rust tests or TypeScript tests based on `Quasar.toml`. By default, Quasar prints the active phase and configured test command. With `--verbose`, the build step streams its underlying command output and test/install commands are echoed before execution.

| Flag | Effect |
|------|--------|
| `--debug` | Build with debug symbols before testing |
| `--show-output` | Forward `--show-output` to `cargo test` |
| `-f, --filter PATTERN` | Only run tests matching the pattern |
| `-w, --watch` | Watch `src/` for changes and re-run tests automatically |
| `--no-build` | Skip the build step (use existing binary) |
| `--features FEATURES` | Cargo features to enable for the build step |
| `--verbose` | Show build/test commands as they run |

TypeScript tests run through the configured command, which defaults to `npx vitest run`. Rust tests run through the configured command, which defaults to `cargo test tests::`.

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

Remove build artifacts while preserving program keypairs from `target/deploy/`, `target/profile/`, `target/idl/`, and the configured `clients.path` directory from `Quasar.toml`.

### `quasar idl <path>`

Generate the IDL for a program crate. Produces JSON, TypeScript client, and Rust client module. No `idl-build` feature is needed: `#[program]` emits a host-only IDL test.

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
type = "solana"
# Optional overrides for build, test, and profile:
tools_version = "v1.56"
arch = "v3"

[testing]
language = "rust"

[testing.rust]
framework = "mollusk"
test = { program = "cargo", args = ["test", "tests::"] }

[clients]
path = "target/client"
languages = ["rust"]
```

`tools_version` overrides the platform-tools version passed to `cargo build-sbf`.
If omitted, it defaults to v1.56. `arch` defaults to `"v3"`. Set `arch = "v0"`
when using a test runtime that requires SBPFv0. These settings apply to
normal, debug, watch, test and profiling builds using the Solana toolchain;
the upstream toolchain does not use them. The selected compiler validates the
version/architecture combination. See the
[Anza migration guide](https://github.com/anza-xyz/cargo-build-sbf#sbfpv3-migration).

### Global config (`~/.quasar/config.toml`)

Preferences that apply across all projects:

```toml
[defaults]
toolchain = "solana"
test_language = "rust"
rust_framework = "mollusk"
ts_sdk = "kit"
template = "minimal"
git = "commit"      # "commit", "init", or "skip"
package_manager = "pnpm"

[ui]
animation = true   # Banner on `quasar init`
color = true       # Colored terminal output
```
