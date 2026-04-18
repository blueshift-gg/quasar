<h1 align="center">
  <code>quasar</code>
</h1>
<p align="center">
  Zero-copy, zero-allocation Solana program framework.
</p>

> **Beta** — Quasar is under active development and has not been audited. APIs may change. Use at your own risk.

## Overview

Quasar is a `no_std` Solana program framework. Accounts are pointer-cast directly from the SVM input buffer — no deserialization, no heap allocation, no copies. You write `#[program]`, `#[account]`, and `#[derive(Accounts)]` like Anchor, but the generated code compiles down to near-hand-written CU efficiency.

## Quick Start

```bash
cargo install --path cli
quasar init my-program
quasar build
quasar test
```

```rust
declare_id!("22222222222222222222222222222222222222222222");

#[account(discriminator = 1)]
pub struct Counter {
    pub authority: Address,
    pub count: u64,
}

#[derive(Accounts)]
pub struct Increment<'info> {
    #[account(has_one = authority)]
    pub counter: &'info mut Account<Counter>,
    pub authority: &'info Signer,
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
make kani-pod
make kani-lang
make kani-spl
```

CI installs and runs the same Kani version automatically in [`.github/workflows/ci.yml`](.github/workflows/ci.yml).

## Contributing

The best way to contribute now is playing with Quasar. Build programs, test them and if you found any bug or areas to improve, please open an Issue. We still on a unstable version that will be changing a lot. Check [Contributing](CONTRIBUTING.md)

## Workspace

| Crate | Path | Purpose |
|-------|------|---------|
| `quasar-lang` | `lang/` | Account types, CPI builder, events, sysvars, error handling |
| `quasar-derive` | `derive/` | Proc macros for accounts, instructions, programs, events, errors |
| `quasar-spl` | `spl/` | SPL Token / Token-2022 CPI and zero-copy account types |
| `quasar-profile` | `profile/` | Static CU profiler with flamegraph output |
| `cli` | `cli/` | `quasar` CLI — init, build, test, deploy, profile, dump |

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT), at your option.
