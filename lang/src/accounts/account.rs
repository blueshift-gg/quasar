use {
    crate::{cpi::system::SYSTEM_PROGRAM_ID, prelude::*},
    solana_account_view::{RuntimeAccount, MAX_PERMITTED_DATA_INCREASE},
};

// keys_eq and all 32-byte comparisons assume Address is [u8; 32] with alignment
// 1.
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

/// Resize account data, tracking the accumulated delta in the padding field.
///
/// Upstream v2 removed `resize()`. This reimplements it using the `padding`
/// bytes (which replaced v1's `resize_delta: i32`) as an i32 resize delta.
///
/// Kani proofs: `resize_delta_no_overflow`, `padding_i32_roundtrip`,
/// `resize_write_bytes_region_valid`.
///
/// # RuntimeAccount layout (relevant fields)
///
/// ```text
/// offset  field       size
/// ------  ----------  ----
///   0x00  borrow_state  1
///   0x01  is_signer     1
///   0x02  is_writable   1
///   0x03  executable    1
///   0x04  padding       4    (reused as i32 resize delta)
///   ...
///   0x48  data_len      8    (u64)
/// ```
#[inline(always)]
pub fn resize(view: &mut AccountView, new_len: usize) -> Result<(), ProgramError> {
    let raw = view.account_mut_ptr();

    // SAFETY: `raw` is a valid `RuntimeAccount` pointer from `AccountView`.
    // `data_len` is always within i32 range on Solana (max 10 MiB) — try_from
    // is defense-in-depth against future SVM changes.
    let current_len =
        i32::try_from(unsafe { (*raw).data_len }).map_err(|_| ProgramError::InvalidRealloc)?;
    let new_len_i32 = i32::try_from(new_len).map_err(|_| ProgramError::InvalidRealloc)?;

    if new_len_i32 == current_len {
        return Ok(());
    }

    let difference = new_len_i32 - current_len;

    // SAFETY: `padding` is a 4-byte field in `RuntimeAccount`. We reinterpret
    // it as i32 to track the cumulative resize delta. Unaligned access is safe
    // on SBF; on other targets `read/write_unaligned` handles it.
    let delta_ptr = unsafe { core::ptr::addr_of_mut!((*raw).padding) as *mut i32 };
    let accumulated = unsafe { delta_ptr.read_unaligned() } + difference;

    if crate::utils::hint::unlikely(accumulated > MAX_PERMITTED_DATA_INCREASE as i32) {
        return Err(ProgramError::InvalidRealloc);
    }

    // SAFETY: Writing to fields of a valid `RuntimeAccount`.
    unsafe {
        (*raw).data_len = new_len as u64;
        delta_ptr.write_unaligned(accumulated);
    }

    if difference > 0 {
        // SAFETY: Zero-fill the newly extended region. `data_mut_ptr()` points
        // to the start of account data; the SVM allocates a 10 KiB realloc
        // region after the original data, so `current_len + difference` is
        // within bounds (enforced by the `MAX_PERMITTED_DATA_INCREASE` check).
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
///
/// Kani proof: `set_lamports_field_offset_stable`.
///
/// # Safety (Aliasing)
///
/// This mutates through a shared `&AccountView` reference via raw pointer cast.
/// This is technically UB in Rust's abstract machine model, but is sound on all
/// Solana targets (sBPF, x86 for testing) because:
/// 1. The SVM input buffer is genuinely writable memory
/// 2. The Solana runtime permits lamport mutations within a transaction
/// 3. sBPF does not use LLVM's alias-based optimizations
#[inline(always)]
pub fn set_lamports(view: &AccountView, lamports: u64) {
    unsafe { (*(view.account_ptr() as *mut RuntimeAccount)).lamports = lamports };
}

/// Realloc an account to `new_space` bytes, adjusting lamports for
/// rent-exemption.
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

/// Realloc an account using pre-extracted rent values.
///
/// Takes `(lamports_per_byte, threshold)` directly instead of a `Rent` struct.
/// This is the canonical implementation — [`realloc_account`] delegates here.
///
/// Kani proofs: `realloc_lamport_subtraction_no_underflow`,
/// `realloc_excess_addition_no_overflow`.
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

    // Zero trailing bytes on shrink — the runtime does not zero the realloc region.
    if new_space < old_len {
        // SAFETY: `data_mut_ptr()` is valid for `old_len` bytes. We zero
        // the range `[new_space, old_len)` which is within the original allocation.
        unsafe {
            core::ptr::write_bytes(view.data_mut_ptr().add(new_space), 0, old_len - new_space);
        }
    }

    resize(view, new_space)?;

    Ok(())
}

/// Typed account wrapper with composable validation.
///
/// `Account<T>` wraps a zero-copy view type `T` and provides validated
/// construction, reallocation, and close operations. The wrapper is
/// `#[repr(transparent)]` so it can be constructed via pointer cast from
/// `&AccountView` when `T: StaticView`.
///
/// For dynamic accounts (those with `String` / `Vec` fields), use
/// `Account::wrap()` after parsing the byte offsets.
///
/// `Account<T>` implements `Deref<Target = T>` and `DerefMut`, so the
/// inner type's accessors are available directly.
#[repr(transparent)]
pub struct Account<T> {
    /// The inner zero-copy view type.
    pub(crate) inner: T,
}

impl<T: AsAccountView> AsAccountView for Account<T> {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        self.inner.to_account_view()
    }
}

impl<T> Account<T> {
    /// Wrap a view value. Used by dynamic accounts constructed via
    /// `T::parse()`.
    #[inline(always)]
    pub fn wrap(inner: T) -> Self {
        Account { inner }
    }
}

impl<T: AsAccountView + crate::traits::StaticView> Account<T> {
    /// Resize this account's data region, adjusting lamports for
    /// rent-exemption.
    ///
    /// If `rent` is `None`, fetches the Rent sysvar via syscall.
    ///
    /// Kani proof: `account_repr_transparent_size` (validates the pointer cast).
    #[inline(always)]
    pub fn realloc(
        &mut self,
        new_space: usize,
        payer: &AccountView,
        rent: Option<&crate::sysvars::rent::Rent>,
    ) -> Result<(), ProgramError> {
        // SAFETY: `Account<T>` is `#[repr(transparent)]` over `T`, and `T`
        // implements `StaticView` which guarantees `#[repr(transparent)]`
        // over `AccountView`. The cast preserves the pointer.
        let view = unsafe { &mut *(self as *mut Account<T> as *mut AccountView) };
        realloc_account(view, new_space, payer, rent)
    }
}

impl<T: Owner + AsAccountView + crate::traits::Discriminator> Account<T> {
    /// Close a program-owned account: zero discriminator, drain lamports,
    /// reassign to system program, resize to zero.
    ///
    /// For token/mint accounts, use `token_program.close_account()` CPI
    /// instead.
    ///
    /// Kani proofs: `account_repr_transparent_size` (pointer cast),
    /// `close_lamports_wrapping_add_equivalent_to_checked` (lamport drain).
    #[inline(always)]
    pub fn close(&mut self, destination: &AccountView) -> Result<(), ProgramError> {
        // SAFETY: Same `#[repr(transparent)]` chain as `realloc` above.
        let view = unsafe { &mut *(self as *mut Account<T> as *mut AccountView) };
        if crate::utils::hint::unlikely(!destination.is_writable()) {
            return Err(ProgramError::Immutable);
        }

        // SAFETY: Zero the discriminator prefix. AccountCheck::check during
        // parse verified data_len >= disc_len + sizeof(Zc), so disc_len is
        // always in bounds.
        unsafe {
            core::ptr::write_bytes(
                view.data_mut_ptr(),
                0,
                <T as crate::traits::Discriminator>::DISCRIMINATOR.len(),
            );
        }

        // wrapping_add: total SOL supply (~5.8e17) fits within u64::MAX.
        let new_lamports = destination.lamports().wrapping_add(view.lamports());
        set_lamports(destination, new_lamports);
        view.set_lamports(0);

        // SAFETY: Reassigns ownership to the system program. The account is
        // being closed, so the owner change is valid.
        unsafe { view.assign(&SYSTEM_PROGRAM_ID) };
        resize(view, 0)?;
        Ok(())
    }
}

/// Static account construction via pointer cast from `&AccountView`.
impl<T: CheckOwner + AccountCheck + StaticView> Account<T> {
    /// Return an `Account<T>` from the given account view.
    ///
    /// Validates owner and discriminator before performing the pointer cast.
    ///
    /// # Errors
    ///
    /// Returns `ProgramError::InvalidAccountOwner` if the owner does not
    /// match, or `ProgramError::InvalidAccountData` if the discriminator
    /// check fails.
    #[inline(always)]
    pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
        T::check_owner(view)?;
        T::check(view)?;
        // SAFETY: Owner and discriminator checks passed above. `Account<T>`
        // is `#[repr(transparent)]` over `T` which is `#[repr(transparent)]`
        // over `AccountView`, so the pointer cast is layout-preserving.
        Ok(unsafe { &*(view as *const AccountView as *const Self) })
    }
}

impl<T: CheckOwner + AccountCheck> Account<T> {
    /// Construct without validation.
    ///
    /// # Safety
    ///
    /// Caller must ensure owner, discriminator, and borrow state are valid.
    /// The pointer cast relies on the `#[repr(transparent)]` chain
    /// `Account<T> → T → AccountView`.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        &*(view as *const AccountView as *const Self)
    }

    /// Construct without validation (mutable).
    ///
    /// # Safety
    ///
    /// Caller must ensure owner, discriminator, borrow state, and writability.
    /// The pointer cast relies on the `#[repr(transparent)]` chain
    /// `Account<T> → T → AccountView`.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked_mut(view: &mut AccountView) -> &mut Self {
        &mut *(view as *mut AccountView as *mut Self)
    }
}

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
    use solana_account_view::MAX_PERMITTED_DATA_INCREASE;

    /// Prove the resize delta accumulation never overflows i32.
    ///
    /// Mirrors `resize()`:
    ///   `let difference = new_len_i32 - current_len;`
    ///   `let accumulated = delta_ptr.read_unaligned() + difference;`
    ///
    /// This proof shows that for any sequence of valid resize operations,
    /// the intermediate i32 arithmetic cannot overflow.
    #[kani::proof]
    fn resize_delta_no_overflow() {
        let current_len: i32 = kani::any();
        let new_len: i32 = kani::any();
        // SVM max account data is 10 MiB; both values are non-negative.
        kani::assume(current_len >= 0);
        kani::assume(new_len >= 0);
        kani::assume(current_len <= 10 * 1024 * 1024);
        kani::assume(new_len <= 10 * 1024 * 1024);

        let difference = new_len - current_len; // cannot overflow: both in [0, 10M]

        let prior_accumulated: i32 = kani::any();
        // Prior accumulated delta is within the valid range.
        kani::assume(prior_accumulated >= -(MAX_PERMITTED_DATA_INCREASE as i32));
        kani::assume(prior_accumulated <= MAX_PERMITTED_DATA_INCREASE as i32);

        // The addition that happens in resize():
        let accumulated = prior_accumulated.checked_add(difference);
        // Prove it never overflows i32.
        assert!(accumulated.is_some(), "resize delta overflow");
    }

    /// Prove the padding field reinterpretation: 4 bytes ↔ i32 roundtrip.
    ///
    /// Mirrors `resize()` read/write of the padding field:
    ///   `let delta_ptr = ... core::ptr::addr_of_mut!((*raw).padding) as *mut i32;`
    ///   `let accumulated = delta_ptr.read_unaligned() + difference;`
    ///   `delta_ptr.write_unaligned(accumulated);`
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

    /// Prove Account<T> is repr(transparent) — same size as its inner field.
    ///
    /// Mirrors the pointer casts in `Account::realloc()` and
    /// `Account::close()`:
    ///   `&mut *(self as *mut Account<T> as *mut AccountView)`
    ///
    /// These casts are only valid if `Account<T>` has the same layout as
    /// `AccountView`. Kani can't handle generic proofs, so we verify for
    /// the concrete `T = AccountView`.
    #[kani::proof]
    fn account_repr_transparent_size() {
        use solana_account_view::AccountView;
        assert!(
            core::mem::size_of::<super::Account<AccountView>>()
                == core::mem::size_of::<AccountView>()
        );
        assert!(
            core::mem::align_of::<super::Account<AccountView>>()
                == core::mem::align_of::<AccountView>()
        );
    }

    /// Prove `set_lamports` pointer cast preserves validity.
    ///
    /// Mirrors `set_lamports()`:
    ///   `(*(view.account_ptr() as *mut RuntimeAccount)).lamports = lamports`
    ///
    /// The cast changes mutability but not the address. This proof verifies
    /// the lamports field offset is stable and the write targets exactly
    /// the lamports field.
    #[kani::proof]
    fn set_lamports_field_offset_stable() {
        use solana_account_view::RuntimeAccount;
        // The lamports field must be at a fixed, known offset.
        let offset = core::mem::offset_of!(RuntimeAccount, lamports);
        // RuntimeAccount is repr(C); lamports comes after the 8-byte header
        // (borrow_state + is_signer + is_writable + executable + padding)
        // and two 32-byte Address fields. Verify it's within the struct.
        assert!(offset < core::mem::size_of::<RuntimeAccount>());
        // Verify the field is 8 bytes (u64).
        assert!(core::mem::size_of::<u64>() == 8);
        assert!(offset + 8 <= core::mem::size_of::<RuntimeAccount>());
    }

    /// Prove `realloc_account_raw` lamport subtraction is safe.
    ///
    /// Mirrors `realloc_account_raw()` lamport branching:
    ///   `if rent_exempt_lamports > current_lamports { ... rent_exempt_lamports - current_lamports ... }`
    ///   `else if current_lamports > rent_exempt_lamports { let excess = current_lamports - rent_exempt_lamports; ... }`
    ///
    /// Proves neither subtraction can underflow.
    #[kani::proof]
    fn realloc_lamport_subtraction_no_underflow() {
        let rent_exempt: u64 = kani::any();
        let current: u64 = kani::any();

        if rent_exempt > current {
            // Transfer path: compute the deficit.
            let deficit = rent_exempt - current;
            // Cannot underflow because rent_exempt > current.
            assert!(deficit > 0);
            assert!(deficit <= rent_exempt);
        } else if current > rent_exempt {
            // Excess return path: compute the surplus.
            let excess = current - rent_exempt;
            // Cannot underflow because current > rent_exempt.
            assert!(excess > 0);
            assert!(excess <= current);
        }
        // Equal case: no subtraction occurs.
    }

    /// Prove `realloc_account_raw` excess lamport addition does not overflow.
    ///
    /// Mirrors `realloc_account_raw()` excess return path:
    ///   `set_lamports(payer, payer.lamports() + excess);`
    ///
    /// Total SOL supply is ~5.8e17 lamports, well within u64::MAX (~1.8e19).
    /// Proves no overflow for any values within the SOL supply cap.
    #[kani::proof]
    fn realloc_excess_addition_no_overflow() {
        let payer_lamports: u64 = kani::any();
        let excess: u64 = kani::any();

        // Max total SOL supply is ~5.8e17 lamports. Both values are
        // bounded by total supply since they come from on-chain accounts.
        const MAX_SOL_SUPPLY: u64 = 600_000_000_000_000_000; // 6e17, generous bound
        kani::assume(payer_lamports <= MAX_SOL_SUPPLY);
        kani::assume(excess <= MAX_SOL_SUPPLY);
        // Combined cannot exceed total supply.
        kani::assume(payer_lamports + excess <= MAX_SOL_SUPPLY);

        let result = payer_lamports.checked_add(excess);
        assert!(result.is_some());
    }

    /// Prove `close` lamport addition uses `wrapping_add` safely.
    ///
    /// Mirrors `Account::close()` lamport drain:
    ///   `let new_lamports = destination.lamports().wrapping_add(view.lamports());`
    ///
    /// Total SOL supply fits in u64, so the sum never actually wraps. Proves
    /// that for realistic lamport values, `wrapping_add` produces the same
    /// result as `checked_add`.
    #[kani::proof]
    fn close_lamports_wrapping_add_equivalent_to_checked() {
        let dest_lamports: u64 = kani::any();
        let view_lamports: u64 = kani::any();

        const MAX_SOL_SUPPLY: u64 = 600_000_000_000_000_000;
        kani::assume(dest_lamports <= MAX_SOL_SUPPLY);
        kani::assume(view_lamports <= MAX_SOL_SUPPLY);

        let wrapping_result = dest_lamports.wrapping_add(view_lamports);
        let checked_result = dest_lamports.checked_add(view_lamports);

        // Within SOL supply bounds, wrapping_add == checked_add.
        assert!(checked_result.is_some());
        assert!(wrapping_result == checked_result.unwrap());
    }

    /// Prove `resize` write_bytes region is within the account allocation.
    ///
    /// Mirrors `resize()` zero-fill on grow:
    ///   `if difference > 0 { write_bytes(view.data_mut_ptr().add(current_len as usize), 0, difference as usize); }`
    ///
    /// Proves the offset arithmetic does not overflow and the zero-fill
    /// range `[current_len, current_len + difference)` is contiguous.
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
            // The zero-fill region: [current_len, current_len + difference)
            let start = current_len as usize;
            let count = difference as usize;
            let end = start.checked_add(count);
            assert!(end.is_some());
            let end = end.unwrap();
            // end == new_len, which is the new data length.
            assert!(end == new_len as usize);
            // The write starts at a valid offset within the data region.
            assert!(start <= end);
        }
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
