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

pub fn spl_token_program_id() -> Pubkey {
    quasar_svm::SPL_TOKEN_PROGRAM_ID
}

pub fn token_2022_program_id() -> Pubkey {
    quasar_svm::SPL_TOKEN_2022_PROGRAM_ID
}

pub fn with_signers(mut ix: Instruction, indices: &[usize]) -> Instruction {
    for &i in indices {
        ix.accounts[i].is_signer = true;
    }
    ix
}

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

/// Raw Account with custom data for adversarial tests (wrong owner, bad data,
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

pub fn svm_misc() -> QuasarSvm {
    let elf = read_deploy_elf("quasar_test_misc");
    QuasarSvm::new().with_program(&quasar_test_misc::ID, &elf)
}

pub fn svm_errors() -> QuasarSvm {
    let elf = read_deploy_elf("quasar_test_errors");
    QuasarSvm::new().with_program(&quasar_test_errors::ID, &elf)
}

/// test-errors plus test-misc, for cross-program return-data tests.
pub fn svm_errors_with_misc() -> QuasarSvm {
    let errors_elf = read_deploy_elf("quasar_test_errors");
    let misc_elf = read_deploy_elf("quasar_test_misc");
    QuasarSvm::new()
        .with_program(&quasar_test_errors::ID, &errors_elf)
        .with_program(&quasar_test_misc::ID, &misc_elf)
}

const SIMPLE_ACCOUNT_SIZE: usize = 42; // 1 disc + 32 addr + 8 u64 + 1 u8

/// Build raw data for SimpleAccount (disc=2).
pub fn build_simple_data(authority: Pubkey, value: u64, bump: u8) -> Vec<u8> {
    let mut data = vec![0u8; SIMPLE_ACCOUNT_SIZE];
    data[0] = 2;
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

pub fn svm_raw() -> QuasarSvm {
    let elf = read_deploy_elf("quasar_test_raw");
    QuasarSvm::new().with_program(&quasar_test_raw::ID, &elf)
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

const METADATA_PROGRAM_BYTES: [u8; 32] = [
    11, 112, 101, 177, 227, 209, 124, 69, 56, 157, 82, 127, 107, 4, 195, 205, 88, 184, 108, 115,
    26, 160, 253, 181, 73, 182, 209, 188, 3, 248, 41, 70,
];

pub fn svm_metadata_validate() -> QuasarSvm {
    let elf = read_deploy_elf("quasar_test_metadata_validate");
    let mpl_elf = std::fs::read("../../tests/fixtures/mpl_token_metadata.so")
        .expect("missing mpl_token_metadata.so fixture; run `make build-sbf` first");
    QuasarSvm::new()
        .with_program(&quasar_test_metadata_validate::ID, &elf)
        .with_program(&Address::new_from_array(METADATA_PROGRAM_BYTES), &mpl_elf)
}

/// NFT mint with supply=1 and freeze_authority set (both required by Metaplex).
pub fn nft_mint_account(address: Pubkey, authority: Pubkey, token_program: Pubkey) -> Account {
    quasar_svm::token::create_keyed_mint_account_with_program(
        &address,
        &Mint {
            mint_authority: Some(authority).into(),
            supply: 1,
            decimals: 0,
            is_initialized: true,
            freeze_authority: Some(authority).into(),
        },
        &token_program,
    )
}

pub fn metadata_program_id() -> Pubkey {
    Address::new_from_array(METADATA_PROGRAM_BYTES)
}

/// Build raw metadata account data (65+ bytes).
///
/// Layout: key(1) | update_authority(32) | mint(32) | trailing zeros
pub fn build_metadata_account_data(update_authority: Pubkey, mint: Pubkey) -> Vec<u8> {
    let mut data = vec![0u8; 128]; // larger than prefix to simulate real account
    data[0] = 4; // KEY_METADATA_V1
    data[1..33].copy_from_slice(update_authority.as_ref());
    data[33..65].copy_from_slice(mint.as_ref());
    data
}

/// Build raw master edition account data (18+ bytes).
///
/// Layout: key(1) | supply(8) | max_supply_flag(1) | max_supply(8)
pub fn build_master_edition_data(supply: u64, max_supply: Option<u64>) -> Vec<u8> {
    let mut data = vec![0u8; 32];
    data[0] = 6; // KEY_MASTER_EDITION_V2
    data[1..9].copy_from_slice(&supply.to_le_bytes());
    match max_supply {
        Some(v) => {
            data[9] = 1;
            data[10..18].copy_from_slice(&v.to_le_bytes());
        }
        None => {
            data[9] = 0;
        }
    }
    data
}

/// Valid metadata account owned by Metaplex.
pub fn metadata_account(address: Pubkey, update_authority: Pubkey, mint: Pubkey) -> Account {
    raw_account(
        address,
        1_000_000,
        build_metadata_account_data(update_authority, mint),
        metadata_program_id(),
    )
}

/// Valid master edition account owned by Metaplex.
pub fn master_edition_account(address: Pubkey, supply: u64, max_supply: Option<u64>) -> Account {
    raw_account(
        address,
        1_000_000,
        build_master_edition_data(supply, max_supply),
        metadata_program_id(),
    )
}

/// Metadata program account (executable).
pub fn metadata_program_account() -> Account {
    Account {
        address: metadata_program_id(),
        lamports: 1_000_000,
        data: vec![],
        owner: Pubkey::default(), // BPF loader doesn't matter for address check
        executable: true,
    }
}
