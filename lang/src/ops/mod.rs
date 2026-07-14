//! Op-dispatch for account lifecycle operations.
//!
//! The derive emits direct capability trait calls for validation, init
//! contribution, and exit actions. Structural ops (init, realloc, PDA
//! verification) use their own inherent methods.
//!
//! `OpCtx` carries instruction-scoped state. Rent is resolved lazily so
//! idempotent/no-op init paths do not pay for sysvar access.

pub mod close;
pub mod init;
pub mod realloc;

use core::{
    cell::{Cell, UnsafeCell},
    mem::MaybeUninit,
};

#[doc(hidden)]
pub trait RentAccess {
    fn get(&self) -> Result<&crate::sysvars::rent::Rent, solana_program_error::ProgramError>;
}

impl RentAccess for crate::sysvars::rent::Rent {
    #[inline(always)]
    fn get(&self) -> Result<&crate::sysvars::rent::Rent, solana_program_error::ProgramError> {
        Ok(self)
    }
}

impl RentAccess for &crate::sysvars::rent::Rent {
    #[inline(always)]
    fn get(&self) -> Result<&crate::sysvars::rent::Rent, solana_program_error::ProgramError> {
        Ok(*self)
    }
}

impl RentAccess for crate::accounts::Sysvar<crate::sysvars::rent::Rent> {
    #[inline(always)]
    fn get(&self) -> Result<&crate::sysvars::rent::Rent, solana_program_error::ProgramError> {
        Ok(self.get())
    }
}

/// Lazily resolves Rent for lifecycle operations.
///
/// Used only when no `Sysvar<Rent>` account is present. The syscall is
/// deferred until the first operation that actually needs a rent value.
#[doc(hidden)]
pub struct RentResolver {
    fetched: Cell<bool>,
    cached: UnsafeCell<MaybeUninit<crate::sysvars::rent::Rent>>,
}

impl RentResolver {
    #[inline(always)]
    pub fn fetch_once() -> Self {
        Self {
            fetched: Cell::new(false),
            cached: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

impl RentAccess for RentResolver {
    #[inline(always)]
    fn get(&self) -> Result<&crate::sysvars::rent::Rent, solana_program_error::ProgramError> {
        if !self.fetched.get() {
            let rent = <crate::sysvars::rent::Rent as crate::sysvars::Sysvar>::get()?;
            // SAFETY: `fetched == false` means the cache is uninitialized, and
            // `&self` has interior mutability through `UnsafeCell`.
            unsafe { (*self.cached.get()).write(rent) };
            self.fetched.set(true);
        }

        // SAFETY: The cache is initialized before this point and remains live
        // until `Drop`.
        Ok(unsafe { &*(*self.cached.get()).as_ptr() })
    }
}

impl Drop for RentResolver {
    #[inline(always)]
    fn drop(&mut self) {
        if self.fetched.get() {
            // SAFETY: `fetched == true` means the cache was initialized.
            unsafe { (*self.cached.get()).assume_init_drop() };
        }
    }
}

/// Lifecycle operation context.
#[doc(hidden)]
pub struct OpCtx<'a, R> {
    pub program_id: &'a solana_address::Address,
    pub rent: R,
}

impl<'a, R> OpCtx<'a, R> {
    #[inline(always)]
    pub fn new(program_id: &'a solana_address::Address, rent: R) -> Self {
        Self { program_id, rent }
    }
}

/// Marker trait for account types that support realloc.
///
/// The `realloc::Op` requires `F: SupportsRealloc` to ensure only
/// realloc-capable accounts are used with `realloc(...)`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be reallocated",
    label = "`realloc` requires `Account<T>` or `InterfaceAccount<T>`",
    note = "only program-owned data accounts support `realloc = ...`"
)]
pub trait SupportsRealloc {}

#[cfg(test)]
mod tests {
    use super::{RentAccess, RentResolver};

    #[test]
    fn borrowed_rent_access_returns_same_rent() {
        // SAFETY: This test only compares pointer identity; it never observes
        // the zeroed rent fields.
        let rent: crate::sysvars::rent::Rent = unsafe { core::mem::zeroed() };
        let borrowed = &rent;
        let resolved = borrowed.get().unwrap();
        assert!(core::ptr::eq(resolved, &rent));
    }

    #[test]
    fn rent_resolver_starts_unfetched() {
        let resolver = RentResolver::fetch_once();
        assert!(!resolver.fetched.get());
    }
}
