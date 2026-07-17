<h1 align="center">
  <code>quasar</code>
</h1>
<p align="center">
    <img src="./assets/logo-full.svg" alt="Quasar Logo" width="400">
</p>
<p align="center">
  Zero-copy, zero-allocation Solana program framework.
</p>

> **Beta** — Quasar is under active development and has not been audited. APIs may change. Use at your own risk.

## Overview

Quasar is a `no_std` Solana program framework. Accounts are pointer-cast directly from the SVM input buffer — no deserialization, no heap allocation, no copies. You write `#[program]`, `#[account]`, and `#[derive(Accounts)]` like Anchor, but the generated code compiles down to near-hand-written CU efficiency.

## Quick Start

```bash
cargo install quasar-cli --version 0.1.0 --locked
quasar init my-program
quasar build
quasar test
```

```rust
use quasar_lang::prelude::*;

declare_id!("22222222222222222222222222222222222222222222");

#[account(discriminator = 1)]
pub struct Counter {
    pub authority: Address,
    pub count: u64,
}

#[derive(Accounts)]
pub struct Increment {
    #[account(mut, has_one(authority))]
    pub counter: Account<Counter>,
    pub authority: Signer,
}

#[program]
mod counter_program {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn increment(ctx: Ctx<Increment>) -> Result<(), ProgramError> {
        ctx.accounts.counter.count += 1;
        Ok(())
    }
}
```

## Documentation

Full documentation at **[quasar-lang.com](https://quasar-lang.com)**.

## Host support

Quasar v0.1.0 supports Ubuntu 24.04 x86-64, macOS 15 on Apple Silicon, and
macOS 15 on Intel. Windows users should run Ubuntu 24.04 x86-64 under WSL2;
native Windows is unsupported. See the exact [host support
contract](HOST_SUPPORT.md), including the CI rows and unsupported paths.

## Compatibility

Quasar `0.1.z` releases preserve the published Rust, proc-macro, IDL wire, and
generated-client contracts. Intentional breaking changes move the lockstep
release train to `0.2.0`. See the exact [compatibility and versioning
policy](VERSIONING.md), including the enforceable baseline gates and release
transition process.

## Verification

Local Kani verification is optional. Normal builds and tests do not require Kani:

```bash
make test
```

If you want to run the model-checking harnesses locally, install `kani 0.67.0` to match CI, then verify the tool version:

```bash
kani --version
make check-kani
```

Run all proof suites:

```bash
make kani
```

Or run a single crate:

```bash
make kani-lang
make kani-spl
make kani-metadata
```

CI installs and runs the same Kani version automatically in [`.github/workflows/ci.yml`](.github/workflows/ci.yml).

## Contributing

The best way to contribute now is playing with Quasar. Build programs, test them and if you found any bug or areas to improve, please open an Issue. We still on a unstable version that will be changing a lot. Check [Contributing](CONTRIBUTING.md)

## Published crates

The v0.1.0 release publishes these workspace packages. Test programs, examples,
and their clients are not published.

<!-- published-crate-inventory:start -->
| Package | Path | Purpose |
| --- | --- | --- |
| `quasar-cli` | `cli/` | CLI for the Quasar Solana framework |
| `quasar-derive` | `derive/` | Proc macros for the Quasar Solana framework |
| `quasar-idl` | `idl/` | IDL generator for the Quasar Solana framework with discriminator collision detection |
| `quasar-idl-schema` | `idl/schema/` | Public IDL JSON schema types for the Quasar Solana framework |
| `quasar-lang` | `lang/` | Zero-copy Solana program framework |
| `quasar-metadata` | `metadata/` | Metaplex Token Metadata integration for the Quasar Solana framework |
| `quasar-profile` | `profile/` | SBF binary profiler for the Quasar Solana framework |
| `quasar-schema` | `schema/` | Shared schema types for Quasar interfaces |
| `quasar-spl` | `spl/` | SPL Token program CPI and zero-copy account types for the Quasar Solana framework |
| `quasar-test` | `testing/` | Project-aware QuasarSVM testing utilities for Quasar programs |
| `quasar-test-derive` | `testing/derive/` | Attribute macros for quasar-test |
| `solana-compiler-builtins` | `solana-compiler-builtins/` | Compiler runtime builtins required by Quasar SBF programs |
<!-- published-crate-inventory:end -->

The TypeScript test SDK is published as `@blueshift-gg/quasar-test`, with
separate `/kit` and `/web3.js` entry points.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT), at your option.
