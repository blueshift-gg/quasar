# SPL Token Integration

The `quasar-spl` crate provides zero-copy account types and CPI methods for the SPL Token program and Token-2022 (Token Extensions) program. All token operations return `CpiCall` values with compile-time-known sizes -- no heap allocation.

## Account Types

### Single-Owner Types

These types validate that the account is owned by exactly one token program:

| Type | Owner check | Deref target | Size |
|------|-------------|-------------|------|
| `Account<Token>` | SPL Token only | `TokenAccountState` | 165 bytes |
| `Account<Mint>` | SPL Token only | `MintAccountState` | 82 bytes |
| `Account<Token2022>` | Token-2022 only | `TokenAccountState` | 165 bytes |
| `Account<Mint2022>` | Token-2022 only | `MintAccountState` | 82 bytes |

```rust
pub vault: &'info Account<Token>,
pub mint: &'info Account<Mint>,
```

Single-owner types intentionally do **not** implement the `Owner` trait. This prevents access to `Account<T>::close()` (direct lamport drain), which would always fail at runtime because the calling program does not own token/mint accounts -- the SPL Token program does. Use the CPI-based `TokenClose` trait instead.

The `impl_single_owner!` macro implements `CheckOwner`, `AccountCheck`, and `ZeroCopyDeref` for each type:

```rust
pub struct Token;
impl_single_owner!(Token, SPL_TOKEN_ID, TokenAccountState);

pub struct Mint;
impl_single_owner!(Mint, SPL_TOKEN_ID, MintAccountState);
```

### Interface Types (Multi-Owner)

`InterfaceAccount<T>` accepts accounts owned by either SPL Token or Token-2022. The same marker types used with `Account<T>` work here:

| Type | Owner check | Deref target |
|------|-------------|-------------|
| `InterfaceAccount<Token>` | SPL Token **or** Token-2022 | `TokenAccountState` |
| `InterfaceAccount<Mint>` | SPL Token **or** Token-2022 | `MintAccountState` |

```rust
// Accepts either SPL Token or Token-2022
pub vault: &'info InterfaceAccount<Token>,
pub mint: &'info InterfaceAccount<Mint>,
```

The base account layout (first 165 bytes for token accounts, first 82 bytes for mints) is identical for both programs. Both interface types deref to the same state structs as their single-owner counterparts:

```rust
// Same field access regardless of which program owns the account
let mint = ctx.accounts.vault.mint();
let amount = ctx.accounts.vault.amount();
```

`InterfaceAccount<T>` validates ownership by checking against both SPL Token and Token-2022 program IDs in `from_account_view`:

```rust
let owner = unsafe { view.owner() };
if !keys_eq(owner, &SPL_TOKEN_ID) && !keys_eq(owner, &TOKEN_2022_ID) {
    return Err(ProgramError::IllegalOwner);
}
```

## Program Types

### `TokenProgram`

Validates that the account address matches the SPL Token program ID and that the account is executable:

```rust
pub token_program: &'info TokenProgram,
```

Defined via `define_account!` with executable and address checks:

```rust
define_account!(pub struct TokenProgram => [checks::Executable, checks::Address]);

impl Program for TokenProgram {
    const ID: Address = Address::new_from_array(SPL_TOKEN_BYTES);
}
```

### `Token2022Program`

Same as `TokenProgram` but validates against the Token-2022 program ID:

```rust
pub token_program: &'info Token2022Program,
```

### `TokenInterface`

Accepts either SPL Token or Token-2022. Validates that the account is executable and its address matches one of the two token program IDs:

```rust
pub token_program: &'info TokenInterface,
```

`TokenInterface` is `#[repr(transparent)]` over `AccountView` and performs its own validation in `from_account_view`:

```rust
pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
    if !view.executable() {
        return Err(ProgramError::InvalidAccountData);
    }
    if view.address() != &SPL_TOKEN_ID && view.address() != &TOKEN_2022_ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(unsafe { &*(view as *const AccountView as *const Self) })
}
```

All three program types (`TokenProgram`, `Token2022Program`, `TokenInterface`) implement the `TokenCpi` trait and expose the same set of CPI methods.

## Zero-Copy State Structs

### `TokenAccountState` (165 bytes)

`#[repr(C)]` struct with alignment 1. Compile-time assertions verify both size and alignment:

```rust
#[repr(C)]
pub struct TokenAccountState {
    mint: Address,                // 32 bytes
    owner: Address,               // 32 bytes
    amount: [u8; 8],              // u64 LE
    delegate_flag: [u8; 4],       // COption tag
    delegate: Address,            // 32 bytes
    state: u8,                    // 0=uninitialized, 1=initialized, 2=frozen
    is_native: [u8; 4],           // COption tag
    native_amount: [u8; 8],       // u64 LE
    delegated_amount: [u8; 8],    // u64 LE
    close_authority_flag: [u8; 4],// COption tag
    close_authority: Address,     // 32 bytes
}

const _ASSERT_TOKEN_ACCOUNT_LEN: () = assert!(TokenAccountState::LEN == 165);
const _ASSERT_TOKEN_ACCOUNT_ALIGN: () = assert!(core::mem::align_of::<TokenAccountState>() == 1);
```

Accessor methods:

```rust
let mint: &Address = account.mint();
let owner: &Address = account.owner();
let amount: u64 = account.amount();
let delegate: Option<&Address> = account.delegate();
let is_native: bool = account.is_native();
let native_amount: Option<u64> = account.native_amount();
let delegated_amount: u64 = account.delegated_amount();
let close_authority: Option<&Address> = account.close_authority();
let is_initialized: bool = account.is_initialized();
let is_frozen: bool = account.is_frozen();
```

Optional fields (`delegate`, `close_authority`, `native_amount`) use COption encoding -- a 4-byte tag where `[1, 0, 0, 0]` means `Some`. The `_unchecked` variants skip the tag check:

```rust
let delegate: &Address = account.delegate_unchecked();
let close_authority: &Address = account.close_authority_unchecked();
```

### `MintAccountState` (82 bytes)

```rust
#[repr(C)]
pub struct MintAccountState {
    mint_authority_flag: [u8; 4],  // COption tag
    mint_authority: Address,       // 32 bytes
    supply: [u8; 8],               // u64 LE
    decimals: u8,
    is_initialized: u8,
    freeze_authority_flag: [u8; 4],// COption tag
    freeze_authority: Address,     // 32 bytes
}

const _ASSERT_MINT_LEN: () = assert!(MintAccountState::LEN == 82);
const _ASSERT_MINT_ALIGN: () = assert!(core::mem::align_of::<MintAccountState>() == 1);
```

Accessor methods:

```rust
let authority: Option<&Address> = mint.mint_authority();
let supply: u64 = mint.supply();
let decimals: u8 = mint.decimals();
let is_initialized: bool = mint.is_initialized();
let freeze_authority: Option<&Address> = mint.freeze_authority();
```

Both state structs use raw byte arrays for integer fields (`[u8; 8]` instead of `u64`) to maintain alignment 1. The accessor methods convert via `u64::from_le_bytes`.

## CPI Methods

The `TokenCpi` trait defines all token CPI methods. It is implemented by `TokenProgram`, `Token2022Program`, and `TokenInterface`. Every method returns a `CpiCall` with compile-time-known sizes.

### `transfer`

Transfer tokens between accounts. Returns `CpiCall<3, 9>`.

```rust
self.token_program.transfer(
    self.maker_ta_a,    // from
    self.vault_ta_a,    // to
    self.maker,         // authority
    amount,
).invoke()?;
```

Data layout: `[3 (opcode), amount (8 bytes LE)]`.

### `transfer_checked`

Transfer with decimal verification. Returns `CpiCall<4, 10>`.

```rust
self.token_program.transfer_checked(
    from, mint, to, authority,
    amount, decimals,
).invoke()?;
```

Data layout: `[12 (opcode), amount (8 bytes LE), decimals (1 byte)]`.

### `mint_to`

Mint tokens to an account. Returns `CpiCall<3, 9>`.

```rust
self.token_program.mint_to(mint, to, authority, amount).invoke()?;
```

### `burn`

Burn tokens from an account. Returns `CpiCall<3, 9>`.

```rust
self.token_program.burn(from, mint, authority, amount).invoke()?;
```

### `approve`

Approve a delegate to transfer tokens. Returns `CpiCall<3, 9>`.

```rust
self.token_program.approve(source, delegate, authority, amount).invoke()?;
```

### `revoke`

Revoke a delegate's authority. Returns `CpiCall<2, 1>`.

```rust
self.token_program.revoke(source, authority).invoke()?;
```

### `close_account`

Close a token account and reclaim lamports. Returns `CpiCall<3, 1>`.

```rust
self.token_program.close_account(account, destination, authority)
    .invoke_signed(&seeds)?;
```

### `sync_native`

Sync the lamport balance of a native SOL token account. Returns `CpiCall<1, 1>`.

```rust
self.token_program.sync_native(token_account).invoke()?;
```

### `initialize_account3`

Initialize a token account (opcode 18). Does not require the Rent sysvar account -- saves one account in the CPI. Returns `CpiCall<2, 33>`.

```rust
self.token_program.initialize_account3(account, mint, &owner).invoke()?;
```

The account must already be allocated with the correct size (165 bytes).

### `initialize_mint2`

Initialize a mint (opcode 20). Does not require the Rent sysvar account. Returns `CpiCall<1, 67>`.

```rust
self.token_program.initialize_mint2(mint, decimals, &mint_authority, freeze_authority)
    .invoke()?;
```

The account must already be allocated with the correct size (82 bytes). `freeze_authority` is `Option<&Address>`.

## Initialization Patterns

### `InitToken` Trait

Extension trait on `Initialize<T>` for token account types. Chains `SystemProgram::create_account` followed by `InitializeAccount3` in two CPIs.

```rust
self.vault_ta_a.init(
    self.system_program,
    self.maker,          // payer
    self.token_program,
    self.mint_a,         // mint
    self.escrow.address(), // owner
    Some(&**self.rent),  // or None for syscall
)?;
```

Implemented for:
- `Initialize<Token>`
- `Initialize<Token2022>`
- `Initialize<InterfaceAccount<Token>>`

#### `init_if_needed`

Conditionally initializes. Checks `owner == system_program` to determine if the account needs initialization. When the account already exists, validates:

1. The account is owned by SPL Token or Token-2022 (prevents passing accounts from arbitrary programs)
2. Data length is at least 165 bytes
3. The account is initialized (state != 0)
4. The mint matches the expected mint address
5. The owner matches the expected owner address

```rust
self.vault_ta_a.init_if_needed(
    self.system_program,
    self.maker,
    self.token_program,
    self.mint_a,
    self.escrow.address(),
    Some(&**self.rent),  // or None
)?;
```

### `InitMint` Trait

Extension trait on `Initialize<T>` for mint account types. Chains `SystemProgram::create_account` followed by `InitializeMint2` in two CPIs.

```rust
self.new_mint.init(
    self.system_program,
    self.payer,
    self.token_program,
    6,                        // decimals
    self.authority.address(), // mint authority
    None,                     // no freeze authority
    None,                     // fetch rent via syscall
)?;
```

Implemented for:
- `Initialize<Mint>`
- `Initialize<Mint2022>`
- `Initialize<InterfaceAccount<Mint>>`

#### `init_if_needed` (Mint)

Same pattern as token accounts. When the account already exists, validates:

1. Owner is SPL Token or Token-2022
2. Data length is at least 82 bytes
3. The mint is initialized
4. The mint authority matches the expected value

## Associated Token Accounts (ATA)

The `quasar-spl` crate provides types, CPI builders, and address derivation functions for the SPL Associated Token Account program.

### Types

| Type | Purpose |
|------|---------|
| `AssociatedTokenProgram` | Program account; validates executable + address matches ATA program ID |
| `AssociatedToken` | Account marker; validates owner is SPL Token; derefs to `TokenAccountState` |

`AssociatedToken` works with `Account<AssociatedToken>` (SPL Token only) or `InterfaceAccount<AssociatedToken>` (SPL Token or Token-2022).

### Address Derivation

ATA addresses are derived from `seeds = [wallet, token_program, mint]` against the ATA program ID:

```rust
// SPL Token (default)
let (address, bump) = get_associated_token_address(wallet, mint);

// Explicit token program (for Token-2022)
let (address, bump) = get_associated_token_address_with_program(wallet, mint, token_program);

// Const-compatible (off-chain / const contexts)
let (address, bump) = get_associated_token_address_const(wallet, mint);
let (address, bump) = get_associated_token_address_with_program_const(wallet, mint, token_program);
```

### CPI Builders

Two free functions build ATA creation CPIs, both returning `CpiCall<6, 1>`:

```rust
// Fails if the ATA already exists
ata_create(ata_program, payer, ata, wallet, mint, system_program, token_program)
    .invoke()?;

// No-ops if the ATA already exists
ata_create_idempotent(ata_program, payer, ata, wallet, mint, system_program, token_program)
    .invoke()?;
```

### `InitAssociatedToken` Trait

Extension trait on `Initialize<AssociatedToken>` providing `.init()` and `.init_if_needed()`:

```rust
// Create ATA -- fails if it already exists
self.new_ata.init(
    self.payer,
    self.wallet,
    self.mint,
    self.system_program,
    self.token_program,
    self.ata_program,
)?;

// Create ATA if needed -- validates existing account
self.new_ata.init_if_needed(
    self.payer,
    self.wallet,
    self.mint,
    self.system_program,
    self.token_program,
    self.ata_program,
)?;
```

`init_if_needed` checks the account owner: if owned by the system program, calls `create_idempotent`; otherwise validates the token account data (mint and authority match).

### Standalone Validation

```rust
validate_ata(view, wallet, mint, token_program)?;
```

Derives the expected ATA address, checks it matches the account, and validates the token account data.

## Metaplex Token Metadata

The `quasar-spl` crate provides zero-copy types and CPI functions for the Metaplex Token Metadata program. Variable-length fields (name, symbol, URI) use `BufCpiCall` for heap-free serialization.

### Types

| Type | Purpose | Deref target |
|------|---------|-------------|
| `MetadataProgram` | Program account; validates executable + address | -- |
| `MetadataAccount` | Metadata account marker; validates key byte = 4 | `MetadataPrefix` |
| `MasterEditionAccount` | Master edition marker; validates key byte = 6 | `MasterEditionPrefix` |

### State Accessors

`MetadataPrefix` (`#[repr(C)]`, 65 bytes):

```rust
let key: u8 = metadata.key();                    // must be 4 (KEY_METADATA_V1)
let update_authority: &Address = metadata.update_authority();
let mint: &Address = metadata.mint();
```

`MasterEditionPrefix` (`#[repr(C)]`, 18 bytes):

```rust
let key: u8 = edition.key();                     // must be 6 (KEY_MASTER_EDITION_V2)
let supply: u64 = edition.supply();
let max_supply: Option<u64> = edition.max_supply();
```

These are prefix structs -- they provide zero-copy access to the fixed-layout header fields. The remaining metadata fields (name, symbol, URI, creators, etc.) are Borsh-encoded and not exposed via zero-copy accessors.

### CPI Functions

The `MetadataCpi` trait (implemented by `MetadataProgram`) provides CPI builders for all Metaplex Token Metadata instructions:

**Variable-length (return `BufCpiCall`)**:

| Method | Accounts | Buffer | Notes |
|--------|----------|--------|-------|
| `create_metadata_accounts_v3` | 7 | 512 | name, symbol, URI, seller_fee, is_mutable |
| `update_metadata_accounts_v2` | 2 | 512 | All fields optional |

**Fixed-length (return `CpiCall`)**:

| Method | Accounts | Data | Notes |
|--------|----------|------|-------|
| `create_master_edition_v3` | 9 | 10 | max_supply: `Option<u64>` |
| `mint_new_edition_from_master_edition_via_token` | 14 | 9 | edition number |
| `sign_metadata` | 2 | 1 | Creator verification |
| `remove_creator_verification` | 2 | 1 | Undo `sign_metadata` |
| `update_primary_sale_happened_via_token` | 3 | 1 | |
| `verify_collection` | 6 | 1 | |
| `verify_sized_collection_item` | 6 | 1 | |
| `unverify_collection` | 5 | 1 | |
| `unverify_sized_collection_item` | 6 | 1 | |
| `approve_collection_authority` | 6 | 1 | |
| `revoke_collection_authority` | 5 | 1 | |
| `set_and_verify_collection` | 7 | 1 | |
| `set_and_verify_sized_collection_item` | 7 | 1 | |
| `freeze_delegated_account` | 4 | 1 | |
| `thaw_delegated_account` | 4 | 1 | |
| `burn_nft` | 6 | 1 | |
| `burn_edition_nft` | 10 | 1 | |
| `set_collection_size` | 4 | 9 | |
| `set_token_standard` | 4 | 1 | |
| `bubblegum_set_collection_size` | 4 | 9 | |
| `utilize` | 6 | 1 | |

Example -- creating metadata and master edition:

```rust
// Create metadata via BufCpiCall (variable-length name/symbol/uri)
self.metadata_program.create_metadata_accounts_v3(
    self.metadata,
    self.mint,
    self.mint_authority,
    self.payer,
    self.update_authority,
    self.system_program,
    b"My NFT",         // name (max 32 bytes)
    b"MNFT",           // symbol (max 10 bytes)
    b"https://...",    // URI (max 200 bytes)
    500,               // seller_fee_basis_points
    true,              // is_mutable
    true,              // update_authority_is_signer
).invoke()?;

// Create master edition via CpiCall (fixed-length)
self.metadata_program.create_master_edition_v3(
    self.master_edition,
    self.mint,
    self.update_authority,
    self.mint_authority,
    self.payer,
    self.metadata,
    self.token_program,
    self.system_program,
    Some(100),         // max_supply (None = unlimited)
).invoke()?;
```

### Derive Attributes

The `#[derive(Accounts)]` macro supports `mint::*`, `metadata::*`, and `master_edition::*` attributes for declarative mint + metadata initialization. All attributes go on the **mint** field:

```rust
#[derive(Accounts)]
pub struct CreateNft<'info> {
    pub payer: &'info mut Signer,
    pub mint_authority: &'info Signer,
    #[account(
        init,
        mint::decimals = 0,
        mint::authority = mint_authority,
        metadata::name = b"My NFT",
        metadata::symbol = b"MNFT",
        metadata::uri = b"https://example.com/nft.json",
        metadata::seller_fee_basis_points = 500,
        metadata::is_mutable = true,
        master_edition::max_supply = 0,
    )]
    pub mint: &'info mut Account<Mint>,
    pub metadata: &'info mut UncheckedAccount,
    pub master_edition: &'info mut UncheckedAccount,
    pub metadata_program: &'info MetadataProgram,
    pub token_program: &'info TokenProgram,
    pub system_program: &'info SystemProgram,
    pub rent: &'info UncheckedAccount,
}
```

The generated code:
1. Creates the mint account (SystemProgram `create_account` + TokenProgram `InitializeMint2`)
2. CPIs into Metaplex `create_metadata_accounts_v3` to create the metadata PDA
3. CPIs into Metaplex `create_master_edition_v3` to create the edition PDA (if `master_edition::max_supply` is present)

Metadata and master edition accounts are `UncheckedAccount` because they are created by the Metaplex CPI — they don't exist at parse time.

**`mint::*` attributes** (required for mint initialization):

| Attribute | Type | Description |
|-----------|------|-------------|
| `mint::decimals` | `Expr` | Token decimals (0 for NFTs) |
| `mint::authority` | `Ident` | Field name of the mint authority signer |
| `mint::freeze_authority` | `Ident` | (Optional) Field name of the freeze authority |

**`metadata::*` attributes** (all required if any is present):

| Attribute | Type | Description |
|-----------|------|-------------|
| `metadata::name` | `Expr` | NFT name (max 32 bytes) |
| `metadata::symbol` | `Expr` | Token symbol (max 10 bytes) |
| `metadata::uri` | `Expr` | Metadata JSON URI (max 200 bytes) |
| `metadata::seller_fee_basis_points` | `Expr` | Royalty in basis points (e.g. 500 = 5%) |
| `metadata::is_mutable` | `Expr` | Whether metadata can be updated |

**`master_edition::*` attributes**:

| Attribute | Type | Description |
|-----------|------|-------------|
| `master_edition::max_supply` | `Expr` | Maximum editions (0 = unique 1/1) |

Requirements:
- `mint::decimals` requires `mint::authority`
- `metadata::*` requires `init` or `init_if_needed` and `mint::decimals`
- `metadata::name`, `metadata::symbol`, and `metadata::uri` must all be present if any is
- `master_edition::max_supply` requires both `init` and all `metadata::*` attributes
- The struct must include `MetadataProgram`, `mint_authority` (or `authority`), `payer`, `TokenProgram`, `SystemProgram`, and `rent` fields
- The struct must include a `metadata` field (`UncheckedAccount`) for metadata CPI
- The struct must include a `master_edition` or `edition` field (`UncheckedAccount`) for master edition CPI

### Init Traits

For manual (non-derive) initialization:

**`InitMetadata`** (on `Initialize<MetadataAccount>`):

```rust
self.metadata.init(
    self.metadata_program,
    self.mint,
    self.mint_authority,
    self.payer,
    self.update_authority,
    self.system_program,
    b"My NFT",      // name
    b"MNFT",        // symbol
    b"https://...", // uri
    500,            // seller_fee_basis_points
    true,           // is_mutable
)?;
```

**`InitMasterEdition`** (on `Initialize<MasterEditionAccount>`):

```rust
self.master_edition.init(
    self.metadata_program,
    self.mint,
    self.update_authority,
    self.mint_authority,
    self.payer,
    self.metadata,
    self.token_program,
    self.system_program,
    Some(100),      // max_supply (None = unlimited)
)?;
```

Both traits also provide `init_signed()` variants for PDA authorities.

## Closing Token Accounts

### `TokenClose` Trait

Extension trait on `Account<T>` that returns a `CpiCall<3, 1>` for closing via the token program. The caller controls `.invoke()` vs `.invoke_signed()`:

```rust
self.vault_ta_a
    .close(self.token_program, self.maker, self.escrow)
    .invoke_signed(&seeds)?;
```

Internally, `TokenClose::close` delegates to `token_program.close_account`:

```rust
pub trait TokenClose: AsAccountView + Sized {
    fn close<'a>(
        &'a self,
        token_program: &'a impl TokenCpi,
        destination: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
    ) -> CpiCall<'a, 3, 1> {
        token_program.close_account(self, destination, authority)
    }
}
```

Implemented for all token/mint account types:
- `Account<Token>`
- `Account<Token2022>`
- `Account<Mint>`
- `Account<Mint2022>`

This is distinct from `Account<T>::close()` (the direct lamport drain), which is only available for program-owned accounts (`T: Owner`). Token/mint accounts are owned by the token program, so they must be closed via CPI.

## Program ID Constants

Token program addresses are defined as raw byte arrays and exposed as `Address` values. On BPF targets they use `static` (placed in `.rodata`); on non-BPF targets they use `const`:

```rust
#[cfg(target_arch = "bpf")]
pub static SPL_TOKEN_ID: Address = Address::new_from_array(SPL_TOKEN_BYTES);
#[cfg(not(target_arch = "bpf"))]
pub const SPL_TOKEN_ID: Address = Address::new_from_array(SPL_TOKEN_BYTES);

#[cfg(target_arch = "bpf")]
pub static TOKEN_2022_ID: Address = Address::new_from_array(TOKEN_2022_BYTES);
#[cfg(not(target_arch = "bpf"))]
pub const TOKEN_2022_ID: Address = Address::new_from_array(TOKEN_2022_BYTES);
```

The `static` vs `const` distinction on BPF ensures the addresses live in read-only memory at a fixed location rather than being inlined at every use site.

## Complete Example: Escrow Make

The `Make` instruction demonstrates token account initialization, escrow creation, and token deposit:

```rust
#[derive(Accounts)]
pub struct Make<'info> {
    pub maker: &'info mut Signer,
    #[account(seeds = [b"escrow", maker], bump)]
    pub escrow: &'info mut Initialize<EscrowAccount>,
    pub mint_a: &'info Account<Mint>,
    pub mint_b: &'info Account<Mint>,
    pub maker_ta_a: &'info mut Account<Token>,
    pub maker_ta_b: &'info mut Initialize<Token>,
    pub vault_ta_a: &'info mut Initialize<Token>,
    pub rent: &'info Sysvar<Rent>,
    pub token_program: &'info TokenProgram,
    pub system_program: &'info SystemProgram,
}

impl<'info> Make<'info> {
    pub fn init_accounts(&self) -> Result<(), ProgramError> {
        let rent = Some(&**self.rent);

        // Initialize vault token account (owned by escrow PDA)
        self.vault_ta_a.init_if_needed(
            self.system_program,
            self.maker,
            self.token_program,
            self.mint_a,
            self.escrow.address(),
            rent,
        )?;

        // Initialize maker's token-B account
        self.maker_ta_b.init_if_needed(
            self.system_program,
            self.maker,
            self.token_program,
            self.mint_b,
            self.maker.address(),
            rent,
        )
    }

    pub fn deposit_tokens(&mut self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .transfer(self.maker_ta_a, self.vault_ta_a, self.maker, amount)
            .invoke()
    }
}
```
