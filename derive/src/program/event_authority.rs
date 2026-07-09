//! The program marker type and its `EventAuthority` PDA companion.
//!
//! `#[program] mod foo` emits a `Foo` program type (used as `Program<Foo>` in
//! account structs) plus an `EventAuthority` newtype whose PDA is derived from
//! the fixed `b"__event_authority"` seed and exposes the `ADDRESS`/`BUMP`
//! consts. The self-CPI event path is the typed `quasar_lang::event::EventCpi`
//! trait, whose impl `#[derive(Accounts)]` emits from a struct's
//! `event_authority` + program fields (`emit_cpi!` calls it).

use {proc_macro2::TokenStream as TokenStream2, quote::quote, syn::Ident};

/// Emit the program marker type and its `EventAuthority` PDA companion.
pub(super) fn emit_program_type(program_type_name: &Ident) -> TokenStream2 {
    quote! {
        quasar_lang::define_account!(pub struct #program_type_name => [quasar_lang::checks::Executable, quasar_lang::checks::Address]);

        impl quasar_lang::traits::Id for #program_type_name {
            const ID: Address = crate::ID;
        }

        #[repr(transparent)]
        pub struct EventAuthority {
            view: AccountView,
        }

        impl AsAccountView for EventAuthority {
            #[inline(always)]
            fn to_account_view(&self) -> &AccountView {
                &self.view
            }
        }

        impl EventAuthority {
            const __PDA: (Address, u8) = quasar_lang::pda::find_program_address_const(
                &[b"__event_authority"],
                &crate::ID,
            );
            pub const ADDRESS: Address = Self::__PDA.0;
            pub const BUMP: u8 = Self::__PDA.1;

            #[inline(always)]
            pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
                if !quasar_lang::keys_eq(view.address(), &Self::ADDRESS) {
                    return Err(ProgramError::InvalidSeeds);
                }
                Ok(unsafe {
                    // SAFETY: `EventAuthority` is repr(transparent) over
                    // AccountView, and the PDA address was validated above.
                    &*(view as *const AccountView as *const Self)
                })
            }

            /// Construct without validation.
            ///
            /// # Safety
            /// Caller must ensure account address matches the expected PDA.
            #[inline(always)]
            pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
                unsafe {
                    // SAFETY: caller guarantees the account is the event
                    // authority PDA.
                    &*(view as *const AccountView as *const Self)
                }
            }
        }

        // SAFETY: `EventAuthority` is `#[repr(transparent)]` over `AccountView`.
        unsafe impl quasar_lang::traits::StaticView for EventAuthority {}

        impl quasar_lang::account_load::AccountLoad for EventAuthority {

            #[inline(always)]
            fn check(view: &AccountView) -> Result<(), ProgramError> {
                if !quasar_lang::keys_eq(view.address(), &Self::ADDRESS) {
                    return Err(ProgramError::InvalidSeeds);
                }
                Ok(())
            }
        }
    }
}
