# Compatibility and versioning

Quasar treats `0.1.z` as one compatible release line. A deliberate break to a
stable contract moves the lockstep release train to `0.2.0`. Program authors
version their own deployed instructions, accounts, and ABI separately.

## Stable contracts

Patch releases preserve:

- the documented `quasar-lang`, `quasar-spl`, and `quasar-test` Rust APIs and
  runtime behavior;
- documented macro syntax and generated behavior;
- the IDL wire format, schema identity, and ABI hashing rules;
- generated Rust, Kit 7, and final Web3.js 3 client signatures and wire
  behavior;
- the documented behavior of stable CLI commands; and
- source-free installation and the canonical init, build, test, deploy,
  verify, and debug journey.

The CLI's Rust modules, command structs, and helper types are private
implementation. No compatibility promise applies to importing `quasar-cli` as
a library.

Rust clients are emitted through the program IDL/build path. `quasar client`
generates Kit and Web3 by default, or one explicitly selected target. The
stable JavaScript dependency floors are Kit `7.x` and Web3.js `3.x`; a
prerelease Web3.js dependency is never a stable 0.1 artifact.

## Supporting crates

`quasar-derive`, `quasar-idl`, `quasar-idl-schema`,
and `quasar-test-derive` are published to support the primary products.

Their intentional contracts are protected where users depend on them:
proc-macro input, diagnostics, and expansion behavior; IDL wire behavior; and
Rust testing macros. Other direct Rust APIs in these implementation crates may
change within `0.1.z` when the stable product contracts remain intact.

Exact internal dependency pins keep the runtime, derives, schema, and CLI in
lockstep. They do not make internal protocols public.

## Preview contracts

Python, Go, and C client generation, validation-plan inspection, and assembly
inspection are preview.

Preview capabilities:

- require explicit invocation and are absent from the canonical starter;
- run their own functional tests when their backend or shared codegen model
  changes;
- do not block an unrelated stable release; and
- carry no patch-level source, formatting, or generated-output compatibility
  promise.

Preview output is still expected to be correct. “Preview” changes the
compatibility and release-gating promise, not the quality bar.

## Change rules

A `0.1.z` patch may add opt-in behavior, diagnostics, and helpers when existing
source and wire behavior remain valid. Removing or renaming a stable item,
changing a stable signature or default, changing instruction bytes, account
ordering, PDA derivation, decoder behavior, IDL meaning, or generated stable
client behavior requires `0.2.0`.

Before the `v0.1.0` tag, release-candidate fixes may update owner-local
fixtures when the pull request explains the contract change. After a tag, its
recorded stable fixtures are immutable. A new patch captures a new reviewed
fixture set rather than rewriting a published record.

Security fixes follow the same rule. If a sound repair cannot preserve a
stable contract, release the next minor version with migration guidance.

## Pull-request evidence

A change to a stable surface states:

- the compatibility impact;
- the affected owner-local fixture or behavioral test;
- why existing users do or do not need source changes; and
- the required version transition if the change is breaking.

Snapshots are evidence, not the compatibility decision. Reviewers also
classify runtime behavior, account requirements, serialization, generated
semantics, and dependency changes.
