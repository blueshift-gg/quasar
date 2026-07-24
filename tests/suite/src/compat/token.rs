//! Token account factories, ported from the previous `quasar-svm` backend.
//!
//! These build pre-initialized SPL Token / Token-2022 `Account`s so tests can
//! seed mints and token accounts without CPI-ing the token program. The
//! packing is identical to the previous backend (same layout, same rent).

use {
    super::{Account, Pubkey},
    solana_program_pack::Pack,
    solana_rent::Rent,
    spl_token_interface::state::{Account as SplTokenAccount, Mint as SplMint},
};

// Re-exports for convenience: the suite refers to these as
// `token::{TokenAccount, Mint}`.
pub use spl_token_interface::state::{Account as TokenAccount, Mint};

/// Create a system-owned account.
pub fn create_keyed_system_account(address: &Pubkey, lamports: u64) -> Account {
    Account {
        address: *address,
        lamports,
        data: vec![],
        owner: solana_sdk_ids::system_program::ID,
        executable: false,
    }
}

/// Create a pre-initialized mint account owned by a specific token program.
#[inline(always)]
pub fn create_keyed_mint_account_with_program(
    address: &Pubkey,
    mint: &SplMint,
    token_program_id: &Pubkey,
) -> Account {
    let mut data = vec![0u8; SplMint::LEN];
    SplMint::pack(*mint, &mut data).unwrap();
    Account {
        address: *address,
        lamports: Rent::default().minimum_balance(SplMint::LEN),
        data,
        owner: *token_program_id,
        executable: false,
    }
}

/// Create a pre-initialized token account owned by a specific token program.
#[inline(always)]
pub fn create_keyed_token_account_with_program(
    address: &Pubkey,
    token: &SplTokenAccount,
    token_program_id: &Pubkey,
) -> Account {
    let mut data = vec![0u8; SplTokenAccount::LEN];
    SplTokenAccount::pack(*token, &mut data).unwrap();
    Account {
        address: *address,
        lamports: Rent::default().minimum_balance(SplTokenAccount::LEN),
        data,
        owner: *token_program_id,
        executable: false,
    }
}
