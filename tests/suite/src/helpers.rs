use {
    mollusk_svm::Mollusk,
    quasar_svm::{
        token::{Mint, TokenAccount},
        Account, Instruction, Pubkey, QuasarSvm,
    },
    solana_address::Address,
    solana_program_pack::Pack,
    spl_token_interface::state::AccountState,
};

// ---------------------------------------------------------------------------
// SVM factories
// ---------------------------------------------------------------------------

fn deploy_artifact_so(name: &str) -> String {
    format!("../../target/deploy/{name}.so")
}

fn read_deploy_elf(name: &str) -> Vec<u8> {
    let path = deploy_artifact_so(name);
    std::fs::read(&path).unwrap_or_else(|error| {
        panic!("failed to read deploy artifact `{path}`: {error}. Run `make build-sbf` first.")
    })
}

pub fn mollusk_for_program(program_id: &Address, name: &str) -> Mollusk {
    let path = deploy_artifact_so(name);
    assert!(
        std::path::Path::new(&path).exists(),
        "missing deploy artifact `{path}`. Run `make build-sbf` first."
    );
    let base = path.trim_end_matches(".so");
    Mollusk::new(program_id, base)
}

pub fn svm_validate() -> QuasarSvm {
    let elf = read_deploy_elf("quasar_test_token_validate");
    QuasarSvm::new().with_program(&quasar_test_token_validate::ID, &elf)
}

pub fn svm_init() -> QuasarSvm {
    let elf = read_deploy_elf("quasar_test_token_init");
    QuasarSvm::new().with_program(&quasar_test_token_init::ID, &elf)
}

pub fn svm_cpi() -> QuasarSvm {
    let elf = read_deploy_elf("quasar_test_token_cpi");
    QuasarSvm::new().with_program(&quasar_test_token_cpi::ID, &elf)
}

// ---------------------------------------------------------------------------
// Program IDs
// ---------------------------------------------------------------------------

pub fn spl_token_program_id() -> Pubkey {
    quasar_svm::SPL_TOKEN_PROGRAM_ID
}

pub fn token_2022_program_id() -> Pubkey {
    quasar_svm::SPL_TOKEN_2022_PROGRAM_ID
}

pub fn ata_program_id() -> Pubkey {
    quasar_svm::SPL_ASSOCIATED_TOKEN_PROGRAM_ID
}

pub fn with_signers(mut ix: Instruction, indices: &[usize]) -> Instruction {
    for &i in indices {
        ix.accounts[i].is_signer = true;
    }
    ix
}

// ---------------------------------------------------------------------------
// Account constructors
// ---------------------------------------------------------------------------

pub fn token_account(
    address: Pubkey,
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    token_program: Pubkey,
) -> Account {
    quasar_svm::token::create_keyed_token_account_with_program(
        &address,
        &TokenAccount {
            mint,
            owner,
            amount,
            state: AccountState::Initialized,
            ..TokenAccount::default()
        },
        &token_program,
    )
}

pub fn token_account_with_delegate(
    address: Pubkey,
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    delegate: Pubkey,
    delegated_amount: u64,
    token_program: Pubkey,
) -> Account {
    quasar_svm::token::create_keyed_token_account_with_program(
        &address,
        &TokenAccount {
            mint,
            owner,
            amount,
            delegate: Some(delegate).into(),
            state: AccountState::Initialized,
            delegated_amount,
            ..TokenAccount::default()
        },
        &token_program,
    )
}

pub fn mint_account(
    address: Pubkey,
    authority: Pubkey,
    decimals: u8,
    token_program: Pubkey,
) -> Account {
    quasar_svm::token::create_keyed_mint_account_with_program(
        &address,
        &Mint {
            mint_authority: Some(authority).into(),
            supply: 1_000_000_000,
            decimals,
            is_initialized: true,
            freeze_authority: None.into(),
        },
        &token_program,
    )
}

pub fn mint_account_with_freeze(
    address: Pubkey,
    authority: Pubkey,
    decimals: u8,
    freeze_authority: Pubkey,
    token_program: Pubkey,
) -> Account {
    quasar_svm::token::create_keyed_mint_account_with_program(
        &address,
        &Mint {
            mint_authority: Some(authority).into(),
            supply: 1_000_000_000,
            decimals,
            is_initialized: true,
            freeze_authority: Some(freeze_authority).into(),
        },
        &token_program,
    )
}

pub fn signer_account(address: Pubkey) -> Account {
    quasar_svm::token::create_keyed_system_account(&address, 1_000_000)
}

pub fn rich_signer_account(address: Pubkey) -> Account {
    quasar_svm::token::create_keyed_system_account(&address, 100_000_000_000)
}

pub fn empty_account(address: Pubkey) -> Account {
    Account {
        address,
        lamports: 0,
        data: vec![],
        owner: quasar_svm::system_program::ID,
        executable: false,
    }
}

// ---------------------------------------------------------------------------
// Raw data packing (for adversarial tests)
// ---------------------------------------------------------------------------

pub fn pack_token_data(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
    let token = TokenAccount {
        mint,
        owner,
        amount,
        state: AccountState::Initialized,
        ..TokenAccount::default()
    };
    let mut data = vec![0u8; TokenAccount::LEN];
    Pack::pack(token, &mut data).unwrap();
    data
}

pub fn pack_mint_data(authority: Pubkey, decimals: u8) -> Vec<u8> {
    let mint = Mint {
        mint_authority: Some(authority).into(),
        supply: 1_000_000_000,
        decimals,
        is_initialized: true,
        freeze_authority: None.into(),
    };
    let mut data = vec![0u8; Mint::LEN];
    Pack::pack(mint, &mut data).unwrap();
    data
}

/// Raw Account with custom data — for adversarial tests (wrong owner, bad data,
/// etc.)
pub fn raw_account(address: Pubkey, lamports: u64, data: Vec<u8>, owner: Pubkey) -> Account {
    Account {
        address,
        lamports,
        data,
        owner,
        executable: false,
    }
}

// ---------------------------------------------------------------------------
// SVM factories — test-misc & test-errors
// ---------------------------------------------------------------------------

pub fn svm_misc() -> QuasarSvm {
    let elf = read_deploy_elf("quasar_test_misc");
    QuasarSvm::new().with_program(&quasar_test_misc::ID, &elf)
}

pub fn svm_errors() -> QuasarSvm {
    let elf = read_deploy_elf("quasar_test_errors");
    QuasarSvm::new().with_program(&quasar_test_errors::ID, &elf)
}

// ---------------------------------------------------------------------------
// Account constructors — test-misc state types
// ---------------------------------------------------------------------------

const SIMPLE_ACCOUNT_SIZE: usize = 42; // 1 disc + 32 addr + 8 u64 + 1 u8

/// Build raw data for SimpleAccount (disc=1).
pub fn build_simple_data(authority: Pubkey, value: u64, bump: u8) -> Vec<u8> {
    let mut data = vec![0u8; SIMPLE_ACCOUNT_SIZE];
    data[0] = 1;
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..41].copy_from_slice(&value.to_le_bytes());
    data[41] = bump;
    data
}

/// Valid SimpleAccount owned by test-misc program.
pub fn simple_account(address: Pubkey, authority: Pubkey, value: u64, bump: u8) -> Account {
    raw_account(
        address,
        1_000_000,
        build_simple_data(authority, value, bump),
        quasar_test_misc::ID,
    )
}

const MULTI_DISC_SIZE: usize = 10; // 2 disc + 8 u64

/// Build raw data for MultiDiscAccount (disc=[1,2]).
pub fn build_multi_disc_data(value: u64) -> Vec<u8> {
    let mut data = vec![0u8; MULTI_DISC_SIZE];
    data[0] = 1;
    data[1] = 2;
    data[2..10].copy_from_slice(&value.to_le_bytes());
    data
}

/// Valid MultiDiscAccount owned by test-misc program.
pub fn multi_disc_account(address: Pubkey, value: u64) -> Account {
    raw_account(
        address,
        1_000_000,
        build_multi_disc_data(value),
        quasar_test_misc::ID,
    )
}

const ERROR_TEST_ACCOUNT_SIZE: usize = 41; // 1 disc + 32 addr + 8 u64

/// Build raw data for ErrorTestAccount (disc=1).
pub fn build_error_test_data(authority: Pubkey, value: u64) -> Vec<u8> {
    let mut data = vec![0u8; ERROR_TEST_ACCOUNT_SIZE];
    data[0] = 1;
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..41].copy_from_slice(&value.to_le_bytes());
    data
}

/// Valid ErrorTestAccount owned by test-errors program.
pub fn error_test_account(address: Pubkey, authority: Pubkey, value: u64) -> Account {
    raw_account(
        address,
        1_000_000,
        build_error_test_data(authority, value),
        quasar_test_errors::ID,
    )
}

pub fn svm_heap() -> QuasarSvm {
    let elf = read_deploy_elf("quasar_test_heap");
    QuasarSvm::new().with_program(&quasar_test_heap::ID, &elf)
}

/// Account with custom lamports (for pre-funded init tests).
pub fn prefunded_account(address: Pubkey, lamports: u64) -> Account {
    Account {
        address,
        lamports,
        data: vec![],
        owner: quasar_svm::system_program::ID,
        executable: false,
    }
}
