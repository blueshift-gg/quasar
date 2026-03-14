<h1 align="center">
  <code>quasar</code>
</h1>
<p align="center">
  Write optimized Solana programs without thinking too much.
</p>

## Overview

Quasar is a `no_std` Solana program framework that combines zero-copy access, zero-allocation dispatch, and Anchor-level developer experience. It provides `#[account]`, `#[derive(Accounts)]`, `#[instruction]`, `#[program]`, `#[event]` — but the generated code operates directly on the SVM input buffer with no deserialization step.

```toml
[dependencies]
quasar = "0.1"
```

This re-exports `quasar-core` and `quasar-spl` (via the `spl` feature, on by default).

## Compute Units

Both programs implement the same vault logic and run against the same test harness:

| Instruction | Quasar | Pinocchio (hand-written) | Delta |
|-------------|--------|--------------------------|-------|
| Deposit     | 2,816  | 2,833                    | -17   |
| Withdraw    | 1,618  | 1,635                    | -17   |

## Quick Start

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

| Document | Content |
|----------|---------|
| [CLI](docs/cli.md) | `quasar` build tool — init, build, test, deploy, profile, dump, config |
| [Accounts](docs/accounts.md) | Account types, zero-copy access, discriminators, constraints, dynamic data, remaining accounts |
| [CPI](docs/cpi.md) | `CpiCall` const-generic builder, SystemProgram CPI, raw syscalls, PDA seeds, signing patterns |
| [Tokens](docs/tokens.md) | SPL Token / Token-2022 integration, interface types, CPI methods, initialization |
| [Pod Types](docs/pod.md) | Alignment-1 integers (`PodU64`, etc.), arithmetic behavior, standalone usage |
| [Events](docs/events.md) | `#[event]` macro, `emit!()` log-based vs self-CPI emission, event authority PDA |
| [Macros](docs/macros.md) | `#[program]`, `#[instruction]`, `#[error_code]`, dispatch model, generated code |
| [IDL](docs/idl.md) | IDL generator CLI, JSON output format, TypeScript codegen, collision detection |
| [Safety](docs/safety.md) | Unsafe inventory, soundness arguments, Miri validation, attack surface analysis |

## Workspace

| Crate | Path | Purpose |
|-------|------|---------|
| `quasar` | `quasar/` | Facade crate — the single dependency for programs |
| `quasar-core` | `core/` | Account types, CPI builder, events, sysvars, error handling |
| `quasar-derive` | `derive/` | Proc macros for accounts, instructions, programs, events, errors |
| `quasar-pod` | `pod/` | Alignment-1 integer types — usable independently of the framework |
| `quasar-spl` | `spl/` | SPL Token program CPI and zero-copy `TokenAccountState` |
| `quasar-idl` | `idl/` | IDL generator with discriminator collision detection |
| `quasar-profile` | `profile/` | Static CU profiler, flamegraph JSON output, background server |
| `cli` | `cli/` | `quasar` binary — build, test, profile, dump, init, config |

## CLI

```bash
cargo install --path cli
```

```bash
quasar init my-program       # Scaffold with saved defaults (no prompts)
quasar init                  # Interactive wizard
quasar build [--watch]       # Compile the on-chain program
quasar test [--watch]        # Run the test suite
quasar deploy                # Deploy to a cluster
quasar profile [--watch]     # Measure compute-unit usage
quasar dump                  # Dump sBPF assembly
quasar config list           # Manage global settings
quasar completions <shell>   # Generate shell completions
```

See [CLI docs](docs/cli.md) for flags and configuration.

## Development

```bash
# Check workspace
cargo check --workspace

# Lint
cargo clippy --workspace -- -D warnings

# Run Miri UB tests (requires nightly)
MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
  cargo +nightly miri test -p quasar-core --test miri
```

## License

MIT
