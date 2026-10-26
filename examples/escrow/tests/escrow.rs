//! The escrow on classic SPL Token, and against public Quasar master's
//! escrow, built from a pinned revision by `bench/rivals.sh`: the same
//! instructions from the same state must succeed or fail alike and leave the
//! same accounts.

use escrow::{Escrow, ID, Offer};
use quasar_lang::sysvar::{Rent, SysvarData};
use quasar_lang::{Address, ProgramError, client::Instruction, error::Error, wrapper::ProgramId};
use quasar_spl::token::{self, Token, Token2022};
use quasar_test::Test;
use solana_program_option::COption;
use solana_program_pack::Pack;
use spl_token_interface::state::{Account as TokenAccount, AccountState, Mint};

const SYSTEM: Address = Address::new_from_array([0; 32]);
const MAKER: Address = Address::new_from_array([1; 32]);
const TAKER: Address = Address::new_from_array([2; 32]);
const MINT_A: Address = Address::new_from_array([3; 32]);
const MINT_B: Address = Address::new_from_array([4; 32]);
const MAKER_TA_A: Address = Address::new_from_array([5; 32]);
const TAKER_TA_B: Address = Address::new_from_array([6; 32]);
/// Token accounts `init` creates, which sign for themselves.
const MAKER_TA_B: Address = Address::new_from_array([7; 32]);
const VAULT_TA_A: Address = Address::new_from_array([8; 32]);
const TAKER_TA_A: Address = Address::new_from_array([9; 32]);

fn escrow_of(maker: &Address) -> Address {
    Address::find_program_address(&[b"escrow", maker.as_ref()], &ID).0
}

/// The maker holds 1,000 of mint A and the taker 1,000 of mint B, all under
/// `program`: Token or Token-2022.
fn prepare(mut t: Test, program: Address) -> Test {
    let elf = if program == Token::ID {
        Test::spl_token_elf()
    } else {
        mollusk_svm_programs_token_2022::ELF
    };
    t.add_elf(program, elf);
    for user in [MAKER, TAKER] {
        t.set(user, SYSTEM, 10_000_000_000, vec![]);
    }
    for mint in [MINT_A, MINT_B] {
        let mut data = vec![0; Mint::LEN];
        Mint { supply: 1_000, decimals: 6, is_initialized: true, ..Mint::default() }
            .pack_into_slice(&mut data);
        t.set(mint, program, t.rent(Mint::LEN), data);
    }
    for (key, mint, owner) in [(MAKER_TA_A, MINT_A, MAKER), (TAKER_TA_B, MINT_B, TAKER)] {
        t.set(key, program, t.rent(TokenAccount::LEN), token_account(mint, owner, 1_000));
    }
    for created in [escrow_of(&MAKER), MAKER_TA_B, VAULT_TA_A, TAKER_TA_A] {
        t.set(created, SYSTEM, 0, vec![]);
    }
    t
}

fn token_account(mint: Address, owner: Address, amount: u64) -> Vec<u8> {
    let mut data = vec![0; TokenAccount::LEN];
    TokenAccount {
        mint,
        owner,
        amount,
        state: AccountState::Initialized,
        close_authority: COption::None,
        ..TokenAccount::default()
    }
    .pack_into_slice(&mut data);
    data
}

fn make(deposit: u64, receive: u64) -> Instruction {
    let accounts = (
        MAKER,
        escrow_of(&MAKER),
        MINT_A,
        MINT_B,
        MAKER_TA_A,
        MAKER_TA_B,
        VAULT_TA_A,
        Rent::ID,
        Token::ID,
        SYSTEM,
    );
    escrow::make::instruction(accounts, Offer { deposit, receive })
}

fn take() -> Instruction {
    let accounts = (
        TAKER,
        escrow_of(&MAKER),
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

fn refund() -> Instruction {
    let accounts =
        (MAKER, escrow_of(&MAKER), MINT_A, MAKER_TA_A, VAULT_TA_A, Rent::ID, Token::ID, SYSTEM);
    escrow::refund::instruction(accounts, ())
}

/// `ix` calling Token-2022 in place of Token.
fn on_token_2022(mut ix: Instruction) -> Instruction {
    for meta in ix.accounts.iter_mut().filter(|meta| meta.pubkey == Token::ID) {
        meta.pubkey = Token2022::ID;
    }
    ix
}

fn amount(t: &Test, account: &Address) -> u64 {
    TokenAccount::unpack(t.data(account)).expect("a token account").amount
}

#[test]
fn a_taker_pays_the_maker_and_receives_the_deposit() {
    let mut t = prepare(Test::new(ID, "escrow"), Token::ID);

    t.run(make(400, 300)).succeeds();
    assert_eq!(t.read::<Escrow>(&escrow_of(&MAKER)).receive, 300);
    assert_eq!(amount(&t, &VAULT_TA_A), 400);

    t.run(take()).succeeds();
    assert_eq!(amount(&t, &TAKER_TA_A), 400);
    assert_eq!(amount(&t, &MAKER_TA_B), 300);
    assert_eq!(amount(&t, &TAKER_TA_B), 700);
    assert_eq!(t.lamports(&escrow_of(&MAKER)), 0);
    assert_eq!(t.lamports(&VAULT_TA_A), 0);
}

#[test]
fn the_maker_can_take_the_deposit_back() {
    let mut t = prepare(Test::new(ID, "escrow"), Token::ID);

    t.run(make(400, 300)).succeeds();
    t.run(refund()).succeeds();

    assert_eq!(amount(&t, &MAKER_TA_A), 1_000);
    assert_eq!(t.lamports(&escrow_of(&MAKER)), 0);
}

#[test]
fn a_token_account_is_not_a_mint_and_a_mint_is_not_a_token_account() {
    let mut t = prepare(Test::new(ID, "escrow"), Token::ID);

    // Each has the other's owner: a mint is too short for a token account,
    // and a token account's length is no mint layout.
    let other_mint = t.account_with(Token::ID, t.rent(Mint::LEN), t.data(&MINT_A).to_vec());
    let other_token_account =
        t.account_with(Token::ID, t.rent(TokenAccount::LEN), t.data(&MAKER_TA_A).to_vec());

    let mut mint_as_token = make(400, 300);
    mint_as_token.accounts[4].pubkey = other_mint;
    t.run(mint_as_token).fails(Error::DataLength);

    let mut token_as_mint = make(400, 300);
    token_as_mint.accounts[2].pubkey = other_token_account;
    // The mint's checks are the Token program's, as it creates the vault:
    // its `InvalidMint`.
    t.run(token_as_mint).fails(ProgramError::Custom(2));
}

#[test]
fn an_existing_token_account_of_another_mint_is_rejected() {
    let mut t = prepare(Test::new(ID, "escrow"), Token::ID);
    // The maker's account for mint B already exists, but holds mint A.
    t.set(MAKER_TA_B, Token::ID, t.rent(TokenAccount::LEN), token_account(MINT_A, MAKER, 0));

    t.run(make(400, 300)).fails(token::Error::Mint);
}

#[test]
fn the_same_escrow_runs_on_token_2022() {
    let mut t = prepare(Test::new(ID, "escrow"), Token2022::ID);

    t.run(on_token_2022(make(400, 300))).succeeds();
    t.run(on_token_2022(take())).succeeds();
    assert_eq!(amount(&t, &TAKER_TA_A), 400);
    assert_eq!(amount(&t, &MAKER_TA_B), 300);
    assert_eq!(t.owner(&TAKER_TA_A), Token2022::ID);

    let mut t = prepare(Test::new(ID, "escrow"), Token2022::ID);
    t.run(on_token_2022(make(400, 300))).succeeds();
    t.run(on_token_2022(refund())).succeeds();
    assert_eq!(amount(&t, &MAKER_TA_A), 1_000);
}

#[test]
fn only_a_token_program_and_its_accounts_are_accepted() {
    let mut t = prepare(Test::new(ID, "escrow"), Token::ID);

    let mut not_a_token_program = make(400, 300);
    not_a_token_program.accounts[8].pubkey = ID;
    t.run(not_a_token_program).fails(ProgramError::IncorrectProgramId);

    let not_a_mint = t.account_with(SYSTEM, t.rent(Mint::LEN), t.data(&MINT_A).to_vec());
    let mut mint_of_another_owner = make(400, 300);
    mint_of_another_owner.accounts[2].pubkey = not_a_mint;
    // Rejected by the Token program, which checks the mint's owner.
    t.run(mint_of_another_owner).fails(ProgramError::IncorrectProgramId);
}

/// `make` leaves the checks of the account it transfers from to the Token
/// program, which runs them: an account that would not have loaded fails
/// the call instead, on either program.
#[test]
fn an_account_the_transfer_vouches_for_is_still_rejected() {
    for program in [Token::ID, Token2022::ID] {
        let mut t = prepare(Test::new(ID, "escrow"), program);
        let held = t.data(&MAKER_TA_A).to_vec();
        let rent = t.rent(TokenAccount::LEN);
        let fakes = [
            ("owned by another program", t.account_with(SYSTEM, rent, held.clone())),
            ("never initialized", t.account_with(program, rent, vec![0; TokenAccount::LEN])),
            ("too short to read", t.account_with(program, rent, held[..100].to_vec())),
        ];
        let before = t.state();
        for (what, fake) in fakes {
            let mut ix = make(400, 300);
            ix.accounts[4].pubkey = fake;
            let ix = if program == Token::ID { ix } else { on_token_2022(ix) };
            assert!(!t.run(ix).is_ok(), "an account {what} was accepted");
            assert_eq!(t.state(), before, "an account {what} changed state");
        }
    }
}

fn scenarios() -> Vec<(&'static str, Vec<Instruction>)> {
    let take_by_the_maker = {
        let mut ix = take();
        ix.accounts[0].pubkey = MAKER;
        ix
    };
    let refund_unsigned = {
        let mut ix = refund();
        ix.accounts[0].is_signer = false;
        ix
    };
    vec![
        ("make", vec![make(400, 300)]),
        ("make then take", vec![make(400, 300), take()]),
        ("make then refund", vec![make(400, 300), refund()]),
        ("take twice", vec![make(400, 300), take(), take()]),
        ("take with nothing made", vec![take()]),
        ("make more than the maker holds", vec![make(2_000, 300)]),
        ("take by an account that holds no mint B", vec![make(400, 300), take_by_the_maker]),
        ("refund without the maker's signature", vec![make(400, 300), refund_unsigned]),
    ]
}

#[test]
fn every_scenario_matches_master() {
    for (name, instructions) in scenarios() {
        let mut ours = prepare(Test::new(ID, "escrow"), Token::ID);
        let mut theirs = prepare(Test::rival(ID, "quasar_escrow"), Token::ID);
        for instruction in instructions {
            let (a, b) = (ours.run(instruction.clone()).is_ok(), theirs.run(instruction).is_ok());
            assert_eq!(a, b, "{name}: we succeeded: {a}, Quasar master succeeded: {b}");
        }
        assert_eq!(ours.state(), theirs.state(), "{name}: account state differs");
    }
}

#[test]
fn the_idl_matches_the_committed_one() {
    let idl = escrow::idl().to_json();
    quasar_test::snapshot(concat!(env!("CARGO_MANIFEST_DIR"), "/idl.json"), &idl);
}

/// Leaving a check to the call that makes it changes nothing a transaction
/// can observe: the build that makes every check itself succeeds on the same
/// instructions and leaves the same state, on both Token programs, for the
/// scenarios and for accounts only the left-out checks would reject.
#[test]
fn eliding_checks_changes_no_outcome() {
    for program in [Token::ID, Token2022::ID] {
        let on = |ix| if program == Token::ID { ix } else { on_token_2022(ix) };
        for (name, instructions) in scenarios() {
            let mut elided = prepare(Test::new(ID, "escrow"), program);
            let mut every = prepare(Test::debug(ID, "escrow"), program);
            for ix in instructions {
                let (a, b) = (elided.run(on(ix.clone())).is_ok(), every.run(on(ix)).is_ok());
                assert_eq!(a, b, "{name}: eliding succeeded: {a}, every check: {b}");
            }
            assert_eq!(elided.state(), every.state(), "{name}: account state differs");
        }
        // Each account a call vouches for, replaced by one that is not what
        // its field declares.
        for (ix, slots) in [(make(400, 300), vec![4]), (take(), vec![6, 8]), (refund(), vec![4])] {
            for slot in slots {
                let mut elided = prepare(Test::new(ID, "escrow"), program);
                let mut every = prepare(Test::debug(ID, "escrow"), program);
                for t in [&mut elided, &mut every] {
                    t.run(on(make(400, 300))).succeeds();
                }
                let rent = elided.rent(TokenAccount::LEN);
                let held = elided.data(&ix.accounts[slot].pubkey).to_vec();
                for (owner, data) in [
                    (SYSTEM, held.clone()),
                    (program, vec![0; TokenAccount::LEN]),
                    (program, held[..100].to_vec()),
                    (program, elided.data(&MINT_A).to_vec()),
                ] {
                    let fake = Address::new_unique();
                    let mut ix = ix.clone();
                    ix.accounts[slot].pubkey = fake;
                    for t in [&mut elided, &mut every] {
                        t.set(fake, owner, rent, data.clone());
                    }
                    let (a, b) = (elided.run(on(ix.clone())).is_ok(), every.run(on(ix)).is_ok());
                    assert_eq!((a, b), (false, false), "slot {slot}: a fake was accepted");
                }
                assert_eq!(elided.state(), every.state(), "slot {slot}: account state differs");
            }
        }
    }
}

/// An escrow that is not at its seeds' address is rejected, even when it
/// holds a valid `Escrow`, signs the transaction, and owns the vault: the
/// program checks the seeds itself, and does not leave them to the runtime's
/// check of the call the escrow signs, which an account that signed the
/// transaction passes without them.
#[test]
fn an_escrow_that_signs_the_transaction_is_still_checked_against_its_seeds() {
    for build in [Test::new as fn(Address, &str) -> Test, Test::debug] {
        let mut t = prepare(build(ID, "escrow"), Token::ID);
        t.run(make(400, 300)).succeeds();
        // A copy of the escrow at an address with a key, and a vault it owns.
        let forged = Address::new_unique();
        let (owner, lamports, data) = (
            t.owner(&escrow_of(&MAKER)),
            t.lamports(&escrow_of(&MAKER)),
            t.data(&escrow_of(&MAKER)).to_vec(),
        );
        t.set(forged, owner, lamports, data);
        let vault = t.account_with(
            Token::ID,
            t.rent(TokenAccount::LEN),
            token_account(MINT_A, forged, 400),
        );

        let before = t.state();
        for signs in [true, false] {
            let mut ix = take();
            ix.accounts[1].pubkey = forged;
            ix.accounts[1].is_signer = signs;
            ix.accounts[8].pubkey = vault;
            // Passed a second time, signing: the runtime gives every entry of
            // an account the privileges of any.
            ix.accounts.push(quasar_lang::client::AccountMeta::new_readonly(forged, true));
            assert!(!t.run(ix).is_ok(), "an escrow not at its seeds' address was accepted");
            assert_eq!(t.state(), before);
        }
        let mut signing = take();
        signing.accounts[1].pubkey = forged;
        signing.accounts[1].is_signer = true;
        signing.accounts[8].pubkey = vault;
        t.run(signing).fails(ProgramError::InvalidSeeds);
    }
}

/// The checks each instruction leaves to a call it always makes, committed:
/// a change to what is elided shows up in review.
#[test]
fn the_elisions_match_the_committed_ones() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/elisions.txt");
    quasar_test::snapshot(path, &escrow::elisions());
}
