use {
    crate::prelude::*,
    solana_account_view::{RuntimeAccount, MAX_PERMITTED_DATA_INCREASE},
};

// Address must be [u8; 32] with alignment 1.
const _: () = {
    assert!(core::mem::size_of::<solana_address::Address>() == 32);
    assert!(core::mem::align_of::<solana_address::Address>() == 1);
};

const _: () = {
    assert!(
        core::mem::offset_of!(RuntimeAccount, padding) == 0x04,
        "RuntimeAccount::padding offset changed; resize() pointer arithmetic is invalid"
    );
};

/// Resize account data. Uses RuntimeAccount::padding (offset 0x04) as i32
/// resize delta tracker.
#[inline(always)]
pub fn resize(view: &mut AccountView, new_len: usize) -> Result<(), ProgramError> {
    let raw = view.account_mut_ptr();

    // SAFETY: `raw` points at the live runtime account behind `view`.
    let current_len =
        i32::try_from(unsafe { (*raw).data_len }).map_err(|_| ProgramError::InvalidRealloc)?;
    let new_len_i32 = i32::try_from(new_len).map_err(|_| ProgramError::InvalidRealloc)?;

    if new_len_i32 == current_len {
        return Ok(());
    }

    let difference = new_len_i32
        .checked_sub(current_len)
        .ok_or(ProgramError::InvalidRealloc)?;

    // SAFETY: `raw` is valid and `padding` is the runtime realloc delta field.
    let delta_ptr = unsafe { core::ptr::addr_of_mut!((*raw).padding) as *mut i32 };
    // SAFETY: `padding` may be unaligned for `i32`, so use unaligned access.
    let accumulated = unsafe { delta_ptr.read_unaligned() }
        .checked_add(difference)
        .ok_or(ProgramError::InvalidRealloc)?;

    if crate::utils::hint::unlikely(accumulated > MAX_PERMITTED_DATA_INCREASE as i32) {
        return Err(ProgramError::InvalidRealloc);
    }

    // SAFETY: `raw` and `delta_ptr` both refer to the same live runtime
    // account. `delta_ptr` uses unaligned access for the i32 padding field.
    unsafe {
        (*raw).data_len = new_len as u64;
        delta_ptr.write_unaligned(accumulated);
    }

    if difference > 0 {
        // Zero-fill extended region (within MAX_PERMITTED_DATA_INCREASE).
        // SAFETY: `data_len` was updated above, so the newly exposed range
        // `[current_len, new_len)` is within the account data allocation.
        unsafe {
            core::ptr::write_bytes(
                view.data_mut_ptr().add(current_len as usize),
                0,
                difference as usize,
            );
        }
    }

    Ok(())
}

/// Set lamports on a shared `&AccountView` via raw pointer cast.
/// Sound on sBPF (no alias-based optimizations); used for cross-account
/// mutations.
#[inline(always)]
pub fn set_lamports(view: &AccountView, lamports: u64) {
    // SAFETY: The SVM account backing `view` is mutable runtime state. This is
    // the same raw lamport write used by account close/realloc paths.
    unsafe { (*(view.account_ptr() as *mut RuntimeAccount)).lamports = lamports };
}

/// Realloc to `new_space` bytes, adjusting lamports for rent-exemption.
#[inline(always)]
pub fn realloc_account(
    view: &mut AccountView,
    new_space: usize,
    payer: &AccountView,
    rent: Option<&crate::sysvars::rent::Rent>,
) -> Result<(), ProgramError> {
    let (rent_lpb, rent_threshold) = if let Some(rent) = rent {
        (rent.lamports_per_byte(), rent.exemption_threshold_raw())
    } else {
        use crate::sysvars::Sysvar;
        let rent = crate::sysvars::rent::Rent::get()?;
        (rent.lamports_per_byte(), rent.exemption_threshold_raw())
    };
    realloc_account_raw(view, new_space, payer, rent_lpb, rent_threshold)
}

/// Realloc with pre-extracted rent values. [`realloc_account`] delegates here.
#[inline(always)]
pub fn realloc_account_raw(
    view: &mut AccountView,
    new_space: usize,
    payer: &AccountView,
    rent_lpb: u64,
    rent_threshold: u64,
) -> Result<(), ProgramError> {
    let rent_exempt_lamports =
        crate::sysvars::rent::minimum_balance_raw(rent_lpb, rent_threshold, new_space as u64)?;

    let current_lamports = view.lamports();

    if rent_exempt_lamports > current_lamports {
        crate::cpi::system::transfer(payer, &*view, rent_exempt_lamports - current_lamports)
            .invoke()?;
    } else if current_lamports > rent_exempt_lamports {
        let excess = current_lamports - rent_exempt_lamports;
        view.set_lamports(rent_exempt_lamports);
        let payer_lamports = payer
            .lamports()
            .checked_add(excess)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        set_lamports(payer, payer_lamports);
    }

    let old_len = view.data_len();

    if new_space < old_len {
        // Zero trailing bytes on shrink.
        // SAFETY: `new_space < old_len`, so the trailing range being cleared
        // is within the account's current data allocation.
        unsafe {
            core::ptr::write_bytes(view.data_mut_ptr().add(new_space), 0, old_len - new_space);
        }
    }

    resize(view, new_space)?;

    Ok(())
}

/// Typed account wrapper. `#[repr(transparent)]` over `T` for pointer-cast
/// construction. Derefs to `T`.
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

// SAFETY: `Account<T>` is `#[repr(transparent)]` over `T`; when `T: StaticView`
// (i.e. `T` is itself transparent over `AccountView`), `Account<T>` is
// transitively transparent over `AccountView`, so the pointer cast is sound.
unsafe impl<T: crate::traits::StaticView> crate::traits::StaticView for Account<T> {}

impl<T: crate::account_layout::AccountLayout> crate::account_layout::AccountLayout for Account<T> {
    type Schema = T::Schema;
    type Target = T::Target;
    const DATA_OFFSET: usize = T::DATA_OFFSET;
}

impl<T: AsAccountView + crate::traits::StaticView + crate::traits::Space> Account<T> {
    /// Resize data region, adjusting lamports for rent-exemption.
    #[inline(always)]
    pub fn realloc(
        &mut self,
        new_space: usize,
        payer: &AccountView,
        rent: Option<&crate::sysvars::rent::Rent>,
    ) -> Result<(), ProgramError> {
        if new_space < <T as crate::traits::Space>::SPACE {
            return Err(ProgramError::AccountDataTooSmall);
        }
        // SAFETY: `Account<T>` is only constructed for account-view-backed
        // wrappers in this impl family; realloc needs the underlying view.
        let view = unsafe { &mut *(self as *mut Account<T> as *mut AccountView) };
        realloc_account(view, new_space, payer, rent)
    }
}

impl<T: Owner + AsAccountView + crate::traits::Discriminator + crate::traits::StaticView>
    Account<T>
{
    /// Close account: zero disc, drain lamports, reassign to system, resize to
    /// zero.
    #[inline(always)]
    pub fn close(&mut self, destination: &AccountView) -> Result<(), ProgramError> {
        // SAFETY: `T: StaticView` guarantees `Account<T>` is `#[repr(transparent)]`
        // over `AccountView`, so this pointer cast is valid. Close operates on the
        // runtime account backing this typed wrapper and does not access `T` after
        // reassigning/resizing it.
        let view = unsafe { &mut *(self as *mut Account<T> as *mut AccountView) };
        crate::ops::close::close_account(
            view,
            destination,
            <T as crate::traits::Discriminator>::DISCRIMINATOR.len(),
        )
    }
}

impl<T: crate::account_load::AccountLoad + CheckOwner + StaticView> Account<T> {
    /// Validate owner + data checks, then pointer-cast.
    #[inline(always)]
    pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
        T::check_owner(view)?;
        T::check(view)?;
        // SAFETY: Owner and account data were validated above.
        Ok(unsafe { Self::from_account_view_unchecked(view) })
    }
}

impl<T> Account<T> {
    /// # Safety
    /// Caller must ensure owner, discriminator, borrow state, and
    /// `AccountView` layout compatibility are valid for `T`.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        // SAFETY: The caller guarantees this `Account<T>` wrapper is layout
        // compatible with `AccountView` and has already satisfied validation.
        unsafe { &*(view as *const AccountView as *const Self) }
    }

    /// # Safety
    /// Same as [`from_account_view_unchecked`](Self::from_account_view_unchecked),
    /// plus the account must be writable.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked_mut(view: &mut AccountView) -> &mut Self {
        // SAFETY: Same layout argument as the immutable cast; the caller also
        // guarantees writable access.
        unsafe { &mut *(view as *mut AccountView as *mut Self) }
    }
}

#[cfg(kani)]
#[path = "../../kani/accounts/account.rs"]
mod kani_proofs;

#[inline(always)]
fn check_owner<T: CheckOwner>(view: &AccountView) -> Result<(), ProgramError> {
    T::check_owner(view).inspect_err(|_| {
        #[cfg(feature = "debug")]
        crate::prelude::log("Owner check failed for account");
    })
}

impl<T: AsAccountView + crate::account_load::AccountLoad + CheckOwner + StaticView>
    crate::account_load::AccountLoad for Account<T>
{
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), solana_program_error::ProgramError> {
        check_owner::<T>(view)?;
        T::check(view)?;
        Ok(())
    }

    #[inline(always)]
    fn check_checked(view: &AccountView) -> Result<(), solana_program_error::ProgramError> {
        check_owner::<T>(view)?;
        T::check_checked(view)?;
        Ok(())
    }

    #[inline(always)]
    fn check_intrinsic(view: &AccountView) -> Result<(), solana_program_error::ProgramError> {
        check_owner::<T>(view)?;
        T::check_intrinsic(view)?;
        Ok(())
    }
}

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

impl<T: crate::account_init::AccountInit> crate::account_init::AccountInit for Account<T> {
    type InitParams<'a> = T::InitParams<'a>;
    const DEFAULT_INIT_PARAMS_VALID: bool = T::DEFAULT_INIT_PARAMS_VALID;

    #[inline(always)]
    fn init<'a, R: crate::ops::RentAccess>(
        ctx: crate::account_init::InitCtx<'a, R>,
        params: &Self::InitParams<'a>,
    ) -> solana_program_error::ProgramResult {
        T::init(ctx, params)
    }
}

impl<T: crate::traits::Space> crate::traits::Space for Account<T> {
    const SPACE: usize = T::SPACE;
}

impl<T: crate::ops::SupportsRealloc> crate::ops::SupportsRealloc for Account<T> {}
