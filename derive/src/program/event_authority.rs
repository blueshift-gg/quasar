//! The program marker type and its `EventAuthority` PDA companion.
//!
//! `#[program] mod foo` emits a `FooProgram` marker (used as
//! `Program<FooProgram>` in account structs) plus an `EventAuthority` newtype whose PDA is derived from
//! the fixed `b"__event_authority"` seed and exposes the `ADDRESS`/`BUMP`
//! consts. The self-CPI event path is the typed `quasar_lang::event::EventCpi`
//! trait, whose impl `#[derive(Accounts)]` emits from a struct's
//! `event_authority` + program fields (`emit_cpi!` calls it).

use {proc_macro2::TokenStream as TokenStream2, quote::quote, syn::Ident};

/// Emit the program marker type and its `EventAuthority` PDA companion.
pub(super) fn emit_program_type(program_type_name: &Ident) -> TokenStream2 {
    let krate = crate::krate::lang_path();
    quote! {
        #krate::define_account!(pub struct #program_type_name => [#krate::checks::Executable, #krate::checks::Address]);

        impl #krate::traits::Id for #program_type_name {
            const ID: #krate::prelude::Address = crate::ID;
        }

        #[repr(transparent)]
        pub struct EventAuthority {
            view: #krate::__internal::AccountView,
        }

        impl #krate::traits::AsAccountView for EventAuthority {
            #[inline(always)]
            fn to_account_view(&self) -> &#krate::__internal::AccountView {
                &self.view
            }
        }

        impl EventAuthority {
            const __PDA: (#krate::prelude::Address, u8) = #krate::pda::find_program_address_const(
                &[b"__event_authority"],
                &crate::ID,
            );
            pub const ADDRESS: #krate::prelude::Address = Self::__PDA.0;
            pub const BUMP: u8 = Self::__PDA.1;

            #[inline(always)]
            pub fn from_account_view(view: &#krate::__internal::AccountView) -> Result<&Self, #krate::__solana_program_error::ProgramError> {
                if !#krate::keys_eq(view.address(), &Self::ADDRESS) {
                    return Err(#krate::__solana_program_error::ProgramError::InvalidSeeds);
                }
                Ok(unsafe {
                    // SAFETY: `EventAuthority` is repr(transparent) over
                    // AccountView, and the PDA address was validated above.
                    &*(view as *const #krate::__internal::AccountView as *const Self)
                })
            }

            /// Construct without validation.
            ///
            /// # Safety
            /// Caller must ensure account address matches the expected PDA.
            #[inline(always)]
            pub unsafe fn from_account_view_unchecked(view: &#krate::__internal::AccountView) -> &Self {
                unsafe {
                    // SAFETY: caller guarantees the account is the event
                    // authority PDA.
                    &*(view as *const #krate::__internal::AccountView as *const Self)
                }
            }
        }

        // SAFETY: `EventAuthority` is `#[repr(transparent)]` over `AccountView`.
        unsafe impl #krate::traits::StaticView for EventAuthority {}

        impl #krate::account_load::AccountLoad for EventAuthority {

            #[inline(always)]
            fn check(view: &#krate::__internal::AccountView) -> Result<(), #krate::__solana_program_error::ProgramError> {
                if !#krate::keys_eq(view.address(), &Self::ADDRESS) {
                    return Err(#krate::__solana_program_error::ProgramError::InvalidSeeds);
                }
                Ok(())
            }
        }
    }
}
