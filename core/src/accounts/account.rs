use crate::cpi::system::SYSTEM_PROGRAM_ID;
use crate::prelude::*;
use solana_account_view::{RuntimeAccount, MAX_PERMITTED_DATA_INCREASE};

/// Resize account data, tracking the accumulated delta in the padding field.
///
/// Upstream v2 removed `resize()`. This reimplements it using the `padding`
/// bytes (which replaced v1's `resize_delta: i32`) as an i32 resize delta.
#[inline(always)]
pub fn resize(view: &mut AccountView, new_len: usize) -> Result<(), ProgramError> {
    let raw = view.account_mut_ptr();
    let current_len = unsafe { (*raw).data_len } as i32;
    let new_len_i32 = i32::try_from(new_len).map_err(|_| ProgramError::InvalidRealloc)?;

    if new_len_i32 == current_len {
        return Ok(());
    }

    let difference = new_len_i32 - current_len;

    let delta_ptr = unsafe { core::ptr::addr_of_mut!((*raw).padding) as *mut i32 };
    let accumulated = unsafe { delta_ptr.read_unaligned() } + difference;

    if accumulated > MAX_PERMITTED_DATA_INCREASE as i32 {
        return Err(ProgramError::InvalidRealloc);
    }

    unsafe {
        (*raw).data_len = new_len as u64;
        delta_ptr.write_unaligned(accumulated);
    }

    if difference > 0 {
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

/// Set lamports on a shared `&AccountView` for cross-account mutations.
///
/// Used when two accounts from a parsed context both need lamport writes
/// (e.g. close drains to destination, realloc returns excess to payer).
#[inline(always)]
pub fn set_lamports(view: &AccountView, lamports: u64) {
    unsafe { (*(view.account_ptr() as *mut RuntimeAccount)).lamports = lamports };
}

/// Realloc an account to `new_space` bytes, adjusting lamports for rent-exemption.
#[inline(always)]
pub fn realloc_account(
    view: &mut AccountView,
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
        crate::cpi::system::transfer(payer, &*view, rent_exempt_lamports - current_lamports)
            .invoke()?;
    } else if current_lamports > rent_exempt_lamports {
        let excess = current_lamports - rent_exempt_lamports;
        view.set_lamports(rent_exempt_lamports);
        set_lamports(payer, payer.lamports() + excess);
    }

    let old_len = view.data_len();

    // Zero trailing bytes on shrink — the runtime does not zero the realloc region.
    if new_space < old_len {
        unsafe {
            core::ptr::write_bytes(view.data_mut_ptr().add(new_space), 0, old_len - new_space);
        }
    }

    resize(view, new_space)?;

    Ok(())
}

/// Typed account wrapper with composable validation.
///
/// `#[repr(transparent)]` over `T`. Static accounts (`T: StaticView`)
/// construct via pointer cast; dynamic accounts carry cached byte offsets.
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
    /// Wrap a view value. Used by dynamic accounts constructed via `T::parse()`.
    #[inline(always)]
    pub fn wrap(inner: T) -> Self {
        Account { inner }
    }
}

impl<T: AsAccountView> Account<T> {
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

impl<T: Owner + AsAccountView> Account<T> {
    #[inline(always)]
    pub fn owner(&self) -> &'static Address {
        &T::OWNER
    }

    /// Close a program-owned account: zero discriminator, drain lamports,
    /// reassign to system program, resize to zero.
    ///
    /// For token/mint accounts, use the CPI-based `TokenClose` trait instead.
    #[inline(always)]
    pub fn close(&mut self, destination: &AccountView) -> Result<(), ProgramError> {
        let view = unsafe { &mut *(self as *mut Account<T> as *mut AccountView) };
        if !destination.is_writable() {
            return Err(ProgramError::Immutable);
        }

        // Zero discriminator to prevent revival within the same transaction.
        let zero_len = view.data_len().min(8);
        unsafe { core::ptr::write_bytes(view.data_mut_ptr(), 0, zero_len) };

        // wrapping_add: total SOL supply (~5.8e17) fits within u64::MAX.
        let new_lamports = destination.lamports().wrapping_add(view.lamports());
        set_lamports(destination, new_lamports);
        view.set_lamports(0);
        unsafe { view.assign(&SYSTEM_PROGRAM_ID) };
        resize(view, 0)?;
        Ok(())
    }
}

/// Static account construction via pointer cast from `&AccountView`.
impl<T: CheckOwner + AccountCheck + StaticView> Account<T> {
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
    /// Caller must ensure owner, discriminator, borrow state, and writability.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked_mut(view: &mut AccountView) -> &mut Self {
        &mut *(view as *mut AccountView as *mut Self)
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
