use {
    quasar_svm::{token, Account, Pubkey, Rent, SPL_TOKEN_PROGRAM_ID},
    spl_token::state::{Account as TokenAccount, AccountState, Mint},
};

/// Create a system-owned account with the supplied balance.
pub fn system_account(address: Pubkey, lamports: u64) -> Account {
    token::create_keyed_system_account(&address, lamports)
}

/// Create an empty system-owned account, suitable for an init target.
pub fn empty_account(address: Pubkey) -> Account {
    system_account(address, 0)
}

/// Create a rent-exempt program-owned account containing `data`.
pub fn program_account(address: Pubkey, owner: Pubkey, data: Vec<u8>) -> Account {
    Account {
        address,
        lamports: Rent::default().minimum_balance(data.len()),
        data,
        owner,
        executable: false,
    }
}

/// Create an initialized SPL Token mint with zero supply.
pub fn mint_account(address: Pubkey, authority: Pubkey, decimals: u8) -> Account {
    mint_account_with_supply(address, authority, 0, decimals)
}

/// Create an initialized SPL Token mint with an explicit supply.
pub fn mint_account_with_supply(
    address: Pubkey,
    authority: Pubkey,
    supply: u64,
    decimals: u8,
) -> Account {
    let mint = Mint {
        mint_authority: Some(authority).into(),
        supply,
        decimals,
        is_initialized: true,
        freeze_authority: None.into(),
    };
    token::create_keyed_mint_account(&address, &mint)
}

/// Create an initialized SPL Token account.
pub fn token_account(address: Pubkey, mint: Pubkey, owner: Pubkey, amount: u64) -> Account {
    let token_account = TokenAccount {
        mint,
        owner,
        amount,
        state: AccountState::Initialized,
        ..TokenAccount::default()
    };
    token::create_keyed_token_account(&address, &token_account)
}

/// Create an initialized associated token account and derive its address.
pub fn associated_token_account(wallet: Pubkey, mint: Pubkey, amount: u64) -> Account {
    token::create_keyed_associated_token_account_with_program(
        &wallet,
        &mint,
        amount,
        &SPL_TOKEN_PROGRAM_ID,
    )
}
