# Cross-Program Invocation (CPI)

Quasar's CPI system uses const generics to keep account arrays and instruction data entirely on the stack. No heap allocation, no intermediate instruction view. The raw `sol_invoke_signed_c` syscall is called directly with pre-built account arrays.

## `CpiCall<const ACCTS, const DATA>`

The central CPI type. Account count and data buffer size are const generics, so the entire struct lives on the stack with sizes known at compile time:

```rust
pub struct CpiCall<'a, const ACCTS: usize, const DATA: usize> {
    program_id: &'a Address,
    accounts: [InstructionAccount<'a>; ACCTS],
    cpi_accounts: [RawCpiAccount<'a>; ACCTS],
    data: [u8; DATA],
}
```

- `program_id` -- the target program to invoke
- `accounts` -- instruction-level account metadata (address, signer/writable flags)
- `cpi_accounts` -- runtime account data pointers for the syscall (56 bytes each)
- `data` -- the instruction data buffer (discriminator + serialized args)

### Construction

`CpiCall::new` takes the program ID, instruction accounts, account views, and the data buffer. It builds the `RawCpiAccount` array from the views using `MaybeUninit` for zero-initialization overhead:

```rust
let cpi_accounts = {
    let mut arr = core::mem::MaybeUninit::<[RawCpiAccount<'a>; ACCTS]>::uninit();
    let ptr = arr.as_mut_ptr() as *mut RawCpiAccount<'a>;
    let mut i = 0;
    while i < ACCTS {
        unsafe { core::ptr::write(ptr.add(i), RawCpiAccount::from_view(views[i])) };
        i += 1;
    }
    unsafe { arr.assume_init() }
};
```

### Invocation

Three invocation methods:

```rust
// No signers
cpi_call.invoke()?;

// Single PDA signer (most common)
cpi_call.invoke_signed(&seeds)?;

// Multiple PDA signers
cpi_call.invoke_with_signers(&[signer1, signer2])?;
```

All three call `invoke_inner`, which passes the data directly to `sol_invoke_signed_c`:

```rust
fn invoke_inner(&self, signers: &[Signer]) -> ProgramResult {
    let result = unsafe {
        invoke_raw(
            self.program_id,
            self.accounts.as_ptr(),
            ACCTS,
            self.data.as_ptr(),
            DATA,
            self.cpi_accounts.as_ptr(),
            ACCTS,
            signers,
        )
    };
    if result == 0 { Ok(()) } else { Err(ProgramError::from(result)) }
}
```

## `BufCpiCall<const ACCTS, const MAX>`

A variant of `CpiCall` for instructions with variable-length serialized data. Where `CpiCall<ACCTS, DATA>` uses a compile-time-exact `DATA` size, `BufCpiCall<ACCTS, MAX>` allocates a stack buffer of `MAX` bytes but tracks the actual data length at runtime:

```rust
pub struct BufCpiCall<'a, const ACCTS: usize, const MAX: usize> {
    program_id: &'a Address,
    accounts: [InstructionAccount<'a>; ACCTS],
    cpi_accounts: [RawCpiAccount<'a>; ACCTS],
    data: [u8; MAX],
    data_len: usize,  // runtime-tracked, passed to syscall
}
```

### When to Use

Use `BufCpiCall` when the instruction data contains variable-length fields -- Borsh strings, optional vectors, or any data whose byte count depends on runtime arguments. The Metaplex Token Metadata CPI uses `BufCpiCall` because name, symbol, and URI are variable-length strings.

Use `CpiCall` when the instruction data is fixed-size (all fields have known byte counts at compile time). Most SPL Token instructions fall into this category.

### Construction and Invocation

```rust
BufCpiCall::new(program_id, accounts, views, data, data_len)
```

`data_len` must be `<= MAX` -- the constructor panics otherwise.

Invocation is identical to `CpiCall`:

```rust
buf_cpi_call.invoke()?;
buf_cpi_call.invoke_signed(&seeds)?;
buf_cpi_call.invoke_with_signers(&[signer1, signer2])?;
```

The only difference from `CpiCall` is that `invoke_inner` passes `data_len` (not `MAX`) to `sol_invoke_signed_c`, so only the active bytes are sent to the target program.

### Test Helpers

On non-SBF targets (tests, native builds), two inspection methods are available:

```rust
let data: &[u8] = buf_cpi_call.instruction_data();      // active bytes only
let len: usize = buf_cpi_call.instruction_data_len();
```

## `RawCpiAccount` Layout

Each CPI account is represented as a 56-byte `#[repr(C)]` struct. The layout is verified at compile time:

```rust
#[repr(C)]
pub(crate) struct RawCpiAccount<'a> {
    address: *const Address,     // 8 bytes (pointer)
    lamports: *const u64,        // 8 bytes (pointer)
    data_len: u64,               // 8 bytes
    data: *const u8,             // 8 bytes (pointer)
    owner: *const Address,       // 8 bytes (pointer)
    rent_epoch: u64,             // 8 bytes
    is_signer: u8,               // 1 byte
    is_writable: u8,             // 1 byte
    executable: u8,              // 1 byte
    _pad: [u8; 5],               // 5 bytes padding
    _lifetime: PhantomData<&'a AccountView>,
}

const _: () = assert!(core::mem::size_of::<RawCpiAccount>() == 56);
const _: () = assert!(core::mem::align_of::<RawCpiAccount>() == 8);
```

`RawCpiAccount::from_view` builds this struct from an `AccountView` by reading fields from the underlying `RuntimeAccount`. The `is_signer`, `is_writable`, and `executable` flags are extracted with a single `copy_nonoverlapping` of 3 bytes from the `RuntimeAccount` header:

```rust
impl<'a> RawCpiAccount<'a> {
    pub(crate) fn from_view(view: &'a AccountView) -> Self {
        let raw = view.account_ptr();
        unsafe {
            let mut account = RawCpiAccount {
                address: &(*raw).address,
                lamports: &(*raw).lamports,
                data_len: (*raw).data_len,
                data: (raw as *const u8).add(core::mem::size_of::<RuntimeAccount>()),
                owner: &(*raw).owner,
                rent_epoch: 0,
                is_signer: 0,
                is_writable: 0,
                executable: 0,
                _pad: [0u8; 5],
                _lifetime: PhantomData,
            };
            core::ptr::copy_nonoverlapping(
                (raw as *const u8).add(1),
                &mut account.is_signer as *mut u8,
                3,
            );
            account
        }
    }
}
```

## Raw Syscall

Under the hood, `invoke_raw` constructs a `CInstruction` (on SBF targets) and calls `sol_invoke_signed_c` directly:

```rust
#[repr(C)]
struct CInstruction<'a> {
    program_id: *const Address,
    accounts: *const InstructionAccount<'a>,
    accounts_len: u64,
    data: *const u8,
    data_len: u64,
}
```

On non-SBF targets (tests, native builds), `invoke_raw` returns 0 (success) as a no-op.

## System Program CPI

The `SystemProgram` type provides typed CPI methods. Each method returns a `CpiCall` with the exact const-generic sizes:

### `create_account`

Creates a new account. Returns `CpiCall<2, 52>` -- 2 accounts (payer + new account), 52-byte data buffer (4-byte instruction index + 8-byte lamports + 8-byte space + 32-byte owner).

```rust
system_program.create_account(payer, new_account, lamports, space, &owner)
    .invoke_signed(&seeds)?;
```

The data buffer is constructed with `MaybeUninit` and direct pointer writes:

```rust
let data = unsafe {
    let mut buf = core::mem::MaybeUninit::<[u8; 52]>::uninit();
    let ptr = buf.as_mut_ptr() as *mut u8;
    core::ptr::copy_nonoverlapping(0u32.to_le_bytes().as_ptr(), ptr, 4);       // instruction index
    core::ptr::copy_nonoverlapping(lamports.to_le_bytes().as_ptr(), ptr.add(4), 8);
    core::ptr::copy_nonoverlapping(space.to_le_bytes().as_ptr(), ptr.add(12), 8);
    core::ptr::copy_nonoverlapping(owner.as_ref().as_ptr(), ptr.add(20), 32);
    buf.assume_init()
};
```

### `create_account_with_minimum_balance`

Convenience method that calculates rent-exempt lamports automatically. Pass `Some(&rent)` to reuse an already-fetched Rent sysvar, or `None` to fetch via syscall.

```rust
system_program.create_account_with_minimum_balance(payer, account, space, &owner, None)?
    .invoke()?;
```

### `transfer`

Transfers SOL between accounts. Returns `CpiCall<2, 12>` -- 2 accounts, 12-byte data (4-byte instruction index + 8-byte lamports).

```rust
system_program.transfer(from, to, amount).invoke()?;
```

### `assign`

Assigns an account to a new owner program. Returns `CpiCall<1, 36>` -- 1 account, 36-byte data (4-byte instruction index + 32-byte owner).

```rust
system_program.assign(account, &new_owner).invoke()?;
```

All system program methods also exist as free functions (`cpi::system::create_account`, `cpi::system::transfer`, `cpi::system::assign`) that take raw `&AccountView` references instead of `&impl AsAccountView`.

## PDA Seed Reconstruction

The `#[derive(Accounts)]` macro generates a `Bumps` struct for each instruction context that captures account addresses at parse time and provides `*_seeds()` methods for PDA seed reconstruction.

### How Seeds Work

When you declare PDA seeds:

```rust
#[account(seeds = [b"escrow", maker], bump)]
pub escrow: &'info mut Initialize<EscrowAccount>,
```

The macro generates:

1. A `bump` field on the bumps struct (e.g., `MakeBumps { escrow: u8 }`)
2. An address capture for each account reference in the seed list
3. A `escrow_seeds()` method that reconstructs the seed array

### The Bumps Struct

For the `Make` instruction with:

```rust
#[account(seeds = [b"escrow", maker], bump)]
pub escrow: &'info mut Initialize<EscrowAccount>,
```

The generated bumps struct looks like:

```rust
#[derive(Copy, Clone)]
pub struct MakeBumps {
    pub escrow: u8,
    // internal: captured address of `maker`
}
```

### Seed Reconstruction Methods

The `*_seeds()` method returns a fixed-size `[Seed; N]` array -- no heap allocation, no re-derivation:

```rust
impl MakeBumps {
    pub fn escrow_seeds(&self) -> [Seed; 3] {
        [
            Seed::from(b"escrow"),
            Seed::from(&self.escrow_maker_addr),  // captured address
            Seed::from(core::slice::from_ref(&self.escrow)),  // bump byte
        ]
    }
}
```

### Usage in CPI

```rust
pub fn make_escrow(&mut self, receive: u64, bumps: &MakeBumps) -> Result<(), ProgramError> {
    let seeds = bumps.escrow_seeds();

    EscrowAccount { /* ... */ }
        .init_signed(
            self.escrow,
            self.maker.to_account_view(),
            Some(&**self.rent),
            &[quasar_core::cpi::Signer::from(&seeds)],
        )
}
```

### `find_program_address` vs `create_program_address`

- **`bump` (no value)** -- uses `find_program_address` syscall to discover the bump. More expensive (~tens of thousands of CU) but necessary when the bump is not yet known (e.g., during `Initialize`).
- **`bump = expr`** -- uses `create_program_address` with the provided bump. Cheaper (~1,500 CU) because it skips the search loop. Use when the bump is stored in the account data.

```rust
// First time: discover bump
#[account(seeds = [b"escrow", maker], bump)]
pub escrow: &'info mut Initialize<EscrowAccount>,

// Subsequent: use stored bump
#[account(seeds = [b"escrow", maker], bump = escrow.bump)]
pub escrow: &'info mut Account<EscrowAccount>,
```

### PDA Functions

Quasar provides three PDA functions in `quasar_core::pda`:

**`create_program_address(seeds, program_id)`** -- creates a PDA from seeds. On SBF, the `Seed` slice passes directly to the syscall with zero conversion because `Seed`'s `#[repr(C)]` layout (`*const u8, u64`) matches the `&[u8]` fat pointer layout expected by the syscall.

**`find_program_address(seeds, program_id)`** -- finds a valid PDA and its bump seed. Same seed-native approach.

**`find_program_address_const(seeds, program_id)`** -- compile-time PDA derivation using `const_crypto` for const-compatible SHA-256 hashing and Ed25519 off-curve evaluation. Suitable for `const` contexts.

## Signer Types

CPI signing uses two types from `solana_instruction_view::cpi`:

- **`Seed`** -- a single seed component (`*const u8, u64` on SBF). Created from byte slices, addresses, or single bytes.
- **`Signer`** -- a set of seeds that together derive a PDA. Created from a `&[Seed]` slice.

```rust
// Single PDA signer
let seeds = bumps.escrow_seeds();  // [Seed; N]
cpi_call.invoke_signed(&seeds)?;   // wraps in single Signer

// Multiple PDA signers
let signer1 = Signer::from(&seeds1);
let signer2 = Signer::from(&seeds2);
cpi_call.invoke_with_signers(&[signer1, signer2])?;
```

## Complete Example: Escrow Take

The `Take` instruction demonstrates the full CPI flow -- PDA-signed token transfer, token account close, and escrow close:

```rust
#[derive(Accounts)]
pub struct Take<'info> {
    pub taker: &'info mut Signer,
    #[account(
        has_one = maker,
        has_one = maker_ta_b,
        constraint = escrow.receive > 0,
        seeds = [b"escrow", maker],
        bump = escrow.bump
    )]
    pub escrow: &'info mut Account<EscrowAccount>,
    pub maker: &'info mut UncheckedAccount,
    // ... other accounts ...
    pub token_program: &'info TokenProgram,
}

impl<'info> Take<'info> {
    pub fn withdraw_tokens_and_close(&mut self, bumps: &TakeBumps) -> Result<(), ProgramError> {
        let seeds = bumps.escrow_seeds();

        // PDA-signed transfer: escrow authority transfers vault tokens to taker
        self.token_program
            .transfer(
                self.vault_ta_a,
                self.taker_ta_a,
                self.escrow,         // PDA authority
                self.vault_ta_a.amount(),
            )
            .invoke_signed(&seeds)?;

        // PDA-signed close: close vault and reclaim lamports
        self.vault_ta_a
            .close(self.token_program, self.taker, self.escrow)
            .invoke_signed(&seeds)
    }
}
```
