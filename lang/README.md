# quasar-lang

The `no_std`, zero-copy runtime for building Solana programs with Quasar. It
provides account views, validation, CPI construction, events, sysvars, errors,
and the macros used by application programs.

## Typed calls to another program

Enable the `declare-program` feature and point `declare_program!` at a Quasar
IDL. The macro generates typed CPI helpers and fixed-layout account snapshots
at compile time:

```rust
use quasar_lang::{declare_program, prelude::*};

declare_program!(vault, "idls/vault.json");

let state = vault::VaultState::read_account(vault_account)?;
let call = vault::deposit(vault_program, vault_account, authority, amount);
```

`read_account` checks the foreign program owner, minimum account length, and
discriminator before decoding. Fixed structs, nested structs, and fixed arrays
are supported. Compact layouts and other dynamic fields are rejected at compile
time instead of being decoded ambiguously.

- [Quick start](https://quasar-lang.com/docs/getting-started/quickstart)
- [API documentation](https://docs.rs/quasar-lang/0.1.0)

Licensed under Apache-2.0 or MIT, at your option.
