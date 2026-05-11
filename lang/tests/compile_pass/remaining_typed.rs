#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

fn parse_signers(accounts: RemainingAccounts<'_>) -> Result<Remaining<Signer, 4>, ProgramError> {
    accounts.parse()
}

fn main() {
    fn _assert_remaining<T, const N: usize>() {
        let _ = core::mem::size_of::<Remaining<T, N>>();
    }

    _assert_remaining::<Signer, 4>();
    _assert_remaining::<UncheckedAccount, 16>();

    let _ = parse_signers;
}
