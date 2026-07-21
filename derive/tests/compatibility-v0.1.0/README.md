# Proc-macro expansion baseline

This baseline enforces the [compatibility and versioning
policy](../../../VERSIONING.md) for public macros and their generated code.

These files freeze representative v0.1.0 expansions for the intentional macro
contracts exported by `quasar-derive`. Each expansion is invoked and asserted
directly by `derive/src/snapshot_tests.rs`; there is no secondary inventory.

The fixtures invoke the same internal expansion functions as the public proc
macro entry points. Their token streams are parsed with `syn` and formatted
with `prettyplease`, both locked by `Cargo.lock`. This keeps compiler spans,
hygiene identifiers, filesystem paths, and rustc pretty-printing out of the
comparison. The expression emitted by `emit_cpi!` is parsed as a `syn::Expr`
and normalized directly. `declare_program!` reads the checked-in IDL under
`fixtures/`, so its generated CPI module is reproducible without a generated
or temporary input.

Run `make check-proc-macro-baselines` to assert the published snapshots. A
changed expansion fails the dedicated CI job with the focused `expect-test`
diff. Run `make bless-proc-macro-baselines` only for an intentional codegen
change and review every changed line before committing it.

Breaking codegen changes require the version transition defined by the
compatibility policy.
