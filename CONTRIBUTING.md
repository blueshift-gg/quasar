# Contributing

Quasar is in beta. The API is unstable and the internals are performance-critical — small changes can have outsized CU impact. We're not accepting pull requests at this time.

To understand how the pieces fit — the compiler front-end (`derive/`), the
runtime it targets (`lang/`), the accounts pipeline phases, and the enforced
layer contracts — read [`ARCHITECTURE.md`](./ARCHITECTURE.md) first. Test
requirements — the suite layers, the per-feature test contract, and the
assertion rules every test must follow — live in
[`TESTING.md`](./TESTING.md). Code style — the unsafe discipline, layout
and constant rules, and the register runtime code is written in — lives in
[`STYLE.md`](./STYLE.md).

## How to help

Open an issue if you:

- **Found a bug** — include a minimal reproduction and the error output
- **Think a feature is missing** — describe the use case, not just the solution
- **See something that could be done better** — we want to hear it, even if we don't act on it immediately

## What we'll do

We read every issue. If it's a real bug, we'll fix it. If it's a good idea, we'll track it. If we disagree, we'll explain why.

## Compatibility review

Read the [compatibility and versioning policy](VERSIONING.md) before changing a
published Rust item, macro expansion, IDL field, wire layout, or generated
client. Every surface change must state its compatibility impact and include
the relevant baseline diff. A red baseline is evidence to review, not a reason
to bless snapshots automatically.

## Compiler diagnostic goldens

The `.stderr` files under `lang/tests/compile_fail/` and `derive/tests/` are the
spec of the diagnostics the macros emit. `make test` (and CI) run trybuild in
assert mode: a diagnostic that drifts from its golden fails the build.

When you intend to change a diagnostic, run `make test-bless` to regenerate the
goldens (`TRYBUILD=overwrite`), then review every regenerated `.stderr` diff
like code — each hunk must be a deliberate, correct diagnostic, not an
accidental regression. Never set `TRYBUILD=overwrite` in `make test` or CI.

## Owner-local compatibility fixtures

Proc-macro expansion fixtures live under
`derive/tests/compatibility-v0.1.0/` and are asserted directly by the derive
crate. A snapshot diff is a codegen change. Regenerate deliberately with
`make bless-proc-macro-baselines`, then review every changed line.

IDL wire and generated-client fixtures live under `idl/tests/`. Rust, Kit, and
Web3 output are stable contracts; Python, Go, and C retain functional tests
without patch-level snapshots. Run `make contracts` to exercise the owning
Rust and JavaScript suites. Never set `UPDATE_EXPECT` in ordinary tests or CI.

## When this changes

Once the API stabilizes and we've been audited, we'll open up contributions. Until then, issues are the best way to shape the project.
