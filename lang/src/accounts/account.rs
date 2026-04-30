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
        "RuntimeAccount::padding offset changed — resize() pointer arithmetic is invalid"
    );
};

/// Resize account data. Uses RuntimeAccount::padding (offset 0x04) as i32
/// resize delta tracker.
#[inline(always)]
pub fn resize(view: &mut AccountView, new_len: usize) -> Result<(), ProgramError> {
    let raw = view.account_mut_ptr();

    let current_len =
        i32::try_from(unsafe { (*raw).data_len }).map_err(|_| ProgramError::InvalidRealloc)?;
    let new_len_i32 = i32::try_from(new_len).map_err(|_| ProgramError::InvalidRealloc)?;

    if new_len_i32 == current_len {
        return Ok(());
    }

    let difference = new_len_i32 - current_len;

    let delta_ptr = unsafe { core::ptr::addr_of_mut!((*raw).padding) as *mut i32 };
    let accumulated = unsafe { delta_ptr.read_unaligned() } + difference;

    if crate::utils::hint::unlikely(accumulated > MAX_PERMITTED_DATA_INCREASE as i32) {
        return Err(ProgramError::InvalidRealloc);
    }

    unsafe {
        (*raw).data_len = new_len as u64;
        delta_ptr.write_unaligned(accumulated);
    }

    if difference > 0 {
        // Zero-fill extended region (within MAX_PERMITTED_DATA_INCREASE).
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
    let r = if let Some(r) = rent {
        r.clone()
    } else {
        use crate::sysvars::Sysvar;
        crate::sysvars::rent::Rent::get()?
    };
    realloc_account_raw(
        view,
        new_space,
        payer,
        r.lamports_per_byte(),
        r.exemption_threshold_raw(),
    )
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
        set_lamports(payer, payer.lamports() + excess);
    }

    let old_len = view.data_len();

    if new_space < old_len {
        // Zero trailing bytes on shrink.
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

impl<T> Account<T> {
    /// Wrap a view value (used by dynamic accounts).
    #[inline(always)]
    pub fn wrap(inner: T) -> Self {
        Account { inner }
    }
}

impl<T: AsAccountView + crate::traits::StaticView> Account<T> {
    /// Resize data region, adjusting lamports for rent-exemption.
    #[inline(always)]
    pub fn realloc(
        &mut self,
        new_space: usize,
        payer: &AccountView,
        rent: Option<&crate::sysvars::rent::Rent>,
    ) -> Result<(), ProgramError> {
        let view = unsafe { &mut *(self as *mut Account<T> as *mut AccountView) };
        realloc_account(view, new_space, payer, rent)
    }
}

impl<T: Owner + AsAccountView + crate::traits::Discriminator> Account<T> {
    /// Close account: zero disc, drain lamports, reassign to system, resize to
    /// zero.
    #[inline(always)]
    pub fn close(&mut self, destination: &AccountView) -> Result<(), ProgramError> {
        let view = unsafe { &mut *(self as *mut Account<T> as *mut AccountView) };
        crate::ops::close_program::close_program_account(
            view,
            destination,
            <T as crate::traits::Discriminator>::DISCRIMINATOR.len(),
        )
    }
}

impl<T: CheckOwner + AccountCheck + StaticView> Account<T> {
    /// Validate owner + discriminator, then pointer-cast.
    #[inline(always)]
    pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
        T::check_owner(view)?;
        T::check(view)?;
        Ok(unsafe { &*(view as *const AccountView as *const Self) })
    }
}

impl<T: CheckOwner + AccountCheck> Account<T> {
    /// # Safety
    /// Caller must ensure owner, discriminator, and borrow state are valid.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        &*(view as *const AccountView as *const Self)
    }

    /// # Safety
    /// Same as above, plus account must be writable.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked_mut(view: &mut AccountView) -> &mut Self {
        &mut *(view as *mut AccountView as *mut Self)
    }
}

#[cfg(kani)]
mod kani_proofs {
    use {super::*, solana_account_view::MAX_PERMITTED_DATA_INCREASE};

    #[kani::proof]
    fn resize_delta_no_overflow() {
        let current_len: i32 = kani::any();
        let new_len: i32 = kani::any();
        kani::assume(current_len >= 0);
        kani::assume(new_len >= 0);
        kani::assume(current_len <= 10 * 1024 * 1024);
        kani::assume(new_len <= 10 * 1024 * 1024);

        let difference = new_len - current_len;

        let prior_accumulated: i32 = kani::any();
        kani::assume(prior_accumulated >= -(MAX_PERMITTED_DATA_INCREASE as i32));
        kani::assume(prior_accumulated <= MAX_PERMITTED_DATA_INCREASE as i32);

        assert!(prior_accumulated.checked_add(difference).is_some());
    }

    #[kani::proof]
    fn padding_i32_roundtrip() {
        let value: i32 = kani::any();
        let mut buf = [0u8; 4];
        unsafe {
            core::ptr::copy_nonoverlapping(&value as *const i32 as *const u8, buf.as_mut_ptr(), 4);
        }
        let read_back = unsafe { (buf.as_ptr() as *const i32).read_unaligned() };
        assert!(read_back == value);
    }

    #[kani::proof]
    fn account_repr_transparent_size() {
        use solana_account_view::AccountView;

        assert!(
            core::mem::size_of::<Account<AccountView>>() == core::mem::size_of::<AccountView>()
        );
        assert!(
            core::mem::align_of::<Account<AccountView>>() == core::mem::align_of::<AccountView>()
        );
    }

    #[kani::proof]
    fn set_lamports_field_offset_stable() {
        let offset = core::mem::offset_of!(RuntimeAccount, lamports);
        assert!(offset < core::mem::size_of::<RuntimeAccount>());
        assert!(offset + core::mem::size_of::<u64>() <= core::mem::size_of::<RuntimeAccount>());
    }

    #[kani::proof]
    fn realloc_lamport_subtraction_no_underflow() {
        let rent_exempt: u64 = kani::any();
        let current: u64 = kani::any();

        if rent_exempt > current {
            let deficit = rent_exempt - current;
            assert!(deficit > 0);
            assert!(deficit <= rent_exempt);
        } else if current > rent_exempt {
            let excess = current - rent_exempt;
            assert!(excess > 0);
            assert!(excess <= current);
        }
    }

    #[kani::proof]
    fn realloc_excess_addition_no_overflow() {
        let payer_lamports: u64 = kani::any();
        let excess: u64 = kani::any();

        const MAX_SOL_SUPPLY: u64 = 600_000_000_000_000_000;
        kani::assume(payer_lamports <= MAX_SOL_SUPPLY);
        kani::assume(excess <= MAX_SOL_SUPPLY);
        kani::assume(payer_lamports + excess <= MAX_SOL_SUPPLY);

        assert!(payer_lamports.checked_add(excess).is_some());
    }

    #[kani::proof]
    fn close_lamports_wrapping_add_equivalent_to_checked() {
        let dest_lamports: u64 = kani::any();
        let view_lamports: u64 = kani::any();

        const MAX_SOL_SUPPLY: u64 = 600_000_000_000_000_000;
        kani::assume(dest_lamports <= MAX_SOL_SUPPLY);
        kani::assume(view_lamports <= MAX_SOL_SUPPLY);

        let wrapping_result = dest_lamports.wrapping_add(view_lamports);
        let checked_result = dest_lamports.checked_add(view_lamports);
        assert!(checked_result.is_some());
        assert!(wrapping_result == checked_result.unwrap());
    }

    #[kani::proof]
    fn resize_write_bytes_region_valid() {
        let current_len: i32 = kani::any();
        let new_len: i32 = kani::any();
        kani::assume(current_len >= 0);
        kani::assume(new_len >= 0);
        kani::assume(current_len <= 10 * 1024 * 1024);
        kani::assume(new_len <= 10 * 1024 * 1024);

        let difference = new_len - current_len;
        if difference > 0 {
            let start = current_len as usize;
            let count = difference as usize;
            let end = start.checked_add(count);
            assert!(end.is_some());
            assert!(end.unwrap() == new_len as usize);
            assert!(start <= end.unwrap());
        }
    }
}

impl<T: AsAccountView + CheckOwner + AccountCheck + StaticView> crate::account_load::AccountLoad
    for Account<T>
{
    type BehaviorTarget = T;

    #[inline(always)]
    fn check(
        view: &AccountView,
        field_name: &str,
    ) -> Result<(), solana_program_error::ProgramError> {
        crate::validation::check_account::<T>(view, field_name)
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

impl<T> crate::traits::FieldLifecycle for Account<T> {}
