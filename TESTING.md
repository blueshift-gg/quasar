# Testing

Quasar tests user promises, not matrix completeness. Every test owns a unique
failure story: a bug that would escape if that test disappeared. Coverage,
mutation experiments, and suite size are supporting signals rather than
release definitions.

## Required release evidence

| Promise | Evidence |
| --- | --- |
| Rust quality | Formatting, Clippy with warnings denied, and warning-free Rustdoc on the supported toolchain |
| Core behavior | Owner-local host tests for stable crates and CLI logic |
| On-chain behavior | Real SBF programs with exact error and resulting-state assertions |
| Stable contracts | Derive diagnostics, IDL wire fixtures, and independently compiled and executed Rust, Kit 7, and Web3.js 3 client contracts |
| Package journey | Immutable package archives install, initialize, build, test, and generate clients without repository source |
| Dependency safety | Normal dependency-audit output for the complete workspace lockfile |
| Unsafe boundaries | Focused Miri, Kani, and fuzz-build checks for changed parsing, aliasing, provenance, or initialization code |
| Performance | Compute-unit and binary-size budgets for runtime, derive, and SPL changes |

Python, Go, and C functional compilation runs when a preview backend or the
shared codegen model changes. It is required for that change, but is not an
unrelated core-release dependency.

## Test ownership

Put a test beside the implementation that owns the contract:

- derive compile failures and representative expansions live under `derive/`;
- IDL wire and generated-client fixtures live under `idl/`;
- runtime and zero-copy host tests live under `lang/`;
- SPL validation and CPI behavior live under `spl/`;
- Rust test utilities and macros live under `testing/`;
- CLI unit tests cover private logic and binary integration tests cover public
  behavior; and
- real SBF fixtures live under `tests/programs/`, with typed assertions in
  `tests/suite/`.

Do not duplicate the same contract in a root snapshot and an owner-local
fixture. Stable client changes should produce one focused diff in the codegen
owner.

## Assertions

Failure tests assert the exact typed error. Success tests assert resulting
state, emitted bytes, or another product outcome rather than status alone.
Lamport-moving tests assert both sides of the transfer. A regression test must
fail before its fix.

Adversarial cases are added where they exercise a distinct branch: one-byte
truncation, bit corruption, account substitution, duplicate regions, wrong
signer or writability, wrong owner, and exact-boundary pointer walking. Values
that flow through the same branch belong in a table-driven test.

## Miri, Kani, and fuzzing

Miri is reserved for undefined-behavior questions:

- pointer provenance and aliasing;
- initialization and all-bit-pattern validity;
- exact-boundary pointer walking;
- duplicate-account region behavior;
- macro-generated decoder soundness; and
- safe downstream extension points backed by unsafe internals.

Ordinary ownership, validation, and semantic round-trip behavior belongs in
fast host or SBF tests. Before removing a Miri case, add or identify the fast
test with the same oracle. Tests that depend on Tree Borrows keep that model;
the small adversarial extension suite also runs under the supported alternate
borrow model.

Kani proves bounded properties in `quasar-lang` and `quasar-spl`. Fuzz target
discovery and builds are required; nightly runs and weekly soaks search the
same live target set. Account-region modeling remains scheduled rather than
blocking every release.

## CI cadence

Pull requests run the stable promises affected by the change. Tag and release
runs execute the complete core gate even when the commit was already tested as
part of a pull request.

Nightly or scheduled jobs run full Kani, fuzzing, informational host coverage,
and longer assurance work. Targeted mutation investigation uses `cargo
mutants` manually; it has no repository baseline or package matrix.

Generated fixtures are reviewed like code. Regeneration is never used to
silence an unexplained semantic diff.
