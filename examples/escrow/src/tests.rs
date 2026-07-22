extern crate std;
use {
    crate::{
        cpi::*,
        state::{Escrow, EscrowData},
    },
    quasar_test::prelude::*,
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
fn base_world(test: &mut Test) {
    test.add(Wallet::new().at(MAKER));
    test.add(
        Mint::new(MAKER)
            .at(MINT_A)
            .supply(1_000_000_000)
            .decimals(9),
    );
    test.add(
        Mint::new(MAKER)
            .at(MINT_B)
            .supply(1_000_000_000)
            .decimals(9),
    );
}

/// Register a live escrow holding 1337 vault tokens, as `make` leaves it.
fn live_escrow(test: &mut Test) -> Pubkey {
    let (escrow, bump) = test.derive_pda_with_bump(Escrow::seeds(&MAKER));
    test.write(
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
    test.add(
        TokenAccount::new(MINT_A, escrow)
            .at(VAULT_TA_A)
            .amount(1337),
    );
    escrow
}

#[quasar_test]
fn test_make_cu(test: &mut Test) {
    base_world(test);
    test.add(
        TokenAccount::new(MINT_A, MAKER)
            .at(MAKER_TA_A)
            .amount(1_000_000),
    );
    let (escrow, bump) = test.derive_pda_with_bump(Escrow::seeds(&MAKER));

    let result = test.send(MakeInstruction {
        maker: MAKER,
        mint_a: MINT_A,
        mint_b: MINT_B,
        maker_ta_a: MAKER_TA_A,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
        deposit: 1337,
        receive: 1337,
    });
    result.succeeds().cu_at_most(MAX_MAKE_CU);

    let state = test.read::<Escrow>(escrow);
    assert_eq!(state.maker, MAKER);
    assert_eq!(state.receive, 1337);
    assert_eq!(state.bump, bump);
}

#[quasar_test]
fn test_take_cu(test: &mut Test) {
    base_world(test);
    test.add(Wallet::new().at(TAKER));
    live_escrow(test);
    test.add(
        TokenAccount::new(MINT_B, TAKER)
            .at(TAKER_TA_B)
            .amount(10_000),
    );

    let result = test.send(TakeInstruction {
        taker: TAKER,
        maker: MAKER,
        mint_a: MINT_A,
        mint_b: MINT_B,
        taker_ta_a: TAKER_TA_A,
        taker_ta_b: TAKER_TA_B,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
    });
    result.succeeds().cu_at_most(MAX_TAKE_CU);
}

#[quasar_test]
fn test_refund_cu(test: &mut Test) {
    base_world(test);
    live_escrow(test);

    let result = test.send(RefundInstruction {
        maker: MAKER,
        mint_a: MINT_A,
        maker_ta_a: MAKER_TA_A,
        vault_ta_a: VAULT_TA_A,
    });
    result.succeeds().cu_at_most(MAX_REFUND_CU);
}

#[quasar_test]
fn test_make_existing_token_accounts(test: &mut Test) {
    base_world(test);
    test.add(
        TokenAccount::new(MINT_A, MAKER)
            .at(MAKER_TA_A)
            .amount(1_000_000),
    );
    let escrow = test.derive_pda(Escrow::seeds(&MAKER));
    test.add(TokenAccount::new(MINT_B, MAKER).at(MAKER_TA_B));
    test.add(TokenAccount::new(MINT_A, escrow).at(VAULT_TA_A));

    test.send(MakeInstruction {
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
fn test_make_existing_maker_ta_b_wrong_mint(test: &mut Test) {
    base_world(test);
    test.add(
        TokenAccount::new(MINT_A, MAKER)
            .at(MAKER_TA_A)
            .amount(1_000_000),
    );
    let escrow = test.derive_pda(Escrow::seeds(&MAKER));
    test.add(TokenAccount::new(MINT_A, MAKER).at(MAKER_TA_B)); // wrong mint
    test.add(TokenAccount::new(MINT_A, escrow).at(VAULT_TA_A));

    let result = test.send(MakeInstruction {
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
fn test_make_existing_maker_ta_b_wrong_owner(test: &mut Test) {
    base_world(test);
    test.add(
        TokenAccount::new(MINT_A, MAKER)
            .at(MAKER_TA_A)
            .amount(1_000_000),
    );
    let escrow = test.derive_pda(Escrow::seeds(&MAKER));
    test.add(TokenAccount::new(MINT_B, WRONG_OWNER).at(MAKER_TA_B)); // wrong owner
    test.add(TokenAccount::new(MINT_A, escrow).at(VAULT_TA_A));

    let result = test.send(MakeInstruction {
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
fn test_take_existing_token_accounts(test: &mut Test) {
    base_world(test);
    test.add(Wallet::new().at(TAKER));
    live_escrow(test);
    test.add(TokenAccount::new(MINT_A, TAKER).at(TAKER_TA_A));
    test.add(
        TokenAccount::new(MINT_B, TAKER)
            .at(TAKER_TA_B)
            .amount(10_000),
    );
    test.add(TokenAccount::new(MINT_B, MAKER).at(MAKER_TA_B).amount(500));

    test.send(TakeInstruction {
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
fn test_refund_existing_maker_ta_a(test: &mut Test) {
    base_world(test);
    test.add(
        TokenAccount::new(MINT_A, MAKER)
            .at(MAKER_TA_A)
            .amount(5_000),
    );
    live_escrow(test);

    test.send(RefundInstruction {
        maker: MAKER,
        mint_a: MINT_A,
        maker_ta_a: MAKER_TA_A,
        vault_ta_a: VAULT_TA_A,
    })
    .succeeds();
}
