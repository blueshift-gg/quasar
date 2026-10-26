# Metaplex Token Metadata

`quasar-metadata` integrates the Metaplex Token Metadata program with Quasar
0.2. It depends only on `quasar-lang` at runtime. The official
`mpl-token-metadata` 5.1.1 SDK is a test dependency used to check instruction
bytes, account ordering, signer flags, writable flags, and PDA derivation.

## Accounts and constraints

`MetadataAccount` is `InterfaceAccount<Metadata>`; `MasterEditionAccount` is
`InterfaceAccount<MasterEdition>`. Both check the program owner, discriminant
and prefix encoding, and accept trailing account data. Metadata exposes its
65-byte prefix (`key`, `update_authority`, `mint`), as the previous integration
did. It does not deserialize or validate the name, creators or other trailing
metadata fields. Master editions expose `supply` and Borsh `Option<u64>`
`max_supply`, including a ten-byte encoding when the option is `None`.

```rust,ignore
use quasar_lang::prelude::*;
use quasar_metadata::prelude::*;

#[derive(Accounts)]
pub struct Read {
    pub mint: UncheckedAccount,
    pub authority: Signer,
    #[account(metadata(mint = mint, update_authority = authority))]
    pub metadata: MetadataAccount,
    #[account(master_edition(mint = mint, metadata = metadata))]
    pub edition: MasterEditionAccount,
}
```

The constraints also check canonical PDA addresses. `metadata(...)` can infer
the mint from the account; `master_edition(...)` requires `mint`, since an
edition does not store it. Both accept `program = metadata_program` to check
an explicitly supplied executable program account.

## Creation and CPI

Declare unallocated metadata and edition PDAs as writable `UncheckedAccount`
fields. Call `init_metadata` and `init_master_edition` to consume those handles
and return fully validated typed accounts. The `_signed` variants accept the
rewrite's `SignerSeeds` for a PDA authority. Metaplex allocates its own PDAs;
these helpers do not ask the calling program to sign for them or create them
with the System Program. Initialization order remains metadata, then edition.

See [the SBF consumer](../tests/metaplex/src/lib.rs) for a complete creation
handler, including reuse of one signer as payer and both authorities.

All 23 previous instruction helpers are available as functions under
`quasar_metadata::instructions`: metadata creation/update/signing, master
edition creation/printing, collection verification and authority management,
freeze/thaw, burns, token standard, primary sale, collection size, and utilize.
They take `&Program<MetadataProgram>` and return `Cpi` (or `CpiWithDataLen`
for variable-length encodings), with `.invoke()` and `.invoke_signed(...)`.

Writable account parameters take `&mut`; the payer is borrowed shared because
the program only debits its lamports. Typed writable accounts become unusable
until `.reload()` rechecks them after CPI. Do not reload an account that was
closed. These helpers conservatively retain all account checks; they do not
claim additional check elision through `ENFORCES`.

`instructions::Data` supplies UTF-8 name, symbol, URI and seller fee. The
builders enforce the Metaplex byte limits (32/10/200) and fee limit (10,000).
As in the previous API, DataV2 creators, collection and uses are encoded as
`None`; updating DataV2 replaces those fields. Optional trailing delegation
records, burn collection metadata and utilize burner accounts are not exposed
by these helpers. Rent accounts are supplied explicitly where supported.

## Migrating from the removed integration

| Previous API | Rewrite API |
| --- | --- |
| `Account<MetadataAccount>` | `MetadataAccount` (alias for `InterfaceAccount<Metadata>`) |
| `Account<MasterEditionAccount>` | `MasterEditionAccount` (alias for `InterfaceAccount<MasterEdition>`) |
| `MetadataCpi` methods | Functions in `instructions`, with explicit mutable borrows |
| Metadata/master-edition `init` behavior attributes | Writable `UncheckedAccount` plus `init_metadata` / `init_master_edition` in the handler |
| Raw signer seeds | `quasar_lang::pda::SignerSeeds` and `_signed` helpers |
| Separate optional update strings | `Option<instructions::Data>`: the full data update or none |
| PDA helpers returning tuples | Fallible PDA helpers returning `Result<(Address, u8)>` |

Stored account formats and the Metaplex program ID are unchanged; no on-chain
data migration is needed. Source code must migrate to the new wrappers and
borrowing rules. Instruction account lists now follow the official SDK,
including the System Program on collection approval and the token, associated
token, System Program and rent accounts on utilize. `max_supply: None` sends
exactly its two-byte instruction encoding.

## Verification

- `cargo test -p quasar-metadata`: layout, validation, PDA, serialization,
  SDK comparisons, and compile-pass/fail tests.
- `make test-miri` / `make test-miri-strict`: the codec, account and instruction
  tests, plus the runtime's CPI pointer and variable-data tests.
- `cargo kani -p quasar-metadata`: bounded writer, integer encoding,
  discriminant and option-tag proofs. These do not prove Metaplex's program.
- `make sbf` then `cargo test -p metaplex`: account constraints executed under
  Mollusk in normal and debug SBF builds. The creation handler is compiled for
  SBF; these tests do not load or execute the external Metaplex binary.

The workspace CI and release workflows include these checks and package
`quasar-metadata` for publication after `quasar-lang`.
