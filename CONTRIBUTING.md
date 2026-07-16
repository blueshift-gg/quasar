# Contributing

Quasar is in beta. The API is unstable and the internals are performance-critical — small changes can have outsized CU impact. We're not accepting pull requests at this time.

To understand how the pieces fit — the compiler front-end (`derive/`), the
runtime it targets (`lang/`), the accounts pipeline phases, and the enforced
layer contracts — read [`ARCHITECTURE.md`](./ARCHITECTURE.md) first. Test
requirements — the suite layers, the per-feature test contract, and the
assertion rules every test must follow — live in
[`TESTING.md`](./TESTING.md).

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

## Macro expansion snapshots

The `.rs` files under
`compatibility-baselines/v0.1.0/proc-macros/expansions/` are pretty-printed
goldens of what each proc macro expands to — the reviewable spec of the
generated code. `make check-proc-macro-baselines` and its dedicated CI job
assert them: an expansion that drifts from its golden fails the build.

A snapshot diff **is** a codegen change. When you intend to change emission,
run `make bless-proc-macro-baselines` to regenerate the goldens, then review
every diff hunk like code — each change must be a deliberate, correct emission
difference, never blessed blindly. Never set `UPDATE_EXPECT` in `make test` or
CI.

## IDL wire-contract snapshots

The pretty JSON files under
`compatibility-baselines/v0.1.0/idl-wire/programs/` contain the exact typed
projection used by the ABI hash, rebuilt from representative programs. Run
`make check-idl-wire-baselines` to compare generated wire contracts. Run
`make bless-idl-wire-baselines` only after reviewing the compatibility impact;
documentation-only changes never appear in this projection.

## Generated-client snapshots

The source trees under
`compatibility-baselines/v0.1.0/generated-clients/outputs/` freeze every
supported client language for representative programs. Run
`make check-generated-client-baselines` to compare generated Rust, TypeScript,
Python, Go, and C files. Run `make bless-generated-client-baselines` only after
reviewing every source diff and explicitly removing obsolete files.

## When this changes

Once the API stabilizes and we've been audited, we'll open up contributions. Until then, issues are the best way to shape the project.
