# Proc-macro expansion baseline

This baseline enforces the [compatibility and versioning
policy](../../../VERSIONING.md) for public macros and their generated code.

These files freeze representative v0.1.0 expansions for every public macro
exported by `quasar-derive`. `profiles.tsv` maps each macro family to one or
more named fixtures and makes the coverage auditable against the Rust public
API baseline.

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

After publishing a compatible 0.1.z release, copy the reviewed snapshots into
that release's baseline directory and update `PROC_MACRO_BASELINE_VERSION`.
Breaking codegen changes require the version transition defined by the
compatibility policy.
