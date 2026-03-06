use crate::cpi::system::SYSTEM_PROGRAM_ID;
use crate::prelude::*;

/// Realloc an account to `new_space` bytes, transferring lamports to/from `payer`
/// to maintain rent-exemption. Used by `Account::realloc` and generated view types.
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
/// `Account<T>` is `#[repr(transparent)]` over `T`, the view type. This
/// enables two construction paths:
///
/// - **Static accounts** (`T: StaticView`): `T` is `#[repr(transparent)]`
///   over `AccountView`. Construction via pointer cast from `&AccountView`.
///
/// - **Dynamic accounts**: `T` carries `&'info AccountView` + cached byte
///   offsets for O(1) field access. Construction by value via `T::parse()`.
///
/// ## Zero-copy access (T: Deref)
///
/// When `T` implements `Deref`, `Account<T>` provides transparent `Deref`
/// to `T::Target` (the ZC companion struct):
///
/// ```ignore
/// let amount = ctx.accounts.token.amount(); // via Deref<Target = TokenAccountState>
/// ```
#[repr(transparent)]
pub struct Account<T> {
    pub(crate) inner: T,
}

impl<T: AsAccountView> AsAccountView for Account<T> {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        self.inner.to_account_view()
    }
}

impl<T> Account<T> {
    /// Construct an `Account<T>` by wrapping a view value.
    ///
    /// Used by dynamic accounts where T carries cached offsets and
    /// is constructed by-value via `T::parse()`.
    #[inline(always)]
    pub fn wrap(inner: T) -> Self {
        Account { inner }
    }
}

impl<T: AsAccountView> Account<T> {
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

impl<T: Owner + AsAccountView> Account<T> {
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

/// Static account construction — pointer cast from `&AccountView`.
///
/// Requires `T: StaticView` which guarantees the repr(transparent) chain:
/// `Account<T>` → `T` → `AccountView`.
impl<T: CheckOwner + AccountCheck + StaticView> Account<T> {
    #[inline(always)]
    pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
        T::check_owner(view)?;
        T::check(view)?;
        Ok(unsafe { &*(view as *const AccountView as *const Self) })
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
    /// # Safety
    ///
    /// Caller must guarantee:
    /// 1. The account is not a duplicate (borrow_state == 0xFF)
    /// 2. The account is writable (is_writable == 1)
    /// 3. Owner has been validated via `T::check_owner(view)`
    /// 4. Discriminator has been validated via `T::check(view)`
    ///
    /// This function uses `invalid_reference_casting` to convert `&AccountView`
    /// to `&mut Self`, which is safe because `Self` is `#[repr(transparent)]`
    /// over `AccountView` and uses interior mutability.
    #[inline(always)]
    #[allow(invalid_reference_casting, clippy::mut_from_ref)]
    pub unsafe fn from_account_view_unchecked_mut(view: &AccountView) -> &mut Self {
        &mut *(view as *const AccountView as *mut Self)
    }
}

/// Deref: `Account<T>` exposes the inner view type T.
///
/// For static accounts: `Account<Wallet>` → `&Wallet` → auto-deref → `&WalletZc`
/// For dynamic accounts: `Account<Profile<'info>>` → `&Profile<'info>` → auto-deref → `&ProfileZc`
///
/// Methods on T (get/set, accessors) are found at the first deref level.
/// Fields on T::Target (ZC companion struct) are found via auto-deref.
impl<T> core::ops::Deref for Account<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> core::ops::DerefMut for Account<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
