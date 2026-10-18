//! The escrow's accounts: the maker holds 1,000 of mint A and the taker
//! 1,000 of mint B.

use escrow::Offer;
use quasar_lang::sysvar::{Rent, SysvarData};
use quasar_lang::{Address, client::Instruction, wrapper::ProgramId};
use quasar_spl::token::Token;
use quasar_test::Test;
use solana_program_pack::Pack;
use spl_token_interface::state::{Account as TokenAccount, AccountState, Mint};

const SYSTEM: Address = Address::new_from_array([0; 32]);
const MAKER: Address = Address::new_from_array([1; 32]);
const TAKER: Address = Address::new_from_array([2; 32]);
const MINT_A: Address = Address::new_from_array([3; 32]);
const MINT_B: Address = Address::new_from_array([4; 32]);
const MAKER_TA_A: Address = Address::new_from_array([5; 32]);
const TAKER_TA_B: Address = Address::new_from_array([6; 32]);
const MAKER_TA_B: Address = Address::new_from_array([7; 32]);
const VAULT_TA_A: Address = Address::new_from_array([8; 32]);
const TAKER_TA_A: Address = Address::new_from_array([9; 32]);

fn escrow() -> Address {
    Address::find_program_address(&[b"escrow", MAKER.as_ref()], &escrow::ID).0
}

pub fn prepare(t: Test) -> Test {
    prepare_on(t, mollusk_svm_programs_token::token::ELF)
}

/// As `prepare`, with `token` as the Token program.
pub fn prepare_on(mut t: Test, token: &[u8]) -> Test {
    t.add_elf(Token::ID, token);
    for user in [MAKER, TAKER] {
        t.set(user, SYSTEM, 10_000_000_000, vec![]);
    }
    for mint in [MINT_A, MINT_B] {
        let mut data = vec![0; Mint::LEN];
        Mint { supply: 1_000, decimals: 6, is_initialized: true, ..Mint::default() }
            .pack_into_slice(&mut data);
        t.set(mint, Token::ID, t.rent(Mint::LEN), data);
    }
    for (key, mint, owner) in [(MAKER_TA_A, MINT_A, MAKER), (TAKER_TA_B, MINT_B, TAKER)] {
        let mut data = vec![0; TokenAccount::LEN];
        let state = AccountState::Initialized;
        TokenAccount { mint, owner, amount: 1_000, state, ..TokenAccount::default() }
            .pack_into_slice(&mut data);
        t.set(key, Token::ID, t.rent(TokenAccount::LEN), data);
    }
    for created in [escrow(), MAKER_TA_B, VAULT_TA_A, TAKER_TA_A] {
        t.set(created, SYSTEM, 0, vec![]);
    }
    t
}

pub fn make() -> Instruction {
    let accounts = (
        MAKER,
        escrow(),
        MINT_A,
        MINT_B,
        MAKER_TA_A,
        MAKER_TA_B,
        VAULT_TA_A,
        Rent::ID,
        Token::ID,
        SYSTEM,
    );
    escrow::make::instruction(accounts, Offer { deposit: 400, receive: 300 })
}

pub fn take() -> Instruction {
    let accounts = (
        TAKER,
        escrow(),
        MAKER,
        MINT_A,
        MINT_B,
        TAKER_TA_A,
        TAKER_TA_B,
        MAKER_TA_B,
        VAULT_TA_A,
        Rent::ID,
        Token::ID,
        SYSTEM,
    );
    escrow::take::instruction(accounts, ())
}

pub fn refund() -> Instruction {
    let accounts = (MAKER, escrow(), MINT_A, MAKER_TA_A, VAULT_TA_A, Rent::ID, Token::ID, SYSTEM);
    escrow::refund::instruction(accounts, ())
}
