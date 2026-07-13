# Generated-client contract baseline

This baseline enforces the [compatibility and versioning
policy](../../../VERSIONING.md) for generated source surfaces.

These files freeze the v0.1.0 output of Quasar's supported Rust, TypeScript,
Python, Go, and C client generators. Paths below each fixture preserve the real
generated language and package layout, so failures identify the affected file
and show the changed symbol in a focused source diff.

The `multisig` and `escrow` fixtures jointly cover fixed and compact
instruction layouts, account and event decoding, custom types, errors, PDA and
constant resolvers, token-related resolution, and bounded and unbounded
remaining accounts.

Run `make check-generated-client-baselines` to build each fixture twice and
compare every emitted file with the release baseline. Raw trees must be
byte-for-byte deterministic. The checked presentation removes trailing spaces
and terminal blank lines, then requires exactly one final newline; no tokens or
internal blank lines change. The gate also rejects missing language roots, file
inventory drift, non-UTF-8 output, and carriage returns. Run
`make bless-generated-client-baselines` only after reviewing every generated
source change and removing any obsolete baseline file explicitly.
