//! The [`Outcome`] of executing a transaction.
//!
//! `Outcome` is a newtype over [`parallax_svm::Outcome`]. Reporting accessors
//! (`logs`, `account`, `events`, ...) are reached through [`Deref`]; the
//! chainable assertions are re-declared so a chain stays in quasar-test's
//! `Outcome`, which is what keeps the strict, quasar-lang-typed [`Self::has_state`]
//! reachable after any other assertion.

use {
    crate::{ProgramError, Pubkey},
    quasar_lang::{
        __zeropod::{ZcElem, ZcValidate},
        traits::{Discriminator, Owner},
    },
    std::ops::Deref,
};

/// The structured result of executing one transaction.
#[must_use = "assert the outcome with succeeds, fails, or fails_with"]
pub struct Outcome(parallax_svm::Outcome);

impl Outcome {
    pub(crate) fn new(inner: parallax_svm::Outcome) -> Self {
        Self(inner)
    }

    /// Assert success and keep the outcome available for chained assertions.
    pub fn succeeds(&self) -> &Self {
        self.0.succeeds();
        self
    }

    /// Assert a typed custom program error.
    pub fn fails_with<E>(&self, expected: E) -> &Self
    where
        E: Into<u32>,
    {
        self.0.fails_with(expected);
        self
    }

    /// Assert a runtime or non-custom program error.
    pub fn fails(&self, expected: ProgramError) -> &Self {
        self.0.fails(expected);
        self
    }

    /// Assert an inclusive compute-unit ceiling.
    pub fn cu_at_most(&self, limit: u64) -> &Self {
        self.0.cu_at_most(limit);
        self
    }

    /// Assert a resulting lamport balance.
    pub fn has_lamports(&self, address: Pubkey, expected: u64) -> &Self {
        self.0.has_lamports(address, expected);
        self
    }

    /// Assert a resulting Token or Token-2022 account balance.
    pub fn has_tokens(&self, address: Pubkey, expected: u64) -> &Self {
        self.0.has_tokens(address, expected);
        self
    }

    /// Assert a resulting Token or Token-2022 mint supply.
    pub fn has_supply(&self, address: Pubkey, expected: u64) -> &Self {
        self.0.has_supply(address, expected);
        self
    }

    /// Assert a resulting account is owned by `program`.
    pub fn owned_by(&self, address: Pubkey, program: Pubkey) -> &Self {
        self.0.owned_by(address, program);
        self
    }

    /// Assert Solana's closed-account state. A runtime may remove the account
    /// entirely or retain its empty system-owned representation.
    pub fn is_closed(&self, address: Pubkey) -> &Self {
        self.0.is_closed(address);
        self
    }

    /// Assert typed post-state at `address`, passing the decoded data to
    /// `check` for user assertions.
    ///
    /// The resulting account is read through `T`'s on-chain wrapper with the
    /// same ownership, discriminator, length, and zero-copy validation as
    /// [`Test::read`](crate::Test::read). Panics with the address and the
    /// specific failure when the account is absent or malformed. Chainable, so
    /// several accounts can be asserted in one expression.
    pub fn has_state<T>(&self, address: Pubkey, check: impl FnOnce(&T::Target)) -> &Self
    where
        T: Discriminator + Owner + Deref,
        T::Target: ZcElem + ZcValidate + Copy,
    {
        let name = core::any::type_name::<T>();
        let account = self.0.account(address).unwrap_or_else(|| {
            panic!("has_state {name}: outcome does not contain account {address}")
        });
        let state = crate::world::validate_typed::<T>("has_state", account);
        check(&state);
        self
    }
}

impl Deref for Outcome {
    type Target = parallax_svm::Outcome;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
