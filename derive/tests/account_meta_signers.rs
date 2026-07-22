#![allow(dead_code, unexpected_cfgs)]

extern crate alloc;

use {
    quasar_derive::{Accounts, Seeds},
    quasar_lang::prelude::*,
    quasar_spl::{
        accounts::{associated_token, token},
        TokenProgram, *,
    },
};

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Seeds)]
#[seeds(b"vault", authority: Address)]
struct Vault;

#[derive(Accounts)]
struct Withdraw {
    authority: Signer,
    /// CHECK: the typed-seed address constraint is the validation under test.
    #[account(mut, address = Vault::seeds(authority.address()))]
    vault: UncheckedAccount,
}

#[derive(Accounts)]
struct InitAssociatedToken {
    #[account(mut)]
    payer: Signer,
    mint: Account<Mint>,
    #[account(
        mut,
        init(idempotent),
        associated_token(
            authority = payer,
            mint = mint,
            token_program = token_program,
            system_program = system_program,
            ata_program = ata_program,
        ),
    )]
    ata: Account<Token>,
    token_program: Program<TokenProgram>,
    system_program: Program<SystemProgram>,
    ata_program: Program<AssociatedTokenProgram>,
}

#[derive(Accounts)]
struct InitTokenAccount {
    #[account(mut)]
    payer: Signer,
    mint: Account<Mint>,
    authority: Signer,
    #[account(
        mut,
        init,
        token(mint = mint, authority = authority, token_program = token_program),
    )]
    token: Account<Token>,
    token_program: Program<TokenProgram>,
    system_program: Program<SystemProgram>,
}

// The client macro expands inside the generated `cpi` module (a child of
// the `#[program]` module); this mirror gives its `super::` paths the same
// shape.
mod cpi {
    use super::*;

    __init_associated_token_instruction!(
        InitAssociatedTokenInstruction,
        InitAssociatedTokenInstructionRaw,
        [0],
        {}
    );
    __init_token_account_instruction!(
        InitTokenAccountInstruction,
        InitTokenAccountInstructionRaw,
        [1],
        {}
    );
    __withdraw_instruction!(WithdrawInstruction, WithdrawInstructionRaw, [2], {});
}
use cpi::*;

fn address(byte: u8) -> Address {
    Address::from([byte; 32])
}

#[test]
fn associated_token_init_does_not_require_the_derived_account_to_sign() {
    let instruction: quasar_lang::client::Instruction = InitAssociatedTokenInstruction {
        payer: address(1),
        mint: address(2),
    }
    .into();

    assert!(instruction.accounts[0].is_signer);
    assert!(!instruction.accounts[2].is_signer);
}

#[test]
fn raw_associated_token_instruction_can_override_the_derived_address() {
    let explicit_ata = address(9);
    let instruction: quasar_lang::client::Instruction = InitAssociatedTokenInstructionRaw {
        payer: address(1),
        mint: address(2),
        ata: explicit_ata,
    }
    .into();

    assert_eq!(instruction.accounts[2].pubkey, explicit_ata);
    assert!(!instruction.accounts[2].is_signer);
}

#[test]
fn canonical_pda_is_inferred_and_raw_pda_is_explicit() {
    let authority = address(3);
    let expected = Vault::find_address(authority, &ID);
    let canonical: quasar_lang::client::Instruction = WithdrawInstruction { authority }.into();
    assert_eq!(canonical.accounts[1].pubkey, expected);

    let explicit = address(8);
    let raw: quasar_lang::client::Instruction = WithdrawInstructionRaw {
        authority,
        vault: explicit,
    }
    .into();
    assert_eq!(raw.accounts[1].pubkey, explicit);
}

#[test]
fn direct_token_init_still_requires_the_new_account_to_sign() {
    let instruction: quasar_lang::client::Instruction = InitTokenAccountInstruction {
        payer: address(1),
        mint: address(2),
        authority: address(3),
        token: address(4),
    }
    .into();

    assert!(instruction.accounts[0].is_signer);
    assert!(instruction.accounts[2].is_signer);
    assert!(instruction.accounts[3].is_signer);
}
