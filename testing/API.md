# quasar-test API standard

The rules the `quasar-test` surface is held to. New methods must fit one of
the shapes below or argue for a new shape here first; a method that can be
expressed through an existing shape does not get its own name.

## The shapes

**Construction** is `new` / `try_new` / `builder` — nothing else. The blessed
path takes exactly the program id; every setup knob (config, artifact path,
crate name) lives on `QuasarTestBuilder`. No `new_with_x`, `from_x`, or
`x_for_y` constructor variants: each knob added that way multiplies the whole
constructor family.

**Nouns answer questions; verbs change the world.** A method that registers
state is a verb — `add_wallet`, `add_mint`, `add_token_account`, `add_ata`,
`fund`, `write`, `load_program` — takes `&mut self`, and returns the
address it registered. A method that only computes or reads is a noun —
`derive_pda`, `derive_ata`, `fresh_address`, `read`, `lamports` — and never
mutates. The
same word never does both. A derivation keeps the bare noun unless a
registering `add_` sibling shares it — then it takes `derive_`, so the pair
cannot be misread: `derive_ata(owner, mint)` computes, `add_ata(owner, mint,
amount)` registers, while `pda` stays bare because nothing registers PDAs.

```rust
let maker = q.add_wallet();
let mint = q.add_mint(maker);
let vault = q.derive_ata(escrow, mint);   // derive only: an init target
let funded = q.add_ata(maker, mint, 10);  // register
```

- The base form uses sensible defaults at a fresh, per-world-deterministic
  address.
- `_at(address, ...)` pins the address and exposes the family's full
  parameters; it is the maximal form, not a third naming axis.
- `_with_x(...)` exists only when `x` is the point of the scenario
  (`add_wallet_with_lamports`), never as general configuration.
- One name per concept: an `_at`/`_with` variant whose reason to exist has
  lapsed is deleted, not kept for symmetry.

**Addresses** come from declarations, not arithmetic: `pda(T::seeds(..))` /
`pda_with_bump`, `derive_ata(owner, mint)`, `fresh_address()`. Tests never hand-roll
seed bytes.

**Typed state** is the symmetric pair `read::<T>(address) -> Snapshot<T>` /
`write(address, TData { .. })`, running the framework's own validation in
both directions. `write` infers the owning type from the data value
(`AccountData::Wrapper`); a type is never named twice in one call.

**Execution** is `send` / `send_with` / `simulate`. `send` is the default and
commits; the other two are the two escape hatches (extra accounts, no
commit). All three are `#[must_use]`: an executed-but-unasserted instruction
is a silent test, and the compiler rejects it. Deliberate deviations from the
canonical call are made on the built instruction (`swap_account`,
`signed_by`), visibly, at the construction site.

**Results** are fluent asserts plus noun getters, and the two families rhyme:
`has_lamports` / `has_tokens` / `has_supply` assert what `lamports` /
`tokens` / `supply` return — on the result for post-execution values and on
the world for current state. `fails_with` takes typed custom errors;
`fails` takes a plain `ProgramError`. Asserting methods chain (`&Self`);
there are no `assert_x` doubles of fluent methods, and failure messages name
the account and the expectation.

## The boundaries

- `QuasarTest` derefs to `QuasarSvm`: the VM's own API is the ejection hatch
  and is never re-exported method-for-method. A `QuasarTest` method must add
  test vocabulary (defaults, derivation, typed access), not forwarding.
- Machinery a user never calls stays out of sight: macro-only entry points go
  through the builder, resolution helpers are private, `PROGRAM_PATH_ENV` is
  the one documented contract with the CLI.
- Panics are the assertion mechanism (this is a test crate); `try_` and
  `SetupError` exist only where a test legitimately handles failure —
  world setup.
- The prelude exports what a test file actually names: the world, the
  attribute, the two extension traits, the wire types, and the token program
  ids. Nothing speculative.
