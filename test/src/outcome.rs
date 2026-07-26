//! The [`Outcome`] of executing a transaction.
//!
//! `Outcome` is a newtype over [`parallax_svm::Outcome`]. Reporting accessors
//! (`logs`, `account`, `events`, ...) are reached through [`Deref`]; the
//! verdicts are re-declared so a chain stays in quasar-test's types. Success
//! yields a quasar-test [`SucceededTransaction`] — a newtype over Parallax's
//! witness that [`Deref`]s to it and delegates `check`/`checks` — so the strict
//! [`State`] facts below group in the same `checks([..])` arrays as the
//! built-ins. The failure verdicts hand back Parallax's [`FailedTransaction`]
//! directly, since a failed transaction commits nothing to check.
//!
//! The [`State`] facts here validate the quasar way — ownership, discriminator,
//! length, and zero-copy validation — replacing Parallax's schema-only `data`
//! predicate in this crate's prelude.

use {
    crate::{ProgramError, Pubkey},
    parallax_svm::{CheckFn, FailedTransaction},
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

    /// Assert success, yielding the [`SucceededTransaction`] witness that
    /// checks run against.
    pub fn succeeds(self) -> SucceededTransaction {
        SucceededTransaction(self.0.succeeds())
    }

    /// Assert a typed custom program error, yielding the failed-transaction
    /// witness for follow-up reads.
    pub fn fails_with<E>(self, expected: E) -> FailedTransaction
    where
        E: Into<u32>,
    {
        self.0.fails_with(expected)
    }

    /// Assert a runtime or non-custom program error, yielding the
    /// failed-transaction witness for follow-up reads.
    pub fn fails(self, expected: ProgramError) -> FailedTransaction {
        self.0.fails(expected)
    }
}

impl Deref for Outcome {
    type Target = parallax_svm::Outcome;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A transaction proven successful by [`Outcome::succeeds`] — the context every
/// [`CheckFn`] runs against.
///
/// A newtype over [`parallax_svm::SucceededTransaction`] so quasar-test owns
/// the chain: `check`/`checks` delegate to Parallax and stay chainable, while
/// all outcome reads remain available through [`Deref`].
pub struct SucceededTransaction(parallax_svm::SucceededTransaction);

impl SucceededTransaction {
    /// Run one check or one [`bundle`](parallax_svm::bundle) — built-in
    /// Parallax facts (`Cu`, `Account::lamports`, `Account::created`, ...),
    /// quasar-test's strict [`State`], closures, and bundles of any of them.
    /// Chainable.
    pub fn check(&self, check: CheckFn) -> &Self {
        self.0.check(check);
        self
    }

    /// Run several checks and/or bundles. Chainable.
    pub fn checks(&self, checks: impl IntoIterator<Item = CheckFn>) -> &Self {
        self.0.checks(checks);
        self
    }
}

impl Deref for SucceededTransaction {
    type Target = parallax_svm::SucceededTransaction;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Typed account-state facts, validated the quasar way.
///
/// `State::of(address, predicate)` reads the account at `address` through `T`'s
/// on-chain wrapper with the same ownership, discriminator, length, and
/// zero-copy validation as [`Test::read`](crate::Test::read) — quasar-test's
/// strict sibling of Parallax's schema-only `Account::data`. It returns a
/// [`CheckFn`], so these facts group in the same `checks([..])` arrays as the
/// built-ins.
pub struct State;

impl State {
    /// Assert `predicate` holds for the decoded, validated state of the account
    /// at `address`. The account is read through `T`'s on-chain wrapper with
    /// full ownership, discriminator, length, and zero-copy validation.
    pub fn of<T>(address: Pubkey, predicate: impl Fn(&T::Target) -> bool + 'static) -> CheckFn
    where
        T: Discriminator + Owner + Deref + 'static,
        T::Target: ZcElem + ZcValidate + Copy,
    {
        CheckFn::new(move |tx| {
            let name = core::any::type_name::<T>();
            let account = tx.account(address).unwrap_or_else(|| {
                panic!("State {name}: transaction does not contain account {address}")
            });
            let state = crate::world::validate_typed::<T>("State", account);
            assert!(predicate(&state), "State predicate failed for {address}");
        })
    }
}
