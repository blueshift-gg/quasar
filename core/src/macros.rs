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
            /// Unchecked construction for optimized parsing where
            /// signer/writable/executable/no-dup flags have been pre-validated via
            /// u32 header comparison during entrypoint deserialization.
            ///
            /// # Safety
            ///
            /// Caller must guarantee that all check trait requirements (`$check`)
            /// have been validated before calling this function.
            #[inline(always)]
            pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
                &*(view as *const AccountView as *const Self)
            }

            /// Unchecked mutable construction for optimized parsing.
            ///
            /// # Safety
            ///
            /// Caller must guarantee:
            /// 1. All check trait requirements have been validated
            /// 2. `view.is_writable()` is true (validated via header check)
            ///
            /// Additionally, this function uses `invalid_reference_casting` to convert
            /// `&AccountView` to `&mut Self`, which is safe because `Self` is
            /// `#[repr(transparent)]` over `AccountView` and uses interior mutability.
            #[inline(always)]
            #[allow(invalid_reference_casting, clippy::mut_from_ref)]
            pub unsafe fn from_account_view_unchecked_mut(view: &AccountView) -> &mut Self {
                &mut *(view as *const AccountView as *mut Self)
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
