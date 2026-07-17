#![allow(dead_code, unexpected_cfgs)]

extern crate alloc;

use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{
        accounts::{associated_token, token},
        TokenProgram, *,
    },
};

solana_address::declare_id!("11111111111111111111111111111112");

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

__init_associated_token_instruction!(InitAssociatedTokenInstruction, [0], {});

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

__init_token_account_instruction!(InitTokenAccountInstruction, [1], {});

fn address(byte: u8) -> Address {
    Address::from([byte; 32])
}

#[test]
fn associated_token_init_does_not_require_the_derived_account_to_sign() {
    let instruction: quasar_lang::client::Instruction = InitAssociatedTokenInstruction {
        payer: address(1),
        mint: address(2),
        ata: address(3),
        token_program: address(4),
        system_program: address(5),
        ata_program: address(6),
    }
    .into();

    assert!(instruction.accounts[0].is_signer);
    assert!(!instruction.accounts[2].is_signer);
}

#[test]
fn direct_token_init_still_requires_the_new_account_to_sign() {
    let instruction: quasar_lang::client::Instruction = InitTokenAccountInstruction {
        payer: address(1),
        mint: address(2),
        authority: address(3),
        token: address(4),
        token_program: address(5),
        system_program: address(6),
    }
    .into();

    assert!(instruction.accounts[0].is_signer);
    assert!(instruction.accounts[2].is_signer);
    assert!(instruction.accounts[3].is_signer);
}
