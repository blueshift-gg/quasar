# Quasar CLI

The `quasar` executable owns the supported program journey:

```text
install → init → build → test → deploy → verify → debug
```

## Install

```bash
cargo install quasar-cli --version 0.1.0 --locked
```

Quasar 0.1.0 supports Ubuntu 24.04 x86-64 and macOS 15 on Apple Silicon or
Intel. Windows users should use Ubuntu under WSL2. See
[`HOST_SUPPORT.md`](../HOST_SUPPORT.md) for the exact contract.

## Start a project

```bash
quasar init my-program --no-git
cd my-program
quasar build
quasar test
```

`init` is non-interactive and generates one minimal Solana starter, a Rust test
using `quasar-test`, a lockfile, a program keypair, and Rust, Kit, and Web3
client configuration. The only flags are `--no-git` and `--verbose`.

## Core commands

- `quasar init <NAME> [--no-git] [--verbose]`
- `quasar build [--debug] [--verbose] [--watch] [--features FEATURES]`
- `quasar test [--no-build] [--filter PATTERN] [--show-output] [--verbose]`
- `quasar deploy`
- `quasar verify`
- `quasar lint [--strict]`
- `quasar profile [ELF] [--json] [--write-budget|--assert-budget]`
- `quasar idl [PATH]`
- `quasar client <IDL> [--target TARGET]`
- `quasar clean`
- `quasar config`
- `quasar keys`
- `quasar completions`

Client targets are `rust`, `kit`, `web3`, `python`, `go`, and `c`. Omitting
`--target` emits only Kit and Web3. Rust generation is part of the IDL/build
path. Python, Go, and C are preview backends and require explicit selection.

`quasar profile --json` emits deterministic JSON without starting the
interactive profile server. Profile sharing and diffing are preview features.

## Preview tools

Preview tools are explicitly labeled and carry no patch-level compatibility
promise:

```bash
quasar inspect validation [IDL] [--json]
quasar inspect asm [ELF] [--function SYMBOL] [--source]
```

`inspect validation` shows the account-validation plan. `inspect asm`
disassembles an sBPF ELF using the supported Solana platform tools and reports
missing tools or symbols as actionable errors.

## Configuration

`Quasar.toml` is one typed model:

```toml
[project]
name = "my-program"

[testing]
command = { program = "cargo", args = ["test", "tests::"] }

[clients]
path = "target/client"
targets = ["rust", "kit", "web3"]
```

Missing sections receive these defaults. Unknown fields and removed values are
rejected with a supported replacement; old files are not rewritten.

## License

Apache-2.0 OR MIT.
