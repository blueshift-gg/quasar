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
            #[inline(always)]
            pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
                $(<$name as $check>::check(view)?;)*
                Ok(unsafe { &*(view as *const AccountView as *const Self) })
            }

            #[inline(always)]
            #[allow(invalid_reference_casting)]
            pub fn from_account_view_mut(view: &AccountView) -> Result<&mut Self, ProgramError> {
                $(<$name as $check>::check(view)?;)*
                if !view.is_writable() {
                    return Err(ProgramError::Immutable);
                }
                Ok(unsafe { &mut *(view as *const AccountView as *mut Self) })
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
