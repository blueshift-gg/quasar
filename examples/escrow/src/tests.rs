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
// Mints are not pinned: their fixtures return the address the world placed.
const MAKER: Pubkey = Pubkey::new_from_array([1; 32]);
const TAKER: Pubkey = Pubkey::new_from_array([2; 32]);
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

/// Register the maker and both mints, returning the mint addresses the fixtures
/// placed.
fn base_world(ctx: &mut Ctx) -> (Pubkey, Pubkey) {
    ctx.add(Wallet::account().at(MAKER));
    let mint_a = ctx.add(Mint::account().with_supply(1_000_000_000).decimals(9));
    let mint_b = ctx.add(Mint::account().with_supply(1_000_000_000).decimals(9));
    (mint_a, mint_b)
}

/// Register a live escrow holding 1337 vault tokens, as `make` leaves it.
fn live_escrow(ctx: &mut Ctx, mint_a: Pubkey, mint_b: Pubkey) -> Pubkey {
    let (escrow, bump) = ctx.derive_pda_with_bump(Escrow::seeds(&MAKER));
    ctx.write(
        escrow,
        EscrowData {
            maker: MAKER,
            mint_a,
            mint_b,
            maker_ta_b: MAKER_TA_B,
            receive: 1337.into(),
            bump,
        },
    );
    ctx.add(
        TokenAccount::account(mint_a, escrow)
            .at(VAULT_TA_A)
            .with_amount(1337),
    );
    escrow
}

#[quasar_test]
fn test_make_cu() {
    let (mint_a, mint_b) = base_world(ctx);
    ctx.add(
        TokenAccount::account(mint_a, MAKER)
            .at(MAKER_TA_A)
            .with_amount(1_000_000),
    );
    let (escrow, bump) = ctx.derive_pda_with_bump(Escrow::seeds(&MAKER));

    ctx.execute(MakeInstruction {
        maker: MAKER,
        mint_a,
        mint_b,
        maker_ta_a: MAKER_TA_A,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
        deposit: 1337,
        receive: 1337,
    })
    .checks([Outcome::success(), Cu::spent(|cu| cu <= MAX_MAKE_CU)]);

    let state = ctx.read::<Escrow>(escrow);
    assert_eq!(state.maker, MAKER);
    assert_eq!(state.receive, 1337);
    assert_eq!(state.bump, bump);
}

#[quasar_test]
fn test_take_cu() {
    let (mint_a, mint_b) = base_world(ctx);
    ctx.add(Wallet::account().at(TAKER));
    live_escrow(ctx, mint_a, mint_b);
    ctx.add(
        TokenAccount::account(mint_b, TAKER)
            .at(TAKER_TA_B)
            .with_amount(10_000),
    );

    ctx.execute(TakeInstruction {
        taker: TAKER,
        maker: MAKER,
        mint_a,
        mint_b,
        taker_ta_a: TAKER_TA_A,
        taker_ta_b: TAKER_TA_B,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
    })
    .checks([Outcome::success(), Cu::spent(|cu| cu <= MAX_TAKE_CU)]);
}

#[quasar_test]
fn test_refund_cu() {
    let (mint_a, mint_b) = base_world(ctx);
    live_escrow(ctx, mint_a, mint_b);

    ctx.execute(RefundInstruction {
        maker: MAKER,
        mint_a,
        maker_ta_a: MAKER_TA_A,
        vault_ta_a: VAULT_TA_A,
    })
    .checks([Outcome::success(), Cu::spent(|cu| cu <= MAX_REFUND_CU)]);
}

#[quasar_test]
fn test_make_existing_token_accounts() {
    let (mint_a, mint_b) = base_world(ctx);
    ctx.add(
        TokenAccount::account(mint_a, MAKER)
            .at(MAKER_TA_A)
            .with_amount(1_000_000),
    );
    let escrow = ctx.derive_pda(Escrow::seeds(&MAKER));
    ctx.add(TokenAccount::account(mint_b, MAKER).at(MAKER_TA_B));
    ctx.add(TokenAccount::account(mint_a, escrow).at(VAULT_TA_A));

    ctx.execute(MakeInstruction {
        maker: MAKER,
        mint_a,
        mint_b,
        maker_ta_a: MAKER_TA_A,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
        deposit: 1337,
        receive: 1337,
    })
    .check(Outcome::success());
}

#[quasar_test]
fn test_make_existing_maker_ta_b_wrong_mint() {
    let (mint_a, mint_b) = base_world(ctx);
    ctx.add(
        TokenAccount::account(mint_a, MAKER)
            .at(MAKER_TA_A)
            .with_amount(1_000_000),
    );
    let escrow = ctx.derive_pda(Escrow::seeds(&MAKER));
    ctx.add(TokenAccount::account(mint_a, MAKER).at(MAKER_TA_B)); // wrong mint
    ctx.add(TokenAccount::account(mint_a, escrow).at(VAULT_TA_A));

    let result = ctx.execute(MakeInstruction {
        maker: MAKER,
        mint_a,
        mint_b,
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
fn test_make_existing_maker_ta_b_wrong_owner() {
    let (mint_a, mint_b) = base_world(ctx);
    ctx.add(
        TokenAccount::account(mint_a, MAKER)
            .at(MAKER_TA_A)
            .with_amount(1_000_000),
    );
    let escrow = ctx.derive_pda(Escrow::seeds(&MAKER));
    ctx.add(TokenAccount::account(mint_b, WRONG_OWNER).at(MAKER_TA_B)); // wrong owner
    ctx.add(TokenAccount::account(mint_a, escrow).at(VAULT_TA_A));

    let result = ctx.execute(MakeInstruction {
        maker: MAKER,
        mint_a,
        mint_b,
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
fn test_take_existing_token_accounts() {
    let (mint_a, mint_b) = base_world(ctx);
    ctx.add(Wallet::account().at(TAKER));
    live_escrow(ctx, mint_a, mint_b);
    ctx.add(TokenAccount::account(mint_a, TAKER).at(TAKER_TA_A));
    ctx.add(
        TokenAccount::account(mint_b, TAKER)
            .at(TAKER_TA_B)
            .with_amount(10_000),
    );
    ctx.add(
        TokenAccount::account(mint_b, MAKER)
            .at(MAKER_TA_B)
            .with_amount(500),
    );

    ctx.execute(TakeInstruction {
        taker: TAKER,
        maker: MAKER,
        mint_a,
        mint_b,
        taker_ta_a: TAKER_TA_A,
        taker_ta_b: TAKER_TA_B,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
    })
    .check(Outcome::success());
}

#[quasar_test]
fn test_refund_existing_maker_ta_a() {
    let (mint_a, mint_b) = base_world(ctx);
    ctx.add(
        TokenAccount::account(mint_a, MAKER)
            .at(MAKER_TA_A)
            .with_amount(5_000),
    );
    live_escrow(ctx, mint_a, mint_b);

    ctx.execute(RefundInstruction {
        maker: MAKER,
        mint_a,
        maker_ta_a: MAKER_TA_A,
        vault_ta_a: VAULT_TA_A,
    })
    .check(Outcome::success());
}
