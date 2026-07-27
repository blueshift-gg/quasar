//! Composable fixtures for common Solana accounts and programs.
//!
//! The fixture types are re-exported unchanged from Parallax. The [`Fixture`]
//! trait, however, is quasar-test's own: its `install` receives quasar-test's
//! [`Ctx`], so an application fixture can use [`Ctx::derive_pda`] and
//! [`Ctx::write`] while installing. The built-in fixtures below delegate to
//! their Parallax implementations.

use crate::{Account, Ctx, Pubkey};

pub use parallax_svm::fixture::{
    AssociatedTokenAccount, Dump, DumpAccounts, DumpProgram, DumpRefresh, Load, LoadAccounts,
    LoadProgram, Mint, Program, TokenAccount, TokenProgram, Wallet,
};

/// State that can install itself into a test world.
///
/// Applications can implement this trait for protocol-level fixtures, but the
/// composition algebra usually suffices: tuples install heterogeneous worlds
/// in one `add` (`ctx.add((Wallet::account(), Mint::account()))`), arrays
/// repeat one fixture type, and closures receiving `&mut Ctx` are fixtures
/// whose return value is the output — the dependency mechanism for worlds
/// where later fixtures need earlier handles. Each fixture returns the
/// address(es) it placed, so tests thread those handles instead of pinning
/// addresses up front.
pub trait Fixture {
    /// Handle or state returned after installation.
    type Output;

    /// Install the fixture and return the handles needed by the test.
    fn install(self, ctx: &mut Ctx) -> Self::Output;
}

impl<F: Fixture, const N: usize> Fixture for [F; N] {
    type Output = [F::Output; N];

    fn install(self, ctx: &mut Ctx) -> Self::Output {
        self.map(|fixture| fixture.install(ctx))
    }
}

/// Delegate a built-in fixture's installation to its Parallax implementation.
macro_rules! delegate_fixture {
    ($($ty:ty),+ $(,)?) => {$(
        impl Fixture for $ty {
            type Output = Pubkey;

            fn install(self, ctx: &mut Ctx) -> Self::Output {
                parallax_svm::fixture::Fixture::install(self, &mut ctx.0)
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
    DumpProgram,
    LoadProgram,
);

/// Delegate Parallax fixtures whose output is the full installed address list.
macro_rules! delegate_addresses_fixture {
    ($($ty:ty),+ $(,)?) => {$(
        impl Fixture for $ty {
            type Output = Vec<Pubkey>;

            fn install(self, ctx: &mut Ctx) -> Self::Output {
                parallax_svm::fixture::Fixture::install(self, &mut ctx.0)
            }
        }
    )+};
}

delegate_addresses_fixture!(DumpRefresh, LoadAccounts);

/// Closures are fixtures, as in Parallax — but here they receive quasar-test's
/// [`Ctx`], so a world can also use [`Ctx::write`], [`Ctx::derive_pda`], and
/// register invariants while building.
impl<O, F: FnOnce(&mut Ctx) -> O> Fixture for F {
    type Output = O;

    fn install(self, ctx: &mut Ctx) -> O {
        self(ctx)
    }
}

macro_rules! impl_fixture_for_tuple {
    ($($name:ident),+) => {
        /// Tuples are fixtures: one `add` installs a heterogeneous world, in
        /// order, and destructures its handles.
        impl<$($name: Fixture),+> Fixture for ($($name,)+) {
            type Output = ($($name::Output,)+);

            fn install(self, ctx: &mut Ctx) -> Self::Output {
                #[allow(non_snake_case)]
                let ($($name,)+) = self;
                ($($name.install(ctx),)+)
            }
        }
    };
}

impl_fixture_for_tuple!(A, B);
impl_fixture_for_tuple!(A, B, C);
impl_fixture_for_tuple!(A, B, C, D);
impl_fixture_for_tuple!(A, B, C, D, E);

/// Delegate Parallax's const-generic plural fixtures.
macro_rules! delegate_plural_fixture {
    ($($ty:ident),+ $(,)?) => {$(
        impl<const N: usize> Fixture for parallax_svm::fixture::$ty<N> {
            type Output = [Pubkey; N];

            fn install(self, ctx: &mut Ctx) -> Self::Output {
                parallax_svm::fixture::Fixture::install(self, &mut ctx.0)
            }
        }
    )+};
}

delegate_plural_fixture!(DumpAccounts, Mints, Wallets, TokenAccounts);
