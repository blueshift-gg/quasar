#![allow(unexpected_cfgs)]
use quasar_derive::Accounts;
use quasar_lang::prelude::*;

#[derive(Accounts)]
pub struct SignerPair {
    pub first: Signer,
    pub second: Signer,
}

fn parse_signers(accounts: RemainingAccounts<'_>) -> Result<Remaining<Signer, 4>, ProgramError> {
    accounts.parse()
}

fn parse_pairs(accounts: RemainingAccounts<'_>) -> Result<Remaining<SignerPair, 4>, ProgramError> {
    accounts.parse()
}

fn main() {
    fn _assert_remaining<T, const N: usize>() {
        let _ = core::mem::size_of::<Remaining<T, N>>();
    }

    _assert_remaining::<Signer, 4>();
    _assert_remaining::<UncheckedAccount, 16>();
    _assert_remaining::<SignerPair, 4>();

    let _ = parse_signers;
    let _ = parse_pairs;
}
