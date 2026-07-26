//! The [`Outcome`] of executing a transaction.
//!
//! `Outcome` is a newtype over [`parallax_svm::Outcome`]. Reporting accessors
//! (`logs`, `account`, `events`, ...) are reached through [`Deref`]; the
//! verdict and `check` are re-declared so a chain stays in quasar-test's
//! `Outcome`. The [`State`] facts here validate the quasar way — ownership,
//! discriminator, length, and zero-copy validation — replacing Parallax's
//! schema-only `State` in this crate's prelude.

use {
    crate::{ProgramError, Pubkey},
    parallax_svm::Assert,
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

    /// Run check values against this outcome — built-in Parallax facts
    /// (`Cu`, `Account::lamports`, `Changes`, ...), quasar-test's strict
    /// [`State`], closures, and arrays or tuples of any of them. Chainable.
    pub fn check(&self, check: impl parallax_svm::Check) -> &Self {
        self.0.check(check);
        self
    }
}

impl Deref for Outcome {
    type Target = parallax_svm::Outcome;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Typed account-state facts, validated the quasar way.
///
/// `State::of(address)` binds the account; `.eq(expected)` and
/// `.with::<T>(closure)` finish the fact. The account is read through `T`'s
/// on-chain wrapper with the same ownership, discriminator, length, and
/// zero-copy validation as [`Test::read`](crate::Test::read) — quasar-test's
/// strict sibling of Parallax's schema-only `Account::state`. Constructors
/// return Parallax's [`Assert`], so these facts group in the same
/// `check([..])` arrays as the built-ins.
pub struct State;

impl State {
    /// Bind the account at `address` for a strict typed-state fact.
    pub fn of(address: Pubkey) -> StateMeasure {
        StateMeasure(address)
    }
}

/// A bound strict typed-state fact awaiting its expected value or closure.
pub struct StateMeasure(Pubkey);

impl StateMeasure {
    /// Assert the account decodes and validates to exactly `expected`.
    pub fn eq<T>(self, expected: T::Target) -> Assert
    where
        T: Discriminator + Owner + Deref + 'static,
        T::Target: ZcElem + ZcValidate + Copy + PartialEq + core::fmt::Debug,
    {
        let address = self.0;
        self.with::<T>(move |state| assert_eq!(*state, expected, "unexpected state for {address}"))
    }

    /// Assert on the decoded, validated state with a closure, for partial or
    /// computed facts.
    pub fn with<T>(self, check: impl Fn(&T::Target) + 'static) -> Assert
    where
        T: Discriminator + Owner + Deref + 'static,
        T::Target: ZcElem + ZcValidate + Copy,
    {
        let address = self.0;
        Assert::from_fn(move |outcome| {
            let name = core::any::type_name::<T>();
            let account = outcome.account(address).unwrap_or_else(|| {
                panic!("State {name}: outcome does not contain account {address}")
            });
            check(&crate::world::validate_typed::<T>("State", account));
        })
    }
}
