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

A framework for Solana programs, written the way Anchor programs are, that
compiles to less code than Anchor, current Quasar and hand-written Pinocchio
in the benchmarks below. Accounts are read and written in place, every check
a program relies on is visible in its source, and properties of a program can
be proved with Kani.

```rust
#![no_std]

use quasar_lang::prelude::*;

declare_id!("33333333333333333333333333333333333333333333");

#[account(discriminator = 1)]
#[seeds(b"vault", owner: Address)]
pub struct Vault {
    pub owner: Address,
    pub deposited: u64,
}

#[derive(InstructionData)]
pub struct Amount {
    pub amount: u64,
}

#[derive(Accounts)]
pub struct Deposit {
    #[account(mut)]
    pub owner: Signer,
    #[account(mut, has_one(owner), seeds(owner))]
    pub vault: Pda<Account<Vault>>,
    pub system_program: Program<SystemProgram>,
}

#[program]
mod vault {
    use super::*;

    #[instruction(0)]
    pub fn deposit(mut ctx: Ctx<Deposit, Amount>) -> Result<()> {
        let amount = ctx.data.amount;
        let Deposit { owner, vault, system_program } = &mut ctx.accounts;
        let deposited = vault.deposited().checked_add(amount);
        vault.set_deposited(deposited.ok_or(ProgramError::ArithmeticOverflow)?)?;
        system_program.transfer(owner, vault, amount).invoke()
    }
}
```

An instruction is three plain parts: its data (`#[derive(InstructionData)]`),
its accounts (`#[derive(Accounts)]`), and a handler, linked by its signature
`Ctx<Deposit, Amount>` and given a discriminator by `#[instruction(0)]`.
`#[program]` finds every handler in its module for the entrypoint to
dispatch to. A mistake, such
as a `Pda` without `seeds(..)`, a missing constraint key or a `String` in
instruction data, is a compile error at the line that makes it.

## CLI

The existing CLI supports project scaffolding, builds, tests, IDL and client
code generation, interface linting, profiling, deployment, keys and shell
completions. Install it from this checkout:

```sh
cargo install --path cli --locked
quasar init my-program --yes --no-git --toolchain solana --test-language rust --rust-framework mollusk
cd my-program
quasar build
quasar test
```

The templates target Quasar 0.3.0. Until those crates are released, replace
the generated `quasar-lang = "=0.3.0"` dependency with a path to this
checkout's `lang` directory. See the [CLI guide](cli/Readme.md) for commands
and configuration.

## Build and test a program

You need Rust 1.92 and the Solana 3.1.14 tools, which `cargo build-sbf`
comes with. The crates are not published yet, so a program depends on them
by path:

```toml
[lib]
crate-type = ["cdylib", "lib"]

[dependencies]
quasar-lang = { path = "../quasar/lang" }

[dev-dependencies]
quasar-test = { path = "../quasar/test" }
```

`cargo build-sbf --tools-version v1.56` builds `target/deploy/<name>.so`. A
test runs that build under the pinned runtime, and builds each instruction
with `name::instruction(accounts, data)`:

```rust
use quasar_lang::Address;
use quasar_test::Test;
use vault::{Amount, ID, Vault};

#[test]
fn a_deposit_is_recorded() {
    let mut t = Test::new(ID, "vault");
    let owner = t.wallet(1_000_000_000);
    let (address, _) = Address::find_program_address(&[b"vault", owner.as_ref()], &ID);
    let vault = t.account_at(address, &Vault { owner, deposited: 0 });
    let system_program = Address::default();

    let deposit = vault::deposit::instruction((owner, vault, system_program), Amount { amount: 500 });
    t.run(deposit).succeeds();

    assert_eq!(t.read::<Vault>(&vault).deposited, 500);
}
```

`examples/` has five complete programs with their tests: `vault`,
`lamport-vault`, `escrow` (SPL Token and Token-2022), `multisig` and `amm`.
Each has an `idl.json` produced from the program itself.

## When an instruction fails

`cargo build-sbf --features quasar-lang/debug` builds the program to test
with: it logs each failed check as rustc would an error, with the account,
what was found and expected, the line of the accounts struct, and how to fix
it. `make sbf` builds it beside the usual build, and a test that fails shows
what it says:

```text
error[HasOne]: account `purse` stores `holder` = 1115..XQs, but the account passed as `holder` is 1117..CNG
    --> src/lib.rs:294:20
     |
 294 |     #[account(mut, has_one(holder))]
     |                    ^^^^^^^ account #2 of the instruction, `purse`, does not satisfy this
     |
     = help: `purse` stores the address of its `holder`: pass that account as `holder`, or pass the `purse` that belongs to this `holder`
```

## Benchmarks

Measured by `make bench` under Agave 3.1.14 through Mollusk: the same
program in each framework. Full tables and method in `bench/RESULTS.md`.

A system-owned PDA such as this vault can be funded at the address of any bump,
so Quasar requires the canonical one: it hashes and curve-checks bumps from 255
down, about 150 compute units per bump, as Quasar master and Pinocchio do.
An account the program owns needs only the hash: the program created it, at
the canonical bump.

| Vault (deposit / withdraw CU, binary bytes) | |
|---|---|
| Quasar, this repository | 2189 / 2216, 5608 |
| Quasar master | 2198 / 2215, 6440 |
| anchor-next v2 | 2280 / 2331, 9584 |
| Pinocchio, by hand | 5877 / 5890, 6344 |

## Documentation

Full documentation at **[quasar-lang.com](https://quasar-lang.com)**.

## Working on Quasar

The [Metaplex integration](metadata/README.md) provides metadata/master-edition
accounts, constraints, initialization and CPI helpers for the rewritten APIs,
with a migration guide from the previous `quasar-metadata` crate.

CI runs `make check`, `make test-miri`, and `make test-miri-strict`.
`make check` includes `make check-features`, which checks each workspace crate's
feature powerset without dev-dependency feature unification. To run it locally,
install `cargo-hack` with `cargo install --locked cargo-hack@0.6.45`.
`ARCHITECTURE.md` is the map of the code.

Workspace SBF builds default to platform-tools v1.56. Override the version with
`make sbf PLATFORM_TOOLS_VERSION=v1.53`, or pass explicit build options with
`make sbf SBF_TOOLS='--tools-version v1.56 --arch v0'`. These overrides also
apply to comparison-program and profiling builds. Benchmark results depend on
the compiler, so CI keeps the pinned default. For CLI projects, configure
`tools_version` and `arch` under `[toolchain]` in `Quasar.toml`; see the
[CLI guide](cli/Readme.md).

Developer shortcuts:

| Command | Action |
| --- | --- |
| `make format` / `make format-fix` | Check / apply workspace formatting |
| `make clippy-fix` | Apply Clippy fixes to the workspace |
| `make build` / `make build-sbf` | Build host crates / normal and debug SBF programs |
| `make generated-client-smoke` | Generate and check clients in all supported languages |
| `make bench-cu` / `make bench-tracked` | Print the current benchmark report |
| `make compare-tracked` | Compare against `bench/RESULTS.md` |
| `make help-kani` / `make check-kani` | Show setup instructions / verify the pinned version |
| `make kani-lang` / `make kani-metadata` | Run one crate's proofs |
| `make test-all` | Run `make check` and both Miri modes |
| `make check-policies` | Check workspace lint inheritance, panic exceptions, and CLI conventions |
| `make nightly-version` | Print the pinned Miri nightly |

`make bench` explicitly updates the benchmark baseline; the reporting shortcuts
leave it intact. Kani helpers target the crates that currently contain proofs.

`make check` includes the repository policy checks. They can also run separately:
`make check-workspace-lints`, `make check-runtime-panics`, and
`make check-cli-conventions`. They require Python 3.11+.

The panic check scans `lang`, `spl`, `metadata`, and `derive` for explicit
`panic!`, `unreachable!`, `todo!`, and `unimplemented!` calls. Reviewed exceptions
in `.github/scripts/check-policies.py` have exact call text, paths, occurrence
counts, and reasons; stale exceptions fail too. CLI checks reserve `exit` for
`cli/src/main.rs` and `sleep` for `cli/src/build_watch.rs`, and reject whitespace
splitting in favor of structured arguments or quote-aware parsing. Comments and
string literals are ignored. These are lexical convention checks: they do not
resolve arbitrary aliases or prove that indexing, arithmetic, or `unwrap()`
cannot panic. `make test-policies` exercises the checker itself.

CI caches Cargo downloads, Solana/SBF toolchains, and cargo-expand/cargo-hack.
Changes are classified by `.github/filters.yml`: root documentation-only edits
skip builds, and CLI-only edits skip Miri. Reusable workflow calls default to a
full check, and release tags always run all gates.

Install the pinned Miri toolchain with
`rustup toolchain install "$(make miri-version)" --component miri`.
The Miri targets check account borrowing, resizing, zero-copy views, CPI
argument buffers, and SPL layouts under Tree Borrows and symbolic alignment;
the strict target also checks strict provenance. These deterministic tests
also run in ordinary Cargo tests. Miri complements Kani proofs and SVM tests:
it checks the host paths exercised by these tests, including simulated CPI
memory access, rather than executing Solana syscalls.

Tags matching the workspace version (for example, `v0.3.0`) run the same gate,
including `make publish-check`, which builds the registry packages without
uploading. The release workflow publishes the framework, CLI, IDL/schema
and profiling crates in dependency order before creating a GitHub release.
Publishing uses `CARGO_REGISTRY_TOKEN` from the `crates-io` environment; reruns
skip crate versions already published.

## Contributing

The best way to contribute now is playing with Quasar. Build programs, test them and if you found any bug or areas to improve, please open an Issue. We still on a unstable version that will be changing a lot. Check [Contributing](CONTRIBUTING.md)

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT), at your option.
