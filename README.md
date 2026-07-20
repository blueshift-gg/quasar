<h1 align="center">
  <code>quasar</code>
</h1>
<p align="center">
  <img src="./assets/logo-full.svg" alt="Quasar Logo" width="400">
</p>
<p align="center">
  Zero-copy, zero-allocation Solana programs.
</p>

> **Beta** — Quasar has not been audited. Do not use it with real funds.

Quasar is a `no_std` Solana program framework. Accounts are viewed directly in
the SVM input buffer, avoiding heap allocation, copies, and routine
deserialization. Familiar program and account macros compile to code designed
for hand-written compute-unit efficiency.

## Quickstart

```bash
cargo install quasar-cli --version 0.1.0 --locked
quasar init my-program
cd my-program
quasar build
quasar test
```

`quasar init` creates the supported minimal starter, a Rust test using
`quasar-test`, and Rust, Kit 7, and Web3.js 3 client configuration. It uses the
Solana `cargo build-sbf` toolchain and does not require a template, framework,
language, SDK, or package-manager choice.

## Products

Quasar 0.1.0 has four primary Rust products:

| Product | Promise |
| --- | --- |
| `quasar-lang` | Program macros, account types, validation, runtime behavior, and the documented zero-copy contract |
| `quasar-spl` | SPL account views, validation, and CPI helpers |
| `quasar-test` | Rust testing utilities and macros for Quasar programs |
| `quasar-cli` | The install, init, build, test, deploy, verify, debug, and client-generation journey |

Several crates are published because those products require them:
`quasar-derive`, `quasar-idl`, `quasar-idl-schema`, `quasar-schema`,
`quasar-test-derive`, and `solana-compiler-builtins`. They are supporting
machinery, not additional products. Their intentional proc-macro, IDL wire,
testing-macro, and compiler-runtime contracts are protected without promising
that every implementation detail is a stable direct Rust API.

## Stable and preview capabilities

Stable CLI behavior includes `init`, `build`, `test`, `deploy`, `verify`,
`lint`, `profile`, `idl`, `client`, `clean`, `config`, `keys`, and
`completions`. The IDL wire format and ABI hash, Rust client generation, Kit 7
generation, and final Web3.js 3 generation are also stable.

Python, Go, and C clients are preview targets. Validation-plan and assembly
inspection, profiler sharing, and the profile-diff server are preview tools.
Preview features require explicit invocation, are not used by the starter, and
carry no patch-level compatibility promise.

See [VERSIONING.md](VERSIONING.md) for the exact contract,
[TESTING.md](TESTING.md) for release evidence, and
[HOST_SUPPORT.md](HOST_SUPPORT.md) for supported development hosts.

## Documentation

User documentation is available at
[quasar-lang.com](https://quasar-lang.com). Repository architecture and
contributor workflows live in [ARCHITECTURE.md](ARCHITECTURE.md) and
[CONTRIBUTING.md](CONTRIBUTING.md). Release owners use
[RELEASING.md](RELEASING.md).

## License

Licensed under either the [Apache License, Version 2.0](LICENSE-APACHE) or the
[MIT License](LICENSE-MIT), at your option.
