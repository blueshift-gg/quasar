# Architecture

Quasar is a **compiler for Solana programs**. You write a program as
attribute-annotated Rust; the `quasar-derive` proc macros compile it into the
account parsing, validation, and CPI code that runs on-chain against the
`quasar-lang` runtime. This document maps the compiler's phases and the layer
contracts that keep it honest. It describes the *current* code — file paths are
the reference, and internal IR types are named only where a file needs them.

## 1. The compiler and its runtime

| Crate | Role |
|-------|------|
| `derive/` (`quasar-derive`) | The **compiler front-end**: proc macros (`#[derive(Accounts)]`, `#[instruction]`, `#[program]`, `#[account]`, `#[event]`, `#[error_code]`, `#[derive(Seeds)]`, `#[derive(QuasarSerialize)]`) that emit runtime-targeting Rust. |
| `lang/` (`quasar-lang`) | The **runtime** the generated code targets: account wrappers, traits, checks (`lang/src/checks/`), CPI, the entrypoint/allocator macros (`lang/src/entrypoint.rs`), and the `AccountBehavior` plugin contract. |
| `spl/`, `metadata/` | **Reference plugins**. They are built entirely on the stable `AccountBehavior` + CPI-trait surface; the derive has *zero* knowledge of what `token`, `mint`, or `metadata` mean. |
| `idl/` (`quasar-idl`) | The IDL 1.0 schema, whole-program lints, and client codegen backends. |
| `cli/` | The `quasar` CLI (build/deploy/idl). It owns the short crate name `quasar`; there is no umbrella library crate (see `lang/src/prelude.rs`). |

Generated code references the runtime crate by the name the *consumer* gave it
in `Cargo.toml` (renames allowed), resolved once in `derive/src/krate.rs`.

## 2. The accounts pipeline

`#[derive(Accounts)]` is a straight-line pipeline; each phase is a directory of
files, and each phase may only do its own job (`derive/src/accounts/mod.rs`
carries the canonical summary):

```
syntax  -> parse raw #[account(...)] directive grammar        derive/src/accounts/syntax/
lower   -> directives become FieldSemantics                   derive/src/accounts/resolve/lower.rs
rules   -> validate structural invariants (no protocol)       derive/src/accounts/resolve/rules.rs
planner -> schedule protocol-neutral phase candidates         derive/src/accounts/resolve/planner.rs (+ specs.rs)
emit    -> generate Rust from the plan, and only the plan     derive/src/accounts/emit/
```

- **syntax** makes no semantic decisions — it only recognizes the grammar.
- **lower**/**rules** never emit code.
- **planner** produces the plan, which is the **sole source of emission**;
  **emit** (`entry.rs`, `parse.rs`, `typed_emit.rs`, `output.rs`, `ix_args.rs`)
  reads the plan and never reaches back into `FieldSemantics`.

`#[program]` has its own three-stage shape under `derive/src/program/`:
**scan** (`scan.rs`, one pass over the module items) -> **model**
(`model.rs`, all validation as `syn::Result` methods) -> **emit**
(`dispatch.rs` for dispatch/entrypoint, `event_authority.rs`, `idl.rs`).

`derive/src/schema_ir.rs` is the **one** compact lowering: the `String<N>` /
`Vec<T, N>` dynamic wire layout is computed there and shared by the account,
instruction, `#[event]`, and `QuasarSerialize` emitters, so all four agree
byte-for-byte.

## 3. The enforced layer contract

Two deny-tests are executable statements of the architecture; they run under
`make test` / CI:

- **`derive/tests/deny_domain_strings.rs`** — no SPL/protocol domain string
  (`TokenParams`, `quasar_spl`, `TokenProgram`, …) may appear anywhere in
  `derive/src/accounts/`. **Rule:** if your change needs a domain string in
  `derive/src/accounts/`, it belongs in a behavior module under `spl/` (or
  `metadata/`), not in the derive.
- **`derive/tests/deny_lang_path.rs`** — no literal `quasar_lang::` may appear
  in emitter source. Generated code resolves the runtime crate through
  `derive/src/krate.rs::lang_path()` (interpolated as `#krate`), so a consumer
  rename can never be defeated by a hard-coded path. `krate.rs` is the only
  sanctioned home for the name.

## 4. The extension seam

Plugins extend Quasar through one stable trait pair in
`lang/src/account_behavior.rs`: **`AccountBehavior<T>`** and
**`BehaviorArgsBuilder`**. A behavior group `#[account(foo(a = x))]` lowers,
protocol-neutrally, to `foo::Args::builder().a(x)` plus
`<foo::Behavior as AccountBehavior<T>>`. A plugin module exports `Args`, an
`ArgsBuilder` (`build_init` / `build_check` / `build_exit`), and a `Behavior`
unit struct. `spl/` and `metadata/` are nothing but implementations of this
surface plus their CPI traits (`TokenCpi`, `MetadataCpi`, …), which reach the
runtime only through `AsAccountView`. Nothing SPL-specific crosses back into the
derive.

## 5. Where checks live

A given invariant is enforced at exactly one layer, chosen for the earliest
point it can fail:

| Layer | Fires at | Example | Where |
|-------|----------|---------|-------|
| Macro-expansion error | derive expansion | `` `init` cannot be used with `Migration<From, To>` `` | `derive/src/accounts/resolve/rules.rs` |
| Emitted `const` assert | consumer compile (monomorphization) | a behavior's `RUN_AFTER_INIT` requires `init` on the field; header/layout size checks | `derive/src/accounts/emit/parse.rs`, `.../ix_args.rs` |
| Type-system bound | consumer compile (trait solving) | `AccountLoad: StaticView` supertrait gates the `repr(transparent)` unchecked cast | `lang/src/account_load.rs` |
| Runtime check | on-chain execution | owner / signer / writable / discriminator / address | `lang/src/checks/` |
| Whole-program idl-build + lint | build / CI | discriminator-collision and `after_init` gates across the whole program | `idl/src/lint/rules.rs` (`P001`–`P011`) |

## 6. Internal vs stable

- **Deliberately internal** (churns; do not depend on it): every IR type —
  `FieldSemantics`, `AccountsPlanTyped` / `FieldPlan` (`resolve/specs.rs`),
  `SchemaIR` / `LayoutClass` (`schema_ir.rs`). Reference the *phase files*, not
  these types.
- **Stable surface**: the attribute grammar (`#[account]` / `#[instruction]` /
  `#[program]` / `#[event]` / `#[error_code]` and their directives), the emitted
  IDL (`quasar-idl/1.0`), and `AccountBehavior` + `BehaviorArgsBuilder`.

---

See also: `CONTRIBUTING.md` (diagnostic goldens, CU sensitivity),
`derive/src/accounts/mod.rs` (pipeline summary),
`lang/src/account_behavior.rs` (plugin contract).
