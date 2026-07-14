# IDL wire-contract baseline

This baseline enforces the [compatibility and versioning
policy](../../../VERSIONING.md) for IDL wire behavior.

These files freeze the v0.1.0 ABI projection emitted by representative Quasar
programs. The projection is the exact typed subset hashed by
`compute_abi_hash`, rendered as pretty JSON for review.

The `multisig` and `escrow` fixtures jointly cover:

- program, instruction, account, event, and error names and discriminators;
- fixed and compact instruction layouts;
- fixed and compact account layouts and their space bounds;
- fixed and dynamic argument codecs;
- ordered account metadata, optionality, signer and writable requirements;
- input, constant, PDA, and token-related resolver data; and
- bounded and unbounded remaining-account policies.

Documentation, schema and package versions, metadata, error messages,
human-readable space formulas, opaque semantics and extensions, and stored
hashes are intentionally absent. Tests prove mutations to those fields leave
both the projection and ABI hash unchanged.

Run `make check-idl-wire-baselines` to rebuild each program twice and compare
its normalized projection with this release baseline. A changed wire contract
fails the dedicated CI job with a focused JSON diff. Run
`make bless-idl-wire-baselines` only for a reviewed compatible addition or the
documented version transition.
