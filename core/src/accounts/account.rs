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
    /// Construct an Account<T> by wrapping a view value.
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
        // SAFETY: Account is repr(transparent) over T, and T: StaticView
        // guarantees T is repr(transparent) over AccountView.
        Ok(unsafe { &*(view as *const AccountView as *const Self) })
    }

    /// # Safety (invalid_reference_casting)
    ///
    /// `Self` is `#[repr(transparent)]` over `T`, which is `#[repr(transparent)]`
    /// over `AccountView`. AccountView uses interior mutability through raw
    /// pointers to SVM account memory. The `&` → `&mut` cast does not create
    /// aliased mutable references to backing memory — all writes go through
    /// `AccountView`'s raw pointer methods.
    #[inline(always)]
    #[allow(invalid_reference_casting, clippy::mut_from_ref)]
    pub fn from_account_view_mut(view: &AccountView) -> Result<&mut Self, ProgramError> {
        if !view.is_writable() {
            return Err(ProgramError::Immutable);
        }
        T::check_owner(view)?;
        T::check(view)?;
        Ok(unsafe { &mut *(view as *const AccountView as *mut Self) })
    }
}

/// Deref: Account<T> exposes the inner view type T.
///
/// For static accounts: Account<Wallet> → &Wallet → auto-deref → &WalletZc
/// For dynamic accounts: Account<Profile<'info>> → &Profile<'info> → auto-deref → &ProfileZc
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
