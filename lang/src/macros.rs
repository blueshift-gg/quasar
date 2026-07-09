//! Core macros for account definitions and runtime assertions.
//!
//! - `define_account!`: generates a `#[repr(transparent)]` account wrapper with
//!   check trait implementations and unchecked constructors for optimized
//!   parsing.
//! - `require!`, `require_eq!`, `require_keys_eq!`: constraint assertion macros
//!   that return early with a typed error on failure.
//! - `emit!`: emits an event via `sol_log_data` (~100 CU).

#[macro_export]
macro_rules! define_account {
    // Schema form: `pub struct Token => [checks::DataLen, checks::ZeroPod]: TokenData`
    //
    // Generates everything from the base form (including `StaticView`) plus:
    // - AccountLayout (DATA_OFFSET = 0, Schema = $schema, Target = <$schema as ZeroPodFixed>::Zc)
    // - Deref/DerefMut at DATA_OFFSET (always 0 for define_account!)
    // - ZeroCopyDeref
    // - AccountLoad::check() composing listed checks
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident => [$($check:path),* $(,)?] : $schema:ty
    ) => {
        $crate::define_account!($(#[$meta])* $vis struct $name => [$($check),*]);

        impl $crate::account_layout::AccountLayout for $name {
            type Schema = $schema;
            const DATA_OFFSET: usize = 0;
        }

        impl core::ops::Deref for $name {
            type Target = <$schema as $crate::__zeropod::ZeroPodFixed>::Zc;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                // SAFETY: Checks validated data_len >= SIZE.
                // Zc companion is #[repr(C)] with alignment 1.
                unsafe { &*(self.view.data_ptr() as *const Self::Target) }
            }
        }

        impl core::ops::DerefMut for $name {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut Self::Target {
                // SAFETY: Same as Deref; length validated, alignment 1.
                unsafe { &mut *(self.view.data_mut_ptr() as *mut Self::Target) }
            }
        }

        impl $crate::traits::ZeroCopyDeref for $name {
            type Target = <$schema as $crate::__zeropod::ZeroPodFixed>::Zc;

            #[inline(always)]
            unsafe fn deref_from(view: &$crate::__internal::AccountView) -> &Self::Target {
                // SAFETY: Caller validated the account data layout and length.
                unsafe { &*(view.data_ptr() as *const Self::Target) }
            }

            #[inline(always)]
            unsafe fn deref_from_mut(view: &mut $crate::__internal::AccountView) -> &mut Self::Target {
                // SAFETY: Same as `deref_from`; caller also guarantees mutable
                // access to the account data.
                unsafe { &mut *(view.data_mut_ptr() as *mut Self::Target) }
            }
        }

        impl $crate::account_load::AccountLoad for $name {
            #[inline(always)]
            fn check(view: &$crate::__internal::AccountView) -> Result<(), $crate::__solana_program_error::ProgramError> {
                $(<$name as $check>::check(view)?;)*
                Ok(())
            }

            #[inline(always)]
            fn check_checked(view: &$crate::__internal::AccountView) -> Result<(), $crate::__solana_program_error::ProgramError> {
                let __data = view.try_borrow()?;
                let __size = core::mem::size_of::<<$schema as $crate::__zeropod::ZeroPodFixed>::Zc>();
                if __data.len() < __size {
                    return Err($crate::__solana_program_error::ProgramError::AccountDataTooSmall);
                }
                <$schema as $crate::__zeropod::ZeroPodFixed>::validate(&__data[..__size])
                    .map_err(|_| $crate::__solana_program_error::ProgramError::InvalidAccountData)?;
                Ok(())
            }

            #[inline(always)]
            fn check_intrinsic(_view: &$crate::__internal::AccountView) -> Result<(), $crate::__solana_program_error::ProgramError> {
                Ok(())
            }
        }

        impl<'__quasar_remaining> $crate::remaining::RemainingItem<'__quasar_remaining> for $name {
            const COUNT: usize = 1;

            #[inline(always)]
            unsafe fn parse_remaining_one(
                account: $crate::__internal::AccountView,
                _program_id: Option<&$crate::prelude::Address>,
                _data: &[u8],
            ) -> Result<Self, $crate::__solana_program_error::ProgramError> {
                $crate::remaining::parse_remaining_view::<Self>(&account)
            }

            #[inline(always)]
            unsafe fn parse_remaining_chunk(
                accounts: &'__quasar_remaining mut [$crate::__internal::AccountView],
                _program_id: Option<&$crate::prelude::Address>,
                _data: &[u8],
            ) -> Result<Self, $crate::__solana_program_error::ProgramError> {
                $crate::remaining::parse_remaining_account::<Self>(accounts)
            }
        }

    };

    // Base form: `pub struct Signer => [checks::Signer]`
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident => [$($check:path),* $(,)?]
    ) => {
        $(#[$meta])*
        #[repr(transparent)]
        $vis struct $name {
            view: $crate::__internal::AccountView,
        }

        // SAFETY: The wrapper is `#[repr(transparent)]` over `AccountView`.
        unsafe impl $crate::traits::StaticView for $name {}

        $(impl $check for $name {})*

        impl $crate::traits::AsAccountView for $name {
            #[inline(always)]
            fn to_account_view(&self) -> &$crate::__internal::AccountView {
                &self.view
            }
        }

        impl $name {
            /// # Safety
            /// Caller must ensure all check traits have been validated.
            #[inline(always)]
            pub unsafe fn from_account_view_unchecked(view: &$crate::__internal::AccountView) -> &Self {
                // SAFETY: Generated account wrappers are `repr(transparent)`
                // over `AccountView`; caller upheld the check invariants.
                unsafe { &*(view as *const $crate::__internal::AccountView as *const Self) }
            }

            /// # Safety
            /// Caller must ensure all check traits and writability.
            #[inline(always)]
            pub unsafe fn from_account_view_unchecked_mut(view: &mut $crate::__internal::AccountView) -> &mut Self {
                // SAFETY: Same layout argument as the immutable cast; caller
                // also guarantees writable access.
                unsafe { &mut *(view as *mut $crate::__internal::AccountView as *mut Self) }
            }
        }

    };
}

#[macro_export]
macro_rules! require {
    ($condition:expr, $error:expr) => {
        if !($condition) {
            return Err($error.into());
        }
    };
}

#[macro_export]
macro_rules! require_eq {
    ($left:expr, $right:expr, $error:expr) => {
        if $left != $right {
            return Err($error.into());
        }
    };
}

#[macro_export]
macro_rules! require_keys_eq {
    ($left:expr, $right:expr, $error:expr) => {
        if !$crate::keys_eq(&$left, &$right) {
            return Err($error.into());
        }
    };
}

#[macro_export]
macro_rules! emit {
    ($event:expr) => {
        $event.emit_log()
    };
}
