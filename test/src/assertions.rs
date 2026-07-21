use crate::{Account, ProgramError, Pubkey};

/// Test-side adjustments to a built [`quasar_svm::Instruction`].
///
/// Generated client instructions encode the canonical call. These helpers
/// express a test's deliberate deviations from it by address, so the
/// deviation is visible where the test constructs the instruction.
pub trait InstructionExt: Sized {
    /// Mark the given accounts as signers.
    fn signed_by(self, signers: &[Pubkey]) -> quasar_svm::Instruction;

    /// Replace every meta for `from` with `to`, e.g. to hand an instruction
    /// the wrong program or a foreign account on purpose.
    fn swap_account(self, from: Pubkey, to: Pubkey) -> quasar_svm::Instruction;
}

impl<T: Into<quasar_svm::Instruction>> InstructionExt for T {
    fn signed_by(self, signers: &[Pubkey]) -> quasar_svm::Instruction {
        let mut instruction = self.into();
        for meta in &mut instruction.accounts {
            if signers.contains(&meta.pubkey) {
                meta.is_signer = true;
            }
        }
        instruction
    }

    fn swap_account(self, from: Pubkey, to: Pubkey) -> quasar_svm::Instruction {
        let mut instruction = self.into();
        for meta in &mut instruction.accounts {
            if meta.pubkey == from {
                meta.pubkey = to;
            }
        }
        instruction
    }
}

/// Assertions layered on [`quasar_svm::ExecutionResult`].
///
/// Asserting methods chain (`.succeeds().has_tokens(vault, 10)`); the noun
/// forms (`lamports`, `tokens`, `supply`) return the resulting values.
pub trait ExecutionResultExt {
    /// Assert success and keep the result available for chained expectations.
    fn succeeds(&self) -> &Self;

    /// Assert a typed custom error and keep the result available for chaining.
    fn fails_with<E>(&self, expected: E) -> &Self
    where
        E: Into<u32>;

    /// Assert a non-custom [`ProgramError`] and keep the result available for
    /// chaining.
    fn fails(&self, expected: ProgramError) -> &Self;

    /// Assert a compute-unit ceiling and keep the result available for
    /// chaining.
    fn cu_below(&self, limit: u64) -> &Self;

    /// Assert a lamport balance and keep the result available for chaining.
    fn has_lamports(&self, address: Pubkey, expected: u64) -> &Self;

    /// Assert a token balance and keep the result available for chaining.
    fn has_tokens(&self, address: Pubkey, expected: u64) -> &Self;

    /// Assert a mint supply and keep the result available for chaining.
    fn has_supply(&self, address: Pubkey, expected: u64) -> &Self;

    /// Assert that an account has been fully closed: zero lamports, no data,
    /// system-owned.
    fn is_closed(&self, address: Pubkey) -> &Self;

    /// An account's resulting lamport balance.
    fn lamports(&self, address: &Pubkey) -> u64;

    /// A token account's resulting balance.
    fn tokens(&self, address: &Pubkey) -> u64;

    /// A mint's resulting supply.
    fn supply(&self, address: &Pubkey) -> u64;
}

impl ExecutionResultExt for quasar_svm::ExecutionResult {
    fn succeeds(&self) -> &Self {
        self.assert_success();
        self
    }

    fn fails_with<E>(&self, expected: E) -> &Self
    where
        E: Into<u32>,
    {
        self.assert_error(quasar_svm::ProgramError::Custom(expected.into()));
        self
    }

    fn fails(&self, expected: ProgramError) -> &Self {
        self.assert_error(expected);
        self
    }

    fn cu_below(&self, limit: u64) -> &Self {
        assert!(
            self.compute_units_consumed < limit,
            "expected fewer than {limit} compute units, consumed {}",
            self.compute_units_consumed
        );
        self
    }

    fn has_lamports(&self, address: Pubkey, expected: u64) -> &Self {
        assert_eq!(
            self.lamports(&address),
            expected,
            "unexpected lamport balance for {address}"
        );
        self
    }

    fn has_tokens(&self, address: Pubkey, expected: u64) -> &Self {
        assert_eq!(
            self.tokens(&address),
            expected,
            "unexpected token balance for {address}"
        );
        self
    }

    fn has_supply(&self, address: Pubkey, expected: u64) -> &Self {
        assert_eq!(
            self.supply(&address),
            expected,
            "unexpected mint supply for {address}"
        );
        self
    }

    fn is_closed(&self, address: Pubkey) -> &Self {
        let account = result_account(self, &address);
        assert_eq!(
            account.lamports, 0,
            "expected {address} to be closed, but it still holds lamports"
        );
        assert!(
            account.data.is_empty(),
            "expected {address} to be closed, but it still has account data"
        );
        assert_eq!(
            account.owner,
            quasar_svm::system_program::ID,
            "expected {address} to be closed, but it is not system-owned"
        );
        self
    }

    fn lamports(&self, address: &Pubkey) -> u64 {
        result_account(self, address).lamports
    }

    fn tokens(&self, address: &Pubkey) -> u64 {
        use spl_token::{solana_program::program_pack::Pack, state::Account as TokenAccount};

        let account = result_account(self, address);
        let token_account = TokenAccount::unpack(&account.data).unwrap_or_else(|error| {
            panic!("could not decode {address} as an SPL Token account: {error}")
        });
        token_account.amount
    }

    fn supply(&self, address: &Pubkey) -> u64 {
        use spl_token::{solana_program::program_pack::Pack, state::Mint};

        let account = result_account(self, address);
        let mint = Mint::unpack(&account.data).unwrap_or_else(|error| {
            panic!("could not decode {address} as an SPL Token mint: {error}")
        });
        mint.supply
    }
}

fn result_account<'a>(result: &'a quasar_svm::ExecutionResult, address: &Pubkey) -> &'a Account {
    result
        .account(address)
        .unwrap_or_else(|| panic!("execution result does not contain account {address}"))
}
