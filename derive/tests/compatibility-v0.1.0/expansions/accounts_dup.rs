#[derive(Copy, Clone)]
pub struct HeaderDupReadonlyBumps;
impl ::quasar_lang::traits::AccountBumps for HeaderDupReadonly {
    type Bumps = HeaderDupReadonlyBumps;
}
impl<'input> ::quasar_lang::traits::ParseAccounts<'input> for HeaderDupReadonly {
    type Bumps = HeaderDupReadonlyBumps;
    const HAS_EPILOGUE: bool = false;
}
unsafe impl<'input> ::quasar_lang::traits::ParseAccountsUnchecked<'input>
for HeaderDupReadonly {
    #[inline(always)]
    unsafe fn parse_with_instruction_data_unchecked(
        accounts: &'input mut [::quasar_lang::__internal::AccountView],
        __ix_data: &[u8],
        __program_id: &::quasar_lang::prelude::Address,
    ) -> Result<
        (Self, Self::Bumps),
        ::quasar_lang::__solana_program_error::ProgramError,
    > {
        let [source, destination] = accounts else {
            unsafe { core::hint::unreachable_unchecked() }
        };
        let source = <Signer as ::quasar_lang::account_load::AccountLoad>::load(source)?;
        let destination = <UncheckedAccount as ::quasar_lang::account_load::AccountLoad>::load_checked(
            destination,
        )?;
        Ok((Self { source, destination }, HeaderDupReadonlyBumps))
    }
}
impl ::quasar_lang::traits::AccountCount for HeaderDupReadonly {
    const COUNT: usize = 2usize;
    const NEEDS_EVENT_CPI: bool = false;
}
unsafe impl ::quasar_lang::traits::ParseAccountsRaw for HeaderDupReadonly {
    #[inline(always)]
    unsafe fn parse_accounts_raw(
        mut input: *mut u8,
        base: *mut ::quasar_lang::__internal::AccountView,
        __offset: usize,
        __program_id: &::quasar_lang::prelude::Address,
    ) -> Result<*mut u8, ::quasar_lang::__solana_program_error::ProgramError> {
        input = unsafe {
            ::quasar_lang::__internal::parse_account::<
                Signer,
                false,
            >(input, base, __offset + 0usize)?
        };
        ::quasar_lang::debug_log!("account source @0: validation passed");
        input = unsafe {
            ::quasar_lang::__internal::parse_account_dup::<
                UncheckedAccount,
                false,
            >(
                input,
                base,
                __offset + 1usize,
                __program_id,
                ::quasar_lang::__internal::ParseFlags {
                    is_optional: false,
                    is_ref_mut: false,
                    allow_dup: true,
                },
            )?
        };
        ::quasar_lang::debug_log!("account destination @1: parsed (dup-aware)");
        Ok(input)
    }
}
impl<'input> ::quasar_lang::remaining::RemainingItem<'input> for HeaderDupReadonly {
    const COUNT: usize = <Self as ::quasar_lang::traits::AccountCount>::COUNT;
    #[inline(always)]
    unsafe fn parse_remaining_chunk(
        accounts: &'input mut [::quasar_lang::__internal::AccountView],
        program_id: Option<&::quasar_lang::prelude::Address>,
        data: &[u8],
    ) -> Result<Self, ::quasar_lang::__solana_program_error::ProgramError> {
        unsafe {
            ::quasar_lang::remaining::parse_group_chunk::<
                Self,
            >(accounts, program_id, data)
        }
    }
}
#[doc(hidden)]
#[allow(unexpected_cfgs)]
#[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
#[macro_export]
macro_rules! __header_dup_readonly_instruction {
    ($struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* }) => {
        pub struct $struct_name { pub source : ::quasar_lang::prelude::Address, pub
        destination : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* }
        impl From < $struct_name > for ::quasar_lang::client::Instruction {
        #[allow(unused_variables)] fn from(ix : $struct_name) ->
        ::quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new_readonly(ix.source, true),
        ::quasar_lang::client::AccountMeta::new_readonly(ix.destination, false),]; let
        data = { let mut _data = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& <
        $arg_ty as ::quasar_lang::client::SerializeArg > ::serialize_arg(& ix.
        $arg_name));)* _data }; ::quasar_lang::client::Instruction { program_id :
        $crate::ID, accounts, data, } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        compact
    ) => {
        pub struct $struct_name { pub source : ::quasar_lang::prelude::Address, pub
        destination : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* }
        impl From < $struct_name > for ::quasar_lang::client::Instruction {
        #[allow(unused_variables)] fn from(ix : $struct_name) ->
        ::quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new_readonly(ix.source, true),
        ::quasar_lang::client::AccountMeta::new_readonly(ix.destination, false),]; let
        data = { let mut _data = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& <
        $arg_ty as ::quasar_lang::client::CompactSerializeArg > ::compact_header(& ix.
        $arg_name));)* $(_data.extend_from_slice(& < $arg_ty as
        ::quasar_lang::client::CompactSerializeArg > ::compact_tail(& ix. $arg_name));)*
        _data }; ::quasar_lang::client::Instruction { program_id : $crate::ID, accounts,
        data, } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        remaining
    ) => {
        pub struct $struct_name { pub source : ::quasar_lang::prelude::Address, pub
        destination : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* pub
        remaining_accounts : ::alloc::vec::Vec < ::quasar_lang::client::AccountMeta >, }
        impl From < $struct_name > for ::quasar_lang::client::Instruction {
        #[allow(unused_variables)] fn from(ix : $struct_name) ->
        ::quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new_readonly(ix.source, true),
        ::quasar_lang::client::AccountMeta::new_readonly(ix.destination, false),];
        accounts.extend(ix.remaining_accounts); let data = { let mut _data =
        ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
        ::quasar_lang::client::SerializeArg > ::serialize_arg(& ix. $arg_name));)* _data
        }; ::quasar_lang::client::Instruction { program_id : $crate::ID, accounts, data,
        } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        compact, remaining
    ) => {
        pub struct $struct_name { pub source : ::quasar_lang::prelude::Address, pub
        destination : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* pub
        remaining_accounts : ::alloc::vec::Vec < ::quasar_lang::client::AccountMeta >, }
        impl From < $struct_name > for ::quasar_lang::client::Instruction {
        #[allow(unused_variables)] fn from(ix : $struct_name) ->
        ::quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new_readonly(ix.source, true),
        ::quasar_lang::client::AccountMeta::new_readonly(ix.destination, false),];
        accounts.extend(ix.remaining_accounts); let data = { let mut _data =
        ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
        ::quasar_lang::client::CompactSerializeArg > ::compact_header(& ix.
        $arg_name));)* $(_data.extend_from_slice(& < $arg_ty as
        ::quasar_lang::client::CompactSerializeArg > ::compact_tail(& ix. $arg_name));)*
        _data }; ::quasar_lang::client::Instruction { program_id : $crate::ID, accounts,
        data, } } }
    };
}
#[cfg(feature = "idl-build")]
::quasar_lang::__private_inventory::submit! {
    ::quasar_lang::idl_build::AccountsMetaFragment(|| {
    (::quasar_lang::idl_build::s("HeaderDupReadonly"),
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::AccountsMetaEntry::Node(::quasar_lang::idl_build::__reexport::IdlAccountNode
    { name : ::quasar_lang::idl_build::s("source"), optional : false, writable :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), signer :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), resolver :
    ::quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    ::quasar_lang::idl_build::Vec::new(), }),
    ::quasar_lang::idl_build::AccountsMetaEntry::Node(::quasar_lang::idl_build::__reexport::IdlAccountNode
    { name : ::quasar_lang::idl_build::s("destination"), optional : false, writable :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), signer :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), resolver :
    ::quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::s("CHECK: test-only unchecked account used to validate duplicate readonly aliases.")],
    })],) })
}
#[cfg(feature = "idl-build")]
::quasar_lang::__private_inventory::submit! {
    ::quasar_lang::idl_build::AccountsValidationFragment(|| {
    (::quasar_lang::idl_build::s("HeaderDupReadonly"),
    ::quasar_lang::idl_build::__reexport::IdlAccountsValidation { rent :
    ::quasar_lang::idl_build::s("NotNeeded"), accounts :
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::__reexport::IdlAccountValidation
    { name : ::quasar_lang::idl_build::s("source"), account_type :
    ::quasar_lang::idl_build::s("Signer"), wrapper :
    ::quasar_lang::idl_build::s("Signer"), writable : false, signer : true, optional :
    false, allow_duplicate : false, load :
    ::quasar_lang::idl_build::s("Fixed(validates=[])"), pre_load :
    ::quasar_lang::idl_build::vec![], post_load : ::quasar_lang::idl_build::vec![],
    epilogue : ::quasar_lang::idl_build::vec![], },
    ::quasar_lang::idl_build::__reexport::IdlAccountValidation { name :
    ::quasar_lang::idl_build::s("destination"), account_type :
    ::quasar_lang::idl_build::s("UncheckedAccount"), wrapper :
    ::quasar_lang::idl_build::s("UncheckedAccount"), writable : false, signer : false,
    optional : false, allow_duplicate : true, load :
    ::quasar_lang::idl_build::s("Fixed(validates=[])"), pre_load :
    ::quasar_lang::idl_build::vec![], post_load : ::quasar_lang::idl_build::vec![],
    epilogue : ::quasar_lang::idl_build::vec![], }], },) })
}
