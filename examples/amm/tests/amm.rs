//! The pool on the real SPL Token program.

use amm::{AmmError, Deposit, Fee, ID, Pool, Trade, Withdraw};
use quasar_lang::{Address, client::Instruction, wrapper::ProgramId};
use quasar_spl::token::Token;
use quasar_test::Test;
use solana_program_pack::Pack;
use spl_token_interface::state::{Account as TokenAccount, AccountState, Mint};

const SYSTEM: Address = Address::new_from_array([0; 32]);
const PAYER: Address = Address::new_from_array([1; 32]);
const ALICE: Address = Address::new_from_array([2; 32]);
const BOB: Address = Address::new_from_array([3; 32]);
const MINT_A: Address = Address::new_from_array([4; 32]);
const MINT_B: Address = Address::new_from_array([5; 32]);
const VAULT_A: Address = Address::new_from_array([6; 32]);
const VAULT_B: Address = Address::new_from_array([7; 32]);
const LP_MINT: Address = Address::new_from_array([8; 32]);

fn pool() -> Address {
    Address::find_program_address(&[b"pool", MINT_A.as_ref(), MINT_B.as_ref()], &ID).0
}

/// Each user's token account for `mint`.
fn token_account_of(owner: &Address, mint: &Address) -> Address {
    let mut bytes = owner.to_bytes();
    bytes[31] = mint.as_ref()[0];
    Address::new_from_array(bytes)
}

/// Alice and Bob each hold 1,000,000 of both mints and an empty LP token
/// account; the pool is initialized with a 0.3% fee.
fn prepare() -> Test {
    let mut t = Test::new(ID, "amm");
    t.add_elf(Token::ID, Test::spl_token_elf());
    for account in [PAYER, ALICE, BOB] {
        t.set(account, SYSTEM, 10_000_000_000, vec![]);
    }
    for mint in [MINT_A, MINT_B] {
        let mut data = vec![0; Mint::LEN];
        Mint { supply: 4_000_000, decimals: 6, is_initialized: true, ..Mint::default() }
            .pack_into_slice(&mut data);
        t.set(mint, Token::ID, t.rent(Mint::LEN), data);
        for owner in [ALICE, BOB] {
            let data = token_account(&mint, &owner, 1_000_000);
            t.set(token_account_of(&owner, &mint), Token::ID, t.rent(TokenAccount::LEN), data);
        }
    }
    for created in [pool(), VAULT_A, VAULT_B, LP_MINT] {
        t.set(created, SYSTEM, 0, vec![]);
    }
    t.run(initialize(30)).succeeds();
    for owner in [ALICE, BOB] {
        let data = token_account(&LP_MINT, &owner, 0);
        t.set(token_account_of(&owner, &LP_MINT), Token::ID, t.rent(TokenAccount::LEN), data);
    }
    t
}

fn token_account(mint: &Address, owner: &Address, amount: u64) -> Vec<u8> {
    let mut data = vec![0; TokenAccount::LEN];
    let state = AccountState::Initialized;
    TokenAccount { mint: *mint, owner: *owner, amount, state, ..TokenAccount::default() }
        .pack_into_slice(&mut data);
    data
}

fn initialize(fee_bps: u16) -> Instruction {
    let accounts = (PAYER, MINT_A, MINT_B, pool(), VAULT_A, VAULT_B, LP_MINT, Token::ID, SYSTEM);
    amm::initialize::instruction(accounts, Fee { fee_bps })
}

/// Deposit and withdraw take the same accounts.
fn liquidity(
    owner: Address,
) -> (Address, Address, Address, Address, Address, Address, Address, Address, Address) {
    let [a, b, lp] = [MINT_A, MINT_B, LP_MINT].map(|mint| token_account_of(&owner, &mint));
    (owner, pool(), LP_MINT, a, b, lp, VAULT_A, VAULT_B, Token::ID)
}

fn deposit(owner: Address, a: u64, b: u64, min_shares: u64) -> Instruction {
    amm::deposit::instruction(liquidity(owner), Deposit { max_a: a, max_b: b, min_shares })
}

fn withdraw(owner: Address, shares: u64) -> Instruction {
    amm::withdraw::instruction(liquidity(owner), Withdraw { shares, min_a: 0, min_b: 0 })
}

fn shares(t: &Test, owner: &Address) -> u64 {
    amount(t, &token_account_of(owner, &LP_MINT))
}

fn swap(owner: Address, amount_in: u64, min_out: u64, a_for_b: bool) -> Instruction {
    let accounts = (
        owner,
        pool(),
        token_account_of(&owner, &MINT_A),
        token_account_of(&owner, &MINT_B),
        VAULT_A,
        VAULT_B,
        Token::ID,
    );
    amm::swap::instruction(accounts, Trade { amount_in, min_out, a_for_b })
}

fn amount(t: &Test, account: &Address) -> u64 {
    TokenAccount::unpack(t.data(account)).expect("a token account").amount
}

#[test]
fn the_first_deposit_sets_the_price_and_earns_the_geometric_mean() {
    let mut t = prepare();

    t.run(deposit(ALICE, 1_000, 4_000, 0)).succeeds();

    assert_eq!((amount(&t, &VAULT_A), amount(&t, &VAULT_B)), (1_000, 4_000));
    assert_eq!(shares(&t, &ALICE), 2_000);
    assert_eq!(t.read::<Pool>(&pool()).lp_mint, LP_MINT);
}

#[test]
fn a_later_deposit_takes_the_pools_ratio_and_rounds_for_the_pool() {
    let mut t = prepare();
    t.run(deposit(ALICE, 1_000, 3_001, 0)).succeeds();

    // ⌊√(1,000 · 3,001)⌋ = 1,732 shares. Offering 500 and 2,000 earns 866
    // shares, limited by mint A; they cost exactly 500 of A and 1,500.5 of B,
    // which rounds up to 1,501.
    t.run(deposit(BOB, 500, 2_000, 0)).succeeds();

    assert_eq!(shares(&t, &BOB), 866);
    assert_eq!((amount(&t, &VAULT_A), amount(&t, &VAULT_B)), (1_500, 4_502));
}

#[test]
fn a_swap_pays_the_fee_into_the_pool() {
    let mut t = prepare();
    t.run(deposit(ALICE, 1_000, 1_000, 0)).succeeds();

    // 100 in, 99 after the 0.3% fee: ⌊1,000 · 99 / 1,099⌋ = 90 out.
    t.run(swap(BOB, 100, 90, true)).succeeds();

    assert_eq!((amount(&t, &VAULT_A), amount(&t, &VAULT_B)), (1_100, 910));
    assert_eq!(amount(&t, &token_account_of(&BOB, &MINT_B)), 1_000_090);
}

#[test]
fn a_swap_worse_than_its_limit_is_refused() {
    let mut t = prepare();
    t.run(deposit(ALICE, 1_000, 1_000, 0)).succeeds();

    t.run(swap(BOB, 100, 91, true)).fails(AmmError::Slippage);
    t.run(swap(BOB, 1, 0, true)).fails(AmmError::Slippage); // rounds to nothing
}

#[test]
fn a_withdrawal_returns_a_share_of_the_reserves() {
    let mut t = prepare();
    t.run(deposit(ALICE, 1_000, 1_000, 0)).succeeds();
    t.run(swap(BOB, 100, 0, true)).succeeds();

    t.run(withdraw(ALICE, 500)).succeeds();

    // Half of 1,100 and 910.
    assert_eq!((amount(&t, &VAULT_A), amount(&t, &VAULT_B)), (550, 455));
    assert_eq!(shares(&t, &ALICE), 500);
}

#[test]
fn only_an_lp_tokens_owner_withdraws_and_only_what_it_holds() {
    let mut t = prepare();
    t.run(deposit(ALICE, 1_000, 1_000, 0)).succeeds();
    t.run(deposit(BOB, 10, 10, 0)).succeeds();

    // Burning more LP tokens than held: the Token program's `InsufficientFunds`.
    t.run(withdraw(ALICE, 1_001)).fails(quasar_lang::ProgramError::Custom(1));

    // Bob naming Alice's LP tokens: they are not his.
    let mut theft = withdraw(BOB, 5);
    theft.accounts[5].pubkey = token_account_of(&ALICE, &LP_MINT);
    t.run(theft).fails(quasar_spl::token::Error::Authority);
}

#[test]
fn a_fee_of_the_whole_amount_is_refused() {
    let mut t = prepare();
    for account in [pool(), VAULT_A, VAULT_B, LP_MINT] {
        t.set(account, SYSTEM, 0, vec![]);
    }

    t.run(initialize(10_000)).fails(AmmError::Fee);
}

#[test]
fn the_idl_matches_the_committed_one() {
    let idl = amm::idl().to_json();
    quasar_test::snapshot(concat!(env!("CARGO_MANIFEST_DIR"), "/idl.json"), &idl);
}

/// The checks each instruction leaves to a call it always makes, committed:
/// a change to what is elided shows up in review.
#[test]
fn the_elisions_match_the_committed_ones() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/elisions.txt");
    quasar_test::snapshot(path, &amm::elisions());
}
