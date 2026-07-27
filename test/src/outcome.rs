//! The [`Outcome`] of executing a transaction.
//!
//! `Outcome` is a newtype over [`parallax_svm::Outcome`]. Reporting accessors
//! (`logs`, `account`, `events`, ...) are reached through [`Deref`];
//! `check`/`checks` are re-declared so a chain stays in quasar-test's type,
//! and the verdict facts — [`Outcome::success`] / [`Outcome::error`] — are
//! re-declared so the prelude's `Outcome` names them exactly as Parallax's
//! does. Facts self-diagnose: evaluated against a failed transaction they
//! panic with the transaction's error and logs.
//!
//! The [`State`] facts here validate the quasar way — ownership,
//! discriminator, length, and zero-copy validation — replacing Parallax's
//! schema-only `Account::data` predicate in this crate's prelude.

use {
    crate::Pubkey,
    parallax_svm::{CheckFn, IntoTransactionError},
    quasar_lang::{
        __zeropod::{ZcElem, ZcValidate},
        traits::{Discriminator, Owner},
    },
    std::ops::Deref,
};

/// The structured result of executing one transaction.
#[must_use = "check the outcome (Outcome::success(), Outcome::error(..), or any fact)"]
pub struct Outcome(parallax_svm::Outcome);

impl Outcome {
    pub(crate) fn new(inner: parallax_svm::Outcome) -> Self {
        Self(inner)
    }

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

    /// Assert the transaction succeeded. Optional before other facts (they
    /// self-diagnose on a failed transaction), and useful to state intent.
    pub fn success() -> CheckFn {
        parallax_svm::Outcome::success()
    }

    /// Assert the transaction failed with exactly this error — a
    /// [`ProgramError`](crate::ProgramError), or a program's typed error
    /// (anything `Into<u32>`).
    pub fn error(expected: impl IntoTransactionError + 'static) -> CheckFn {
        parallax_svm::Outcome::error(expected)
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
/// `State::of(address, predicate)` reads the account at `address` through `T`'s
/// on-chain wrapper with the same ownership, discriminator, length, and
/// zero-copy validation as [`Ctx::read`](crate::Ctx::read) — quasar-test's
/// strict sibling of Parallax's schema-only `Account::data`. It returns a
/// [`CheckFn`], so these facts group in the same `checks([..])` arrays as the
/// built-ins, and it self-diagnoses like every fact.
pub struct State;

impl State {
    /// Assert `predicate` holds for the decoded, validated state of the account
    /// at `address`.
    pub fn of<T>(address: Pubkey, predicate: impl Fn(&T::Target) -> bool + 'static) -> CheckFn
    where
        T: Discriminator + Owner + Deref + 'static,
        T::Target: ZcElem + ZcValidate + Copy,
    {
        CheckFn::new(move |tx| {
            if let Some(error) = tx.failure() {
                let logs = tx.logs();
                let rendered = if logs.is_empty() {
                    String::new()
                } else {
                    format!("\nprogram logs:\n  {}", logs.join("\n  "))
                };
                panic!("fact checked against a failed transaction: {error}{rendered}");
            }
            let name = core::any::type_name::<T>();
            let account = tx.account(address).unwrap_or_else(|| {
                panic!("State {name}: transaction does not contain account {address}")
            });
            let state = crate::world::validate_typed::<T>("State", account);
            assert!(predicate(&state), "State predicate failed for {address}");
        })
    }
}
