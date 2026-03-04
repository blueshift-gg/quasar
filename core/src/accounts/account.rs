use crate::cpi::system::SYSTEM_PROGRAM_ID;
use crate::prelude::*;
use core::marker::PhantomData;

/// Realloc an account to `new_space` bytes, transferring lamports to/from `payer`
/// to maintain rent-exemption. Used by `Account::realloc` and generated View types.
#[inline(always)]
pub fn realloc_account(
    view: &AccountView,
    new_space: usize,
    payer: &AccountView,
    rent: Option<&crate::sysvars::rent::Rent>,
) -> Result<(), ProgramError> {
    let rent_exempt_lamports = match rent {
        Some(rent) => rent.try_minimum_balance(new_space)?,
        None => {
            use crate::sysvars::Sysvar;
            crate::sysvars::rent::Rent::get()?.try_minimum_balance(new_space)?
        }
    };

    let current_lamports = view.lamports();

    if rent_exempt_lamports > current_lamports {
        crate::cpi::system::transfer(payer, view, rent_exempt_lamports - current_lamports)
            .invoke()?;
    } else if current_lamports > rent_exempt_lamports {
        let excess = current_lamports - rent_exempt_lamports;
        view.set_lamports(rent_exempt_lamports);
        payer.set_lamports(payer.lamports() + excess);
    }

    let old_len = view.data_len();

    // Zero trailing bytes on shrink to prevent data leakage if the account
    // is later re-grown — the runtime does not zero the realloc region.
    if new_space < old_len {
        // SAFETY: data_ptr() is valid for old_len bytes. The bytes in
        // [new_space..old_len] are within the current allocation.
        unsafe {
            core::ptr::write_bytes(view.data_ptr().add(new_space), 0, old_len - new_space);
        }
    }

    view.resize(new_space)?;

    Ok(())
}

/// Typed account wrapper with composable validation.
///
/// `Account<T>` is the unified wrapper for all validated on-chain accounts.
/// The trait bounds on `T` determine which capabilities are available:
///
/// ## Single-owner accounts (T: Owner)
///
/// ```ignore
/// // Validates owner == SPL Token program
/// pub token: &'info Account<Token>,
/// ```
///
/// Types implementing [`Owner`] get a blanket [`CheckOwner`] impl that
/// compares against a single address (~20 CU).
///
/// ## Multi-owner (interface) accounts (T: CheckOwner)
///
/// ```ignore
/// // Validates owner == SPL Token OR Token-2022
/// pub token: &'info InterfaceAccount<Token>,
/// ```
///
/// Types implementing [`CheckOwner`] directly use explicit comparison
/// chains instead of slice iteration, avoiding ~20-40 CU overhead.
///
/// ## Zero-copy access (T: ZeroCopyDeref)
///
/// When `T` implements [`ZeroCopyDeref`], `Account<T>` provides
/// `Deref`/`DerefMut` to the ZC companion struct:
///
/// ```ignore
/// let amount = ctx.accounts.token.amount(); // via Deref<Target = TokenAccountState>
/// ```
///
/// ## Borsh access (T: QuasarAccount)
///
/// When `T` implements [`QuasarAccount`], `Account<T>` provides
/// `.get()` / `.set()` for Borsh-style (de)serialization.
///
/// ## Polymorphic dispatch (via InterfaceAccount<T>)
///
/// For accounts that can be owned by multiple programs with different layouts,
/// use `InterfaceAccount<T>` (from `quasar_spl`) which provides `.resolve()`
/// for runtime dispatch:
///
/// ```ignore
/// // In your accounts struct:
/// pub oracle: &'info InterfaceAccount<OracleInterface>,
///
/// // In your instruction handler:
/// match ctx.accounts.oracle.resolve()? {
///     OraclePrice::Pyth(price) => { /* read Pyth fields */ }
///     OraclePrice::Switchboard(price) => { /* read Switchboard fields */ }
/// }
/// ```
#[repr(transparent)]
pub struct Account<T> {
    view: AccountView,
    _marker: PhantomData<T>,
}

impl<T> AsAccountView for Account<T> {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.view
    }
}

impl<T> Account<T> {
    #[inline(always)]
    pub fn realloc(
        &self,
        new_space: usize,
        payer: &AccountView,
        rent: Option<&crate::sysvars::rent::Rent>,
    ) -> Result<(), ProgramError> {
        realloc_account(self.to_account_view(), new_space, payer, rent)
    }
}

impl<T: Owner> Account<T> {
    #[inline(always)]
    pub fn owner(&self) -> &'static Address {
        &T::OWNER
    }

    /// Close a program-owned account: zero discriminator, drain lamports,
    /// reassign to system program, and resize to zero.
    ///
    /// Zeroes the discriminator bytes before draining to prevent account revival
    /// attacks within the same transaction.
    ///
    /// Only works for accounts owned by the calling program (i.e. types
    /// implementing [`Owner`]). For token/mint accounts owned by the SPL Token
    /// or Token-2022 programs, use the CPI-based close via the token program.
    #[inline(always)]
    pub fn close(&self, destination: &AccountView) -> Result<(), ProgramError> {
        let view = self.to_account_view();
        if !destination.is_writable() {
            return Err(ProgramError::Immutable);
        }

        // Zero discriminator bytes to prevent revival within the same transaction.
        // SAFETY: data_ptr() is valid for data_len() bytes. We only write up to
        // 8 bytes (max discriminator size) or data_len, whichever is smaller.
        let zero_len = view.data_len().min(8);
        if zero_len > 0 {
            unsafe {
                core::ptr::write_bytes(view.data_ptr(), 0, zero_len);
            }
        }

        let new_lamports = destination
            .lamports()
            .checked_add(view.lamports())
            .ok_or(ProgramError::InvalidArgument)?;
        destination.set_lamports(new_lamports);
        view.set_lamports(0);
        unsafe { view.assign(&SYSTEM_PROGRAM_ID) };
        view.resize(0)?;
        Ok(())
    }
}

impl<T: CheckOwner + AccountCheck> Account<T> {
    /// Unchecked construction for optimized parsing where all flag checks
    /// (signer/writable/executable/no-dup) have been pre-validated via u32
    /// header comparison during entrypoint deserialization.
    ///
    /// # Safety
    ///
    /// Caller must guarantee:
    /// 1. The account is not a duplicate (borrow_state == 0xFF)
    /// 2. Owner has been validated via `T::check_owner(view)`
    /// 3. Discriminator has been validated via `T::check(view)`
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        &*(view as *const AccountView as *const Self)
    }

    /// Unchecked mutable construction for optimized parsing.
    ///
    /// # Safety (invalid_reference_casting + validation requirements)
    ///
    /// Caller must guarantee:
    /// 1. The account is not a duplicate (borrow_state == 0xFF)
    /// 2. The account is writable (is_writable == 1)
    /// 3. Owner has been validated via `T::check_owner(view)`
    /// 4. Discriminator has been validated via `T::check(view)`
    #[inline(always)]
    #[allow(invalid_reference_casting, clippy::mut_from_ref)]
    pub unsafe fn from_account_view_unchecked_mut(view: &AccountView) -> &mut Self {
        &mut *(view as *const AccountView as *mut Self)
    }
}

impl<T: QuasarAccount> Account<T> {
    #[inline(always)]
    pub fn get(&self) -> Result<T, ProgramError> {
        let data = self.view.try_borrow()?;
        let disc = T::DISCRIMINATOR;
        if data.len() < disc.len() || &data[..disc.len()] != disc {
            return Err(ProgramError::InvalidAccountData);
        }
        T::deserialize(&data[disc.len()..])
    }

    #[inline(always)]
    pub fn set(&mut self, value: &T) -> Result<(), ProgramError> {
        let mut data = self.view.try_borrow_mut()?;
        let disc = T::DISCRIMINATOR;
        value.serialize(&mut data[disc.len()..])
    }
}

impl<T: ZeroCopyDeref> core::ops::Deref for Account<T> {
    type Target = T::Target;

    /// SAFETY: Bounds validated by `AccountCheck::check` during `from_account_view`.
    /// For fixed accounts, the target is a ZC companion struct with alignment 1.
    /// For dynamic accounts, the target is a `#[repr(transparent)]` View over AccountView.
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        T::deref_from(&self.view)
    }
}

impl<T: ZeroCopyDeref> core::ops::DerefMut for Account<T> {
    /// SAFETY: Same as Deref — bounds checked upstream.
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        T::deref_from_mut(&self.view)
    }
}
