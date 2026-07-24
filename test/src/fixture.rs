//! Composable fixtures for common Solana accounts and programs.
//!
//! The fixture types are re-exported unchanged from Parallax. The [`Fixture`]
//! trait, however, is quasar-test's own: its `install` receives quasar-test's
//! [`Test`], so an application fixture can use [`Test::derive_pda`] and
//! [`Test::write`] while installing. The built-in fixtures below delegate to
//! their Parallax implementations.

use crate::{Account, Pubkey, Test};

pub use parallax_svm::fixture::{
    AssociatedTokenAccount, Mint, Program, TokenAccount, TokenProgram, Wallet,
};

/// State that can install itself into a test world.
///
/// Applications can implement this trait for protocol-level fixtures and
/// compose the built-in account fixtures inside [`Fixture::install`]. Arrays of
/// one fixture type are fixtures too, so repeated setup can be installed with
/// `test.add([Wallet::new(); 3])`. Each fixture returns the address it placed,
/// so tests thread those handles instead of pinning addresses up front.
pub trait Fixture {
    /// Handle or state returned after installation.
    type Output;

    /// Install the fixture and return the handles needed by the test.
    fn install(self, test: &mut Test) -> Self::Output;
}

impl<F: Fixture, const N: usize> Fixture for [F; N] {
    type Output = [F::Output; N];

    fn install(self, test: &mut Test) -> Self::Output {
        self.map(|fixture| fixture.install(test))
    }
}

/// Delegate a built-in fixture's installation to its Parallax implementation,
/// which operates on the wrapped [`parallax_svm::Test`].
macro_rules! delegate_fixture {
    ($($ty:ty),+ $(,)?) => {$(
        impl Fixture for $ty {
            type Output = Pubkey;

            fn install(self, test: &mut Test) -> Self::Output {
                parallax_svm::fixture::Fixture::install(self, &mut test.0)
            }
        }
    )+};
}

delegate_fixture!(
    Account,
    Wallet,
    Mint,
    TokenAccount,
    AssociatedTokenAccount,
    Program<'_>,
);
