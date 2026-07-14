# Compatibility and versioning

Quasar treats `0.1.z` as one compatible release line. Code and generated
artifacts that work with an earlier `0.1.z` release must continue to work with
a later one without source changes. A deliberate compatibility break requires
the lockstep package train to move to `0.2.0`.

This promise applies to Quasar's output for unchanged program source. Program
authors version changes to their own instructions, accounts, and deployed ABI
separately.

## Supported contract

The compatibility contract covers all published Quasar libraries, public
procedural macros, the IDL wire and generated-client output produced for an
unchanged program, and the documented behavior of those surfaces. Private
items, `#[doc(hidden)]` implementation protocols, tests, and undocumented
internal modules are not public API.

All Quasar packages in the release train share one version. Exact internal
dependency pins keep the proc-macro/runtime protocol in lockstep; they do not
make that hidden protocol public.

The automated gates are necessary evidence, not the whole compatibility
decision. A change can preserve syntax while changing runtime behavior, account
requirements, serialization, or generated semantics. Reviewers must classify
those changes even when every snapshot is green.

## Surface rules

### Rust public API

The [Rust API baseline](api-baselines/v0.1.0/README.md) records every published
library under its supported feature profiles. Run `make check-public-api`.

Allowed in `0.1.z`:

- internal refactors and performance fixes that preserve documented behavior;
- new public items that do not invalidate existing source; and
- new inherent methods or trait implementations only when they cannot introduce
  method-resolution or coherence conflicts for existing users.

Requires `0.2.0`:

- removing, renaming, moving, or reducing the visibility of a public item;
- changing a public signature, type, generic bound, feature-gated surface, or
  observable layout guarantee;
- adding a required trait item or a variant to an exhaustive public enum; and
- removing an auto trait or adding an implementation that makes previously
  valid downstream implementations incoherent.

The gate rejects missing or changed baseline items and permits additions.
"Additive" is not automatically compatible: enum variants, trait items,
blanket implementations, and names that collide under glob imports still need
human review.

### Procedural macros

The [proc-macro baseline](compatibility-baselines/v0.1.0/proc-macros/README.md)
checks exported macro names, helper attributes, and representative normalized
expansions. Run `make check-proc-macro-baselines`.

Allowed in `0.1.z`:

- accepting new opt-in syntax without changing existing invocations;
- improving diagnostics for invalid input; and
- changing internal expansion structure when generated public names, trait
  requirements, runtime behavior, and wire output remain compatible.

Requires `0.2.0`:

- removing or renaming a macro or supported helper attribute;
- rejecting a previously valid invocation;
- changing the meaning or default behavior of existing syntax; and
- changing generated public symbols, required user traits, instruction/account
  behavior, or serialization for an unchanged invocation.

Any expansion diff is reviewed as code. It may be compatible, but it is never
blessed solely to make CI green.

### IDL schema and wire contract

The [IDL wire baseline](compatibility-baselines/v0.1.0/idl-wire/README.md)
compares the exact typed projection consumed by `compute_abi_hash`. Run
`make check-idl-wire-baselines`.

Allowed in `0.1.z`:

- documentation, package metadata, error-message text, human-readable
  formulas, and stored-hash presentation changes;
- additive opaque data below `extensions`; and
- new opt-in framework syntax whose output appears only when a program adopts
  it.

Requires `0.2.0`:

- changing discriminators, instruction arguments/codecs/layouts, account
  order or flags, resolver behavior, account/type/event layout, or error
  names/codes for unchanged source;
- changing the meaning of an existing IDL field; and
- adding strict root or leaf fields that existing `quasar-idl/1.x` readers
  reject.

The IDL schema version and the Quasar package version are separate. A Quasar
break requires `0.2.0`; a breaking JSON schema change also requires a new
`quasar-idl/2.0.0` schema identifier. A Quasar `0.2.0` release that preserves
the schema does not need to change the schema major.

### Generated clients

The [generated-client baseline](compatibility-baselines/v0.1.0/generated-clients/README.md)
records normalized Rust, TypeScript, Python, Go, and C output. Run
`make check-generated-client-baselines`; the separate generated-client smoke
job compiles and exercises the emitted code on pinned toolchains.

Allowed in `0.1.z`:

- non-semantic formatting or comment changes;
- additive helpers or opt-in output that do not collide with existing names or
  invalidate exhaustive consumers; and
- implementation fixes that preserve existing public symbols, accepted input,
  encoded bytes, account metadata, and supported dependency floors.

Requires `0.2.0`:

- removing, renaming, or changing the signature/type of a generated public
  symbol;
- changing generated instruction bytes, account ordering/flags, PDA
  derivation, decoders, or error interpretation for the same IDL; and
- changing package/module paths or dependency requirements in a way that
  forces existing generated-client users to change source.

Because the source gate reports every output diff, reviewers must distinguish a
compatible presentation/addition from a breaking client contract change.

## Baseline lifecycle

Before the `v0.1.0` tag, release-candidate fixes may update the `v0.1.0`
baselines when the issue and pull request explain every contract change. After
the tag, that baseline family is immutable.

For a compatible `0.1.z` release:

1. Compare the candidate with the latest published baseline and classify every
   diff as compatible.
2. Capture the reviewed result in a new `v0.1.z` baseline directory; retain all
   older directories as release records.
3. Point the active baseline version in the Makefile at the newly published
   snapshot only as part of that release transition.
4. Publish every Quasar package at the same `0.1.z` version and include the
   compatibility classification in the release notes.

For an intentional break:

1. Target the `0.2.0` release line rather than a `0.1.z` patch.
2. Add a new `v0.2.0` baseline family; never rewrite a tagged `v0.1.z`
   baseline to hide the break.
3. Document affected Rust, macro, IDL, and generated-client migrations.
4. If the strict IDL JSON contract changes, move its schema identifier to
   `quasar-idl/2.0.0` and document reader/writer coexistence.

Security and correctness fixes follow the same rule. If no compatible repair
exists, accelerate the next minor release and provide migration guidance; do
not silently bless a breaking patch.

## Pull-request evidence

A pull request that changes one of these surfaces must state:

- **Compatibility impact:** none, compatible addition, or breaking;
- the affected surface and baseline diff;
- why existing `0.1.z` users do or do not need source changes; and
- the target version transition when the change is breaking.

Run the relevant baseline command before blessing. Baseline updates belong in
the same atomic change as the reviewed surface change so the contract diff is
visible to reviewers.
