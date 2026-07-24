//! The generated client instruction struct exposes `{field}_address(&self)`
//! for every derived (PDA/ATA) account, and names stored-data seed inputs by
//! the shared collision rule (bare field name when unambiguous).
#![allow(dead_code, unexpected_cfgs)]

extern crate alloc;

use {
    quasar_derive::{Accounts, Seeds},
    quasar_lang::prelude::*,
};

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 3)]
#[seeds(b"escrow", authority: Address)]
pub struct Escrow {
    pub authority: Address,
    pub amount: u64,
    pub bump: u8,
}

#[derive(Seeds)]
#[seeds(b"vault", authority: Address)]
pub struct Vault;

#[derive(Seeds)]
#[seeds(b"receipt", amount: u64)]
pub struct Receipt;

#[derive(Accounts)]
pub struct Claim {
    pub authority: Signer,
    pub escrow: Account<Escrow>,
    /// PDA derived from a caller account address.
    #[account(address = Vault::seeds(authority.address()))]
    pub vault: UncheckedAccount,
    /// PDA derived from `escrow`'s stored `amount` field: the client takes the
    /// bare `amount` input, not `escrow_amount_seed`.
    #[account(address = Receipt::seeds(escrow.amount.into()))]
    pub receipt: UncheckedAccount,
}

// The client macro expands inside the generated `cpi` module (a child of the
// `#[program]` module); this mirror gives its `super::` paths the same shape.
mod cpi {
    use super::*;

    __claim_instruction!(ClaimInstruction, ClaimInstructionRaw, [0], {});
}
use cpi::*;

fn address(byte: u8) -> Address {
    Address::from([byte; 32])
}

#[test]
fn builder_exposes_derived_addresses_without_rederiving() {
    // The stored-data seed input is the bare `amount` (collision rule).
    let ix = ClaimInstruction {
        authority: address(1),
        escrow: address(2),
        amount: 42,
    };

    let vault = ix.vault_address();
    let receipt = ix.receipt_address();

    // Same recipe the builder uses internally.
    assert_eq!(vault, Vault::find_address(address(1), &ID));
    assert_eq!(receipt, Receipt::find_address(42u64, &ID));

    // ...and the accessors agree with the built instruction's account metas.
    let instruction: quasar_lang::client::Instruction = ix.into();
    assert_eq!(instruction.accounts[2].pubkey, vault);
    assert_eq!(instruction.accounts[3].pubkey, receipt);
}
