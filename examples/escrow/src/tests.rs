extern crate std;
use {
    crate::{
        cpi::*,
        state::{Escrow, EscrowData},
    },
    quasar_test::{prelude::*, DEFAULT_WALLET_LAMPORTS},
};

// Deterministic addresses avoid Pubkey::new_unique(), whose global counter
// produces different values depending on test binary layout / discovery order.
const MAKER: Pubkey = Pubkey::new_from_array([1; 32]);
const TAKER: Pubkey = Pubkey::new_from_array([2; 32]);
const MINT_A: Pubkey = Pubkey::new_from_array([3; 32]);
const MINT_B: Pubkey = Pubkey::new_from_array([4; 32]);
const MAKER_TA_A: Pubkey = Pubkey::new_from_array([5; 32]);
const MAKER_TA_B: Pubkey = Pubkey::new_from_array([6; 32]);
const VAULT_TA_A: Pubkey = Pubkey::new_from_array([7; 32]);
const TAKER_TA_A: Pubkey = Pubkey::new_from_array([8; 32]);
const TAKER_TA_B: Pubkey = Pubkey::new_from_array([9; 32]);
const WRONG_OWNER: Pubkey = Pubkey::new_from_array([10; 32]);
const MAX_ELF_BYTES: usize = 44_320;
const MAX_MAKE_CU: u64 = 21_035;
const MAX_TAKE_CU: u64 = 29_256;
const MAX_REFUND_CU: u64 = 16_942;

fn assert_cu(instruction: &str, consumed: u64, maximum: u64) {
    assert!(
        consumed <= maximum,
        "{instruction} consumed {consumed} CU; budget is {maximum}"
    );
}

#[test]
fn elf_size_stays_within_budget() {
    let bytes = std::fs::read("../../target/deploy/quasar_escrow.so").unwrap();
    assert!(
        bytes.len() <= MAX_ELF_BYTES,
        "escrow ELF grew to {} bytes; budget is {MAX_ELF_BYTES}",
        bytes.len()
    );
}

/// Register the maker and both mints.
fn base_world(q: &mut QuasarTest) {
    q.fund(MAKER, DEFAULT_WALLET_LAMPORTS);
    q.add_mint_at(MINT_A, MAKER, 1_000_000_000, 9);
    q.add_mint_at(MINT_B, MAKER, 1_000_000_000, 9);
}

/// Register a live escrow holding 1337 vault tokens, as `make` leaves it.
fn live_escrow(q: &mut QuasarTest) -> Pubkey {
    let (escrow, bump) = q.derive_pda_with_bump(Escrow::seeds(&MAKER));
    q.write(
        escrow,
        EscrowData {
            maker: MAKER,
            mint_a: MINT_A,
            mint_b: MINT_B,
            maker_ta_b: MAKER_TA_B,
            receive: 1337.into(),
            bump,
        },
    );
    q.add_token_account_at(VAULT_TA_A, escrow, MINT_A, 1337);
    escrow
}

#[quasar_test]
fn test_make_cu(q: &mut QuasarTest) {
    base_world(q);
    q.add_token_account_at(MAKER_TA_A, MAKER, MINT_A, 1_000_000);
    let (escrow, bump) = q.derive_pda_with_bump(Escrow::seeds(&MAKER));

    let result = q.send(MakeInstruction {
        maker: MAKER,
        mint_a: MINT_A,
        mint_b: MINT_B,
        maker_ta_a: MAKER_TA_A,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
        deposit: 1337,
        receive: 1337,
    });
    result.succeeds();

    let state = q.read::<Escrow>(escrow);
    assert_eq!(state.maker, MAKER);
    assert_eq!(state.receive, 1337);
    assert_eq!(state.bump, bump);

    assert_cu("make", result.compute_units_consumed, MAX_MAKE_CU);
}

#[quasar_test]
fn test_take_cu(q: &mut QuasarTest) {
    base_world(q);
    q.fund(TAKER, DEFAULT_WALLET_LAMPORTS);
    live_escrow(q);
    q.add_token_account_at(TAKER_TA_B, TAKER, MINT_B, 10_000);

    let result = q.send(TakeInstruction {
        taker: TAKER,
        maker: MAKER,
        mint_a: MINT_A,
        mint_b: MINT_B,
        taker_ta_a: TAKER_TA_A,
        taker_ta_b: TAKER_TA_B,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
    });
    result.succeeds();

    assert_cu("take", result.compute_units_consumed, MAX_TAKE_CU);
}

#[quasar_test]
fn test_refund_cu(q: &mut QuasarTest) {
    base_world(q);
    live_escrow(q);

    let result = q.send(RefundInstruction {
        maker: MAKER,
        mint_a: MINT_A,
        maker_ta_a: MAKER_TA_A,
        vault_ta_a: VAULT_TA_A,
    });
    result.succeeds();

    assert_cu("refund", result.compute_units_consumed, MAX_REFUND_CU);
}

#[quasar_test]
fn test_make_existing_token_accounts(q: &mut QuasarTest) {
    base_world(q);
    q.add_token_account_at(MAKER_TA_A, MAKER, MINT_A, 1_000_000);
    let escrow = q.derive_pda(Escrow::seeds(&MAKER));
    q.add_token_account_at(MAKER_TA_B, MAKER, MINT_B, 0);
    q.add_token_account_at(VAULT_TA_A, escrow, MINT_A, 0);

    q.send(MakeInstruction {
        maker: MAKER,
        mint_a: MINT_A,
        mint_b: MINT_B,
        maker_ta_a: MAKER_TA_A,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
        deposit: 1337,
        receive: 1337,
    })
    .succeeds();
}

#[quasar_test]
fn test_make_existing_maker_ta_b_wrong_mint(q: &mut QuasarTest) {
    base_world(q);
    q.add_token_account_at(MAKER_TA_A, MAKER, MINT_A, 1_000_000);
    let escrow = q.derive_pda(Escrow::seeds(&MAKER));
    q.add_token_account_at(MAKER_TA_B, MAKER, MINT_A, 0); // wrong mint
    q.add_token_account_at(VAULT_TA_A, escrow, MINT_A, 0);

    let result = q.send(MakeInstruction {
        maker: MAKER,
        mint_a: MINT_A,
        mint_b: MINT_B,
        maker_ta_a: MAKER_TA_A,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
        deposit: 1337,
        receive: 1337,
    });
    assert!(
        result.is_err(),
        "make should fail with wrong mint on maker_ta_b"
    );
}

#[quasar_test]
fn test_make_existing_maker_ta_b_wrong_owner(q: &mut QuasarTest) {
    base_world(q);
    q.add_token_account_at(MAKER_TA_A, MAKER, MINT_A, 1_000_000);
    let escrow = q.derive_pda(Escrow::seeds(&MAKER));
    q.add_token_account_at(MAKER_TA_B, WRONG_OWNER, MINT_B, 0); // wrong owner
    q.add_token_account_at(VAULT_TA_A, escrow, MINT_A, 0);

    let result = q.send(MakeInstruction {
        maker: MAKER,
        mint_a: MINT_A,
        mint_b: MINT_B,
        maker_ta_a: MAKER_TA_A,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
        deposit: 1337,
        receive: 1337,
    });
    assert!(
        result.is_err(),
        "make should fail with wrong owner on maker_ta_b"
    );
}

#[quasar_test]
fn test_take_existing_token_accounts(q: &mut QuasarTest) {
    base_world(q);
    q.fund(TAKER, DEFAULT_WALLET_LAMPORTS);
    live_escrow(q);
    q.add_token_account_at(TAKER_TA_A, TAKER, MINT_A, 0);
    q.add_token_account_at(TAKER_TA_B, TAKER, MINT_B, 10_000);
    q.add_token_account_at(MAKER_TA_B, MAKER, MINT_B, 500);

    q.send(TakeInstruction {
        taker: TAKER,
        maker: MAKER,
        mint_a: MINT_A,
        mint_b: MINT_B,
        taker_ta_a: TAKER_TA_A,
        taker_ta_b: TAKER_TA_B,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
    })
    .succeeds();
}

#[quasar_test]
fn test_refund_existing_maker_ta_a(q: &mut QuasarTest) {
    base_world(q);
    q.add_token_account_at(MAKER_TA_A, MAKER, MINT_A, 5_000);
    live_escrow(q);

    q.send(RefundInstruction {
        maker: MAKER,
        mint_a: MINT_A,
        maker_ta_a: MAKER_TA_A,
        vault_ta_a: VAULT_TA_A,
    })
    .succeeds();
}
