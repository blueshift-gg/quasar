# Contributing

Quasar is in beta, performance-critical, and unaudited. We are not accepting
pull requests yet; issues with concrete reproductions and use cases are
welcome.

## Repository shape

- `lang/`, `spl/`, `test/`, and `cli/` own the four products.
- `derive/` and `idl/` support those products; the IDL wire schema lives under
  `idl/schema/`.
- `examples/` contains maintained programs that exercise generated clients and
  enforce their own compute-unit and ELF-size ceilings.
- `tests/programs/` contains SBF fixtures; `tests/suite/` asserts their behavior.

`test/` is the source of the published `quasar-test` crate. The plural
`tests/` directory is repository-only integration coverage.

Keep a rule next to the code that owns it. Do not add package inventories,
workflow parsers, source scanners, or tests whose only subject is another
check. Cargo discovers packages and test targets; compiler lints own source
policy.

Unsafe code needs a local `SAFETY` argument stating the invariant. Tests at an
unsafe boundary should name the failure they catch. Semantic behavior belongs
in fast host or SBF tests; Miri is for aliasing, provenance, initialization,
pointer boundaries, and downstream extension points.

## Before review

Run the narrow gate while iterating, then the affected product gate:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
make test
make contracts
make package-check
```

Unsafe changes also run `make miri`, `make kani`, and `make fuzz-build`.
Compute-unit and ELF-size budgets are assertions in the owning example tests,
so `make test` protects them without a separate benchmark framework.

## Compatibility fixtures

Read [VERSIONING.md](VERSIONING.md) before changing a published Rust item,
macro expansion, IDL field, wire layout, or generated client. State the
compatibility impact and update the owning behavioral or generated-output
fixture.

Trybuild `.stderr` files specify compiler diagnostics. Regenerate an intended
change with `make test-bless` and review every changed line. Proc-macro
expansions live under `derive/tests/compatibility-v0.1.0/`; regenerate them
with `make bless-proc-macro-baselines`. IDL wire and generated-client fixtures
live under `idl/tests/`. Never enable snapshot-update modes in ordinary tests
or CI.

## Releasing

Exact-head CI verifies the repository, while `make package-check` validates the
publishable archive contents. Credentialed publication, registry-sequenced
verification, and GitHub release creation happen in a separately controlled
environment with `cargo publish --workspace --locked`; publisher code and
registry credentials do not belong here. Final `@solana/web3.js@3.0.0` is a
0.1.0 tag requirement.
