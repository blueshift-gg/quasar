//! Core macros for account definitions and runtime assertions.
//!
//! - `define_account!` — generates a `#[repr(transparent)]` account wrapper
//!   with check trait implementations and unchecked constructors for optimized
//!   parsing.
//! - `require!`, `require_eq!`, `require_keys_eq!` — constraint assertion
//!   macros that return early with a typed error on failure.
//! - `emit!` — emits an event via `sol_log_data` (~100 CU).

#[macro_export]
macro_rules! define_account {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident => [$($check:path),* $(,)?]
    ) => {
        $(#[$meta])*
        #[repr(transparent)]
        $vis struct $name {
            view: AccountView,
        }

        $(impl $check for $name {})*

        impl AsAccountView for $name {
            #[inline(always)]
            fn to_account_view(&self) -> &AccountView {
                &self.view
            }
        }

        impl $name {
            /// # Safety
            /// Caller must ensure all check traits have been validated.
            #[inline(always)]
            pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
                &*(view as *const AccountView as *const Self)
            }

            /// # Safety
            /// Caller must ensure all check traits and writability.
            #[inline(always)]
            pub unsafe fn from_account_view_unchecked_mut(view: &mut AccountView) -> &mut Self {
                &mut *(view as *mut AccountView as *mut Self)
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
