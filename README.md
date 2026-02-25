<h1 align="center">
  <code>quasar</code>
</h1>
<p align="center">
  Write optimized Solana programs without thinking too much.
</p>

## Overview

Quasar is a `no_std` Solana program framework that brings everything the ecosystem has learned about CU optimization — from [Pinocchio](https://github.com/anza-xyz/pinocchio/blob/main/README.md) programs to zero-copy tricks — into a declarative macro system with Anchor-level developer experience.

It provides `#[account]`, `#[derive(Accounts)]`, `#[instruction]`, `#[program]`, `#[event]` — but the generated code is zero-copy and zero-allocation, operating directly on the SVM input buffer with no deserialization step.

The framework is a workspace of six crates:

| Crate | Path | Purpose |
|-------|------|---------|
| `quasar` | `quasar/` | Facade crate — the single dependency for programs |
| `quasar-core` | `core/` | Account types, CPI builder, events, sysvars, error handling |
| `quasar-derive` | `derive/` | Proc macros for accounts, instructions, programs, events, errors |
| `quasar-pod` | `pod/` | Alignment-1 integer types — usable independently of the framework |
| `quasar-spl` | `spl/` | SPL Token program CPI and zero-copy `TokenAccountState` |
| `quasar-idl` | `idl/` | IDL generator with discriminator collision detection |

```toml
[dependencies]
quasar = "0.1"
```

This re-exports `quasar-core` and `quasar-spl` (via the `spl` feature, on by default).

## Account Types

Every field in a `#[derive(Accounts)]` struct uses a wrapper type that defines what validations are performed at parse time.

### `Signer`

Checks that the account has the `is_signer` flag set. Used for transaction payers, authority accounts, and any account that must prove it authorized the transaction.

```rust
pub maker: &'info mut Signer,
```

### `Account<T>`

Validates owner and discriminator. `T` implements `Owner` (checks `account.owner == program_id`) and `AccountCheck` (checks discriminator bytes and data length). After parsing, `Account<T>` dereferences to the zero-copy companion struct — field access is a pointer cast, not deserialization.

```rust
pub escrow: &'info mut Account<EscrowAccount>,
pub vault: &'info Account<TokenAccount>,
```

### `Initialize<T>`

For accounts that don't exist yet. Skips owner and discriminator validation — the account will be created via `init()`. The mutable variant checks `is_writable`.

```rust
pub escrow: &'info mut Initialize<EscrowAccount>,
```

### `UncheckedAccount`

No validation. Used for accounts where you handle validation yourself — PDAs owned by other programs, lamport-only destinations, etc.

```rust
pub maker: &'info mut UncheckedAccount,
```

### `SystemProgram` / `TokenProgram`

Program account wrappers that validate the account address matches the expected program ID. Provide typed CPI methods.

```rust
pub system_program: &'info SystemProgram,
pub token_program: &'info TokenProgram,
```

### `Sysvar<T>`

Sysvar account access. Validates the account address matches `T::ID`. Dereferences directly to the sysvar data type via `borrow_unchecked` — no RefCell overhead.

```rust
pub rent: &'info Sysvar<Rent>,
pub clock: &'info Sysvar<Clock>,
```

Access sysvar fields through `Deref`:

```rust
let lamports = self.rent.minimum_balance_unchecked(data_len);
let slot = self.clock.slot;
```

### Mutability

`&'info mut` references automatically assert the account's `is_writable` flag. `&'info` references are read-only — no writable check.

```rust
pub payer: &'info mut Signer,        // writable + signer
pub config: &'info Account<Config>,   // read-only, owner + discriminator
```

## State Definition

### `#[account]`

Defines on-chain state. The macro generates a zero-copy companion struct (`EscrowAccountZc`) where `u64` becomes `PodU64` (alignment 1), a `Deref` impl for direct field access, discriminator validation, space calculation, owner traits, and `init()` / `init_signed()` methods with re-initialization protection.

```rust
#[account(discriminator = 1)]
pub struct EscrowAccount {
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub maker_ta_b: Address,
    pub receive: u64,
    pub bump: u8,
}
```

Discriminators are explicit integers — no sha256 hashing. All-zero discriminators are rejected at compile time (they're indistinguishable from uninitialized account data).

Multi-byte discriminators:

```rust
#[account(discriminator = [1, 2])]
pub struct MyAccount { ... }
```

### Type Mapping

Account fields are stored as alignment-1 Pod types in the ZC companion struct:

| Rust type | ZC type | Size |
|-----------|---------|------|
| `u8` / `i8` | `u8` / `i8` | 1 |
| `u16` | `PodU16` | 2 |
| `u32` | `PodU32` | 4 |
| `u64` | `PodU64` | 8 |
| `u128` | `PodU128` | 16 |
| `i16` - `i128` | `PodI16` - `PodI128` | 2-16 |
| `bool` | `PodBool` | 1 |
| `Address` | `Address` | 32 |

Pod types use `#[repr(transparent)]` over `[u8; N]`. Arithmetic operators use wrapping semantics in release builds for CU efficiency. Use `checked_add`, `checked_sub`, `checked_mul`, `checked_div` when overflow matters.

### Initialization

Create accounts with `init()` or `init_signed()` (for PDA-owned accounts):

```rust
EscrowAccount {
    maker: *self.maker.address(),
    mint_a: *self.maker_ta_a.mint(),
    receive,
    bump: bumps.escrow,
    // ...
}
.init_signed(
    self.escrow,
    self.maker.to_account_view(),
    Some(&**self.rent),
    &[quasar_core::cpi::Signer::from(&seeds)],
)?;
```

Re-initialization protection: `init()` checks that the discriminator region is all-zero before writing. Since all-zero discriminators are banned at compile time, uninitialized data can never match a valid account.

### Closing Accounts

```rust
self.escrow.close(self.maker.to_account_view())?;
```

Transfers all lamports to the destination, reassigns to the system program, and resizes to 0.

### Realloc

```rust
self.account.realloc(new_space, payer.to_account_view(), None)?;
```

Adjusts account size and transfers lamports for rent exemption. Pass `Some(rent)` to use the account-provided Rent sysvar instead of a syscall.

## Account Directives

Directives are specified in `#[account(...)]` attributes on fields of a `#[derive(Accounts)]` struct.

### `seeds` + `bump`

PDA derivation. `seeds` accepts byte slices and account references (account references automatically resolve to their address).

```rust
// find_program_address — bump is auto-discovered and stored in the bumps struct
#[account(seeds = [b"escrow", maker], bump)]
pub escrow: &'info mut Initialize<EscrowAccount>,

// create_program_address — cheaper when bump is already known
#[account(seeds = [b"escrow", maker], bump = escrow.bump)]
pub escrow: &'info mut Account<EscrowAccount>,
```

The bumps struct (`MakeBumps`, `TakeBumps`, etc.) captures account addresses at parse time and exposes `*_seeds()` methods that return fixed-size `[Seed; N]` arrays — PDA seeds are reconstructed without re-derivation:

```rust
let seeds = bumps.escrow_seeds();
cpi_call.invoke_signed(&seeds)?;
```

### `has_one`

Cross-account validation. Checks that a field in the validated account matches the address of another account in the struct:

```rust
#[account(has_one = maker, has_one = maker_ta_b)]
pub escrow: &'info mut Account<EscrowAccount>,
pub maker: &'info mut Signer,
pub maker_ta_b: &'info mut Account<TokenAccount>,
```

Generates: `require!(escrow.maker == *maker.address(), QuasarError::HasOneMismatch)`.

### `constraint`

Arbitrary boolean expression. Any valid Rust expression that evaluates to `bool`:

```rust
#[account(constraint = escrow.receive > 0)]
pub escrow: &'info mut Account<EscrowAccount>,
```

Generates: `require!(escrow.receive > 0, QuasarError::ConstraintViolation)`.

## Dynamic Data

For variable-length fields — strings and arrays that can change size after initialization.

### Defining Dynamic Fields

Use `String<'a, N>` for variable-length UTF-8 strings and `Vec<'a, T, N>` for variable-length arrays. `N` is the maximum byte length (for String) or element count (for Vec). The struct must have a lifetime parameter.

```rust
#[account(discriminator = 5)]
pub struct Profile<'a> {
    pub owner: Address,         // fixed field
    pub score: u64,             // fixed field
    pub name: String<'a, 32>,   // up to 32 bytes
    pub tags: Vec<'a, Address, 10>,  // up to 10 addresses
}
```

`String` and `Vec` are marker types (`PhantomData`). The macro transforms them — `String<'a, 32>` becomes `&'a str`, `Vec<'a, Address, 10>` becomes `&'a [Address]` in the generated code.

### Memory Layout

```
[discriminator][ZC header: fixed fields + PodU16 length descriptors][variable tail: packed data]
```

For the `Profile` example above:

```
[disc: 1 byte][owner: 32 bytes][score: 8 bytes (PodU64)][name_len: 2 bytes (PodU16)][tags_count: 2 bytes (PodU16)][name bytes...][tag elements...]
```

The ZC header has a fixed size regardless of current field values. A `PodU16` descriptor per dynamic field stores the current length/count. The variable tail packs all dynamic data contiguously.

### Rules

- Fixed fields must precede all dynamic fields
- Vec element types must be fixed-size, alignment-1 types (no nested `String`/`Vec`)
- The struct must have a lifetime parameter

All three rules are enforced at compile time.

### Reading Dynamic Fields

Individual accessors — each re-casts the ZC header:

```rust
let name: &str = account.name();
let tags: &[Address] = account.tags();
```

Batch accessor — single ZC cast, one linear scan. O(N) instead of O(N per field):

```rust
let fields = account.dynamic_fields();
// fields: ProfileDynamicFields { name: &str, tags: &[Address] }
```

### Writing Dynamic Fields

Individual setters — each triggers realloc + memmove for subsequent fields:

```rust
account.set_name(&payer, "alice")?;
account.set_tags(&payer, &[addr1, addr2])?;
```

Batch setter — stack buffer, one realloc, zero memmove. Use `Option` to selectively update fields:

```rust
// Update name, keep existing tags
account.set_dynamic_fields(&payer, Some("alice"), None)?;

// Update both
account.set_dynamic_fields(&payer, Some("bob"), Some(&[addr1]))?;
```

The batch setter copies all field data (old for `None`, new for `Some`) into a `[0u8; MAX_TAIL]` stack buffer, does one realloc, and one `copy_from_slice` back. No memmove overlap issues.

### In-Place Mutation (Vec only)

Mutate existing Vec elements without realloc (element count stays the same):

```rust
account.tags_mut()[0] = new_address;
```

### Dynamic Instruction Arguments

Instruction arguments support `String<N>` and `Vec<T, N>` (no lifetime — instruction data is immutable):

```rust
#[instruction(discriminator = 0)]
pub fn create_profile(ctx: Ctx<CreateProfile>, name: String<32>, tags: Vec<Address, 10>) -> Result<(), ProgramError> {
    // name: &str, tags: &[Address] — already parsed from instruction data
}
```

Instruction data layout: `[discriminator][ZC header with PodU16 descriptors][variable tail]`. Bounds and max-length checks are generated automatically. String data is validated as UTF-8.

## Remaining Accounts

Access accounts beyond those declared in the `#[derive(Accounts)]` struct. Zero allocation in the dispatch hot path — the `RemainingAccounts` struct is constructed lazily.

```rust
let remaining = ctx.remaining_accounts();

// Iterate sequentially (builds index for O(1) dup resolution)
for account in remaining.iter() {
    // account: AccountView
}

// Random access by index (O(n) — walks from start)
let third = remaining.get(2);

// Check if there are remaining accounts
if remaining.is_empty() { ... }
```

Uses a boundary pointer (end of accounts region in the SVM buffer) instead of a count. The iterator uses a `MaybeUninit<[AccountView; 64]>` cache for O(1) duplicate account resolution — same pattern as the entrypoint's declared accounts parser.

## Instructions

### `#[instruction]`

Marks a function as a program instruction. Discriminators are explicit integers.

```rust
#[instruction(discriminator = 0)]
pub fn make(ctx: Ctx<Make>, deposit: u64, receive: u64) -> Result<(), ProgramError> {
    // ...
}
```

The first parameter must be `ctx: Ctx<T>` where `T` implements `Accounts`. Additional parameters are deserialized from instruction data through a generated zero-copy struct with a compile-time alignment assertion.

### `#[program]`

Wraps a module to generate the entrypoint, instruction dispatch, self-CPI event handler, and off-chain client module:

```rust
declare_id!("22222222222222222222222222222222222222222222");

#[program]
mod my_program {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn initialize(ctx: Ctx<Initialize>) -> Result<(), ProgramError> { ... }

    #[instruction(discriminator = 1)]
    pub fn update(ctx: Ctx<Update>, value: u64) -> Result<(), ProgramError> { ... }
}
```

Discriminator collisions and `0xFF` conflicts (reserved for self-CPI events) are caught at compile time.

### Return Data

Instructions that return a type (not `()`) automatically call `sol_set_return_data`:

```rust
#[instruction(discriminator = 3)]
pub fn query(ctx: Ctx<Query>) -> Result<PodU64, ProgramError> {
    Ok(PodU64::from(42))
}
```

The return type must have alignment 1 (Pod types). A compile-time assertion enforces this.

## CPI

Cross-program invocation uses `CpiCall<'a, const ACCTS: usize, const DATA: usize>` — account count and data size are const generics, everything lives on the stack. No heap allocation, no intermediate instruction view.

### SPL Token

```rust
// Transfer
self.token_program.transfer(
    self.maker_ta_a,    // from
    self.vault_ta_a,    // to
    self.maker,         // authority
    amount,
).invoke()?;

// PDA-signed transfer
self.token_program.transfer(from, to, pda_authority, amount)
    .invoke_signed(&seeds)?;

// Close account
self.token_program.close_account(account, destination, authority)
    .invoke_signed(&seeds)?;
```

### System Program

```rust
// Create account
system_program.create_account(payer, new_account, lamports, space, &owner)
    .invoke_signed(&seeds)?;

// Transfer SOL
system_program.transfer(from, to, amount).invoke()?;
```

### Raw CPI

Under the hood, `invoke()` calls `sol_invoke_signed_c` directly with pre-built `RawCpiAccount` arrays (56 bytes per account, layout verified at compile time):

```rust
pub struct CpiCall<'a, const ACCTS: usize, const DATA: usize> {
    program_id: &'a Address,
    accounts: [InstructionAccount<'a>; ACCTS],
    cpi_accounts: [RawCpiAccount<'a>; ACCTS],
    data: [u8; DATA],
}
```

## Events

### Defining Events

```rust
#[event(discriminator = 0)]
pub struct MakeEvent {
    pub escrow: Address,
    pub maker: Address,
    pub deposit: u64,
    pub receive: u64,
}
```

Event serialization is `memcpy` from the `#[repr(C)]` struct. A compile-time assertion guarantees no padding exists.

### Emission

Two paths:

```rust
// Log-based (~100 CU) — spoofable by any program
emit!(MakeEvent { escrow: addr, maker: addr, deposit: 100, receive: 200 });

// Self-CPI (~1,000 CU) — not spoofable, validates event authority PDA
program.emit_event(&event, &event_authority)?;
```

Self-CPI events use a `0xFF`-prefixed instruction data payload. The callee validates the event authority PDA (`seeds = ["__event_authority"]`).

## Error Handling

### `#[error_code]`

Define program errors starting from a base code:

```rust
#[error_code]
pub enum MyError {
    InsufficientFunds = 6000,
    InvalidAuthority,     // 6001
    AccountExpired,       // 6002
}
```

Implements `From<MyError> for ProgramError`. Use with `require!` and `require_eq!` macros:

```rust
require!(amount > 0, MyError::InsufficientFunds);
require_eq!(authority, expected, MyError::InvalidAuthority);
```

### Framework Errors

`QuasarError` covers framework-level failures:

| Code | Error | Cause |
|------|-------|-------|
| 3000 | `AccountNotInitialized` | Account data empty or too small |
| 3001 | `AccountAlreadyInitialized` | Discriminator region non-zero during init |
| 3002 | `InvalidPda` | PDA derivation failed |
| 3003 | `InvalidSeeds` | Seed verification failed |
| 3004 | `ConstraintViolation` | `constraint = expr` check failed |
| 3005 | `HasOneMismatch` | `has_one` address mismatch |
| 3006 | `InvalidDiscriminator` | Wrong discriminator bytes |
| 3007 | `InsufficientSpace` | Account too small for data |
| 3008 | `AccountNotRentExempt` | Below rent-exempt minimum |
| 3009 | `AccountOwnedByWrongProgram` | Owner mismatch |
| 3010 | `AccountNotMutable` | Writable check failed |
| 3011 | `AccountNotSigner` | Signer check failed |
| 3012 | `AddressMismatch` | Address constraint failed |
| 3013 | `DynamicFieldTooLong` | Dynamic field exceeds max |

## Compute Units

Both programs implement the same escrow logic and run against the same test harness:

| Instruction | Quasar | Pinocchio (hand-written) | Delta |
|-------------|--------|--------------------------|-------|
| Make        | 9,395  | 9,853                    | -458   |
| Take        | 17,789 | 17,862                   | -73    |
| Refund      | 11,930 | 12,033                   | -103   |

The codegen advantages come from decisions that are tedious to make by hand: byte-level discriminator checks instead of slice comparisons, eliding borrow tracking when the access pattern is statically known, and folding account header arithmetic at compile time.

## Building

```bash
# Build SBF binary
cargo build-sbf --manifest-path examples/escrow/Cargo.toml

# Run tests (prints CU consumption)
cargo test -p quasar-escrow -- --nocapture

# Check workspace
cargo check --workspace

# Lint
cargo clippy --workspace -- -D warnings

# Generate IDL
cargo run -p quasar-idl

# Run Miri UB tests (requires nightly)
rustup +nightly component add miri
MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
  cargo +nightly miri test -p quasar-core --test miri
```

The `examples/escrow/` directory contains the full reference implementation used for CU benchmarking. `examples/pinocchio-escrow/` contains the hand-written Pinocchio equivalent for comparison.

## Safety

Quasar uses `unsafe` for zero-copy access, raw CPI syscalls, and pointer casts. Every `unsafe` block has a `// SAFETY:` comment explaining the invariant.

### Safety Model

- **Alignment** — ZC companion structs enforce `assert!(align_of::<T>() == 1)` at compile time. Pod types use `#[repr(transparent)]` over `[u8; N]`.
- **Bounds** — Account data length is validated once during `AccountCheck::check`. Field access via `Deref` relies on that upstream check.
- **Initialization** — `init()` verifies the discriminator region is all-zero before writing. All-zero discriminators are banned at compile time.
- **Interior mutability** — `from_account_view_mut` casts `&AccountView` to `&mut Self` (`#[repr(transparent)]`). Mutations go through `AccountView`'s raw pointers to SVM memory — same pattern as Pinocchio.
- **Zero heap allocation** — `no_alloc!()` installs a global allocator that panics on any heap allocation. The entire dispatch -> parse -> CPI path is provably zero-allocation.

### Miri Validation

Every unsafe code path is tested under [Miri](https://github.com/rust-lang/miri) with Tree Borrows and symbolic alignment checking. The test suite covers 54 patterns including `& -> &mut` casts, `copy_nonoverlapping` flag extraction, `MaybeUninit` array initialization, event memcpy, CPI data construction, remaining accounts pointer arithmetic, and dynamic field UB probes (exact-size buffer boundary casts, max-capacity fields touching allocation edges, shared→mut aliasing under Tree Borrows retag, minimum-overlap memmove geometry, `from_raw_parts_mut` write-then-read aliasing, instruction data ZC cast at exact Vec boundary).

### Design Choices

- **Explicit discriminators** — developer-specified integers, not sha256 hashes. You can read the discriminator from the source code.
- **Component crate dependencies** — Quasar depends on decomposed `solana-*` component crates (e.g. `solana-address`, `solana-account-view`) instead of the monolithic `solana-program`. This reduces compile times and dependency surface.

## License

MIT
