#[derive(Copy, Clone)]
pub struct CloseAccountsBumps;
impl ::quasar_lang::traits::AccountBumps for CloseAccounts {
    type Bumps = CloseAccountsBumps;
}
impl<'input> ::quasar_lang::traits::ParseAccounts<'input> for CloseAccounts {
    type Bumps = CloseAccountsBumps;
    const HAS_EPILOGUE: bool = true;
    #[inline(always)]
    fn epilogue(
        &mut self,
    ) -> Result<(), ::quasar_lang::__solana_program_error::ProgramError> {
        {
            let __view = unsafe {
                <Account<
                    OldData,
                > as ::quasar_lang::account_load::AccountLoad>::to_account_view_mut(
                    &mut self.old_data,
                )
            };
            ::quasar_lang::ops::close::Op {
                disc_len: <OldData as ::quasar_lang::traits::Discriminator>::DISCRIMINATOR
                    .len(),
            }
                .apply(__view, self.authority.to_account_view())?;
        }
        Ok(())
    }
}
unsafe impl<'input> ::quasar_lang::traits::ParseAccountsUnchecked<'input>
for CloseAccounts {
    #[inline(always)]
    unsafe fn parse_with_instruction_data_unchecked(
        accounts: &'input mut [::quasar_lang::__internal::AccountView],
        __ix_data: &[u8],
        __program_id: &::quasar_lang::prelude::Address,
    ) -> Result<
        (Self, Self::Bumps),
        ::quasar_lang::__solana_program_error::ProgramError,
    > {
        let [authority, old_data] = accounts else {
            unsafe { core::hint::unreachable_unchecked() }
        };
        let mut authority = <Signer as ::quasar_lang::account_load::AccountLoad>::load_mut(
            authority,
        )?;
        let mut old_data = <Account<
            OldData,
        > as ::quasar_lang::account_load::AccountLoad>::load_mut(old_data)?;
        Ok((Self { authority, old_data }, CloseAccountsBumps))
    }
}
impl ::quasar_lang::traits::AccountCount for CloseAccounts {
    const COUNT: usize = 2usize;
    const NEEDS_EVENT_CPI: bool = false;
}
unsafe impl ::quasar_lang::traits::ParseAccountsRaw for CloseAccounts {
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
                true,
            >(input, base, __offset)?
        };
        ::quasar_lang::debug_log!("account authority @0: validation passed");
        input = unsafe {
            ::quasar_lang::__internal::parse_account::<
                Account<OldData>,
                true,
            >(input, base, __offset + 1usize)?
        };
        ::quasar_lang::debug_log!("account old_data @1: validation passed");
        Ok(input)
    }
}
impl<'input> ::quasar_lang::remaining::RemainingItem<'input> for CloseAccounts {
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
macro_rules! __close_accounts_instruction {
    (
        $struct_name:ident, $raw_name:ident, [$($disc:expr),*], { $($arg_name:ident :
        $arg_ty:ty),* }
    ) => {
        pub struct $struct_name { pub authority : ::quasar_lang::prelude::Address, pub
        old_data : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* } #[doc
        = r" Every account spelled out, including the ones the input builder"] #[doc =
        r" resolves for you. Build it from `$struct_name` and replace an"] #[doc =
        r" address the client would otherwise derive."] pub struct $raw_name { pub
        authority : ::quasar_lang::prelude::Address, pub old_data :
        ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* } impl From <
        $struct_name > for $raw_name { #[allow(unused_variables)] fn from(ix :
        $struct_name) -> $raw_name { $raw_name { authority : ix.authority, old_data : ix
        .old_data, $($arg_name : ix. $arg_name,)* } } } impl From < $struct_name > for
        ::quasar_lang::client::Instruction { #[inline] fn from(ix : $struct_name) ->
        ::quasar_lang::client::Instruction { < $raw_name as ::core::convert::Into <
        ::quasar_lang::client::Instruction >> ::into(< $struct_name as
        ::core::convert::Into < $raw_name >> ::into(ix),) } } impl From < $raw_name > for
        ::quasar_lang::client::Instruction { #[allow(unused_variables)] fn from(ix :
        $raw_name) -> ::quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.authority, true),
        ::quasar_lang::client::AccountMeta::new(ix.old_data, false),]; let data = { let
        mut _data = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
        ::quasar_lang::client::SerializeArg > ::serialize_arg(& ix. $arg_name));)* _data
        }; ::quasar_lang::client::Instruction { program_id : $crate::ID, accounts, data,
        } } }
    };
    (
        $struct_name:ident, $raw_name:ident, [$($disc:expr),*], { $($arg_name:ident :
        $arg_ty:ty),* }, compact
    ) => {
        pub struct $struct_name { pub authority : ::quasar_lang::prelude::Address, pub
        old_data : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* } #[doc
        = r" Every account spelled out, including the ones the input builder"] #[doc =
        r" resolves for you. Build it from `$struct_name` and replace an"] #[doc =
        r" address the client would otherwise derive."] pub struct $raw_name { pub
        authority : ::quasar_lang::prelude::Address, pub old_data :
        ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* } impl From <
        $struct_name > for $raw_name { #[allow(unused_variables)] fn from(ix :
        $struct_name) -> $raw_name { $raw_name { authority : ix.authority, old_data : ix
        .old_data, $($arg_name : ix. $arg_name,)* } } } impl From < $struct_name > for
        ::quasar_lang::client::Instruction { #[inline] fn from(ix : $struct_name) ->
        ::quasar_lang::client::Instruction { < $raw_name as ::core::convert::Into <
        ::quasar_lang::client::Instruction >> ::into(< $struct_name as
        ::core::convert::Into < $raw_name >> ::into(ix),) } } impl From < $raw_name > for
        ::quasar_lang::client::Instruction { #[allow(unused_variables)] fn from(ix :
        $raw_name) -> ::quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.authority, true),
        ::quasar_lang::client::AccountMeta::new(ix.old_data, false),]; let data = { let
        mut _data = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
        ::quasar_lang::client::CompactSerializeArg > ::compact_header(& ix.
        $arg_name));)* $(_data.extend_from_slice(& < $arg_ty as
        ::quasar_lang::client::CompactSerializeArg > ::compact_tail(& ix. $arg_name));)*
        _data }; ::quasar_lang::client::Instruction { program_id : $crate::ID, accounts,
        data, } } }
    };
    (
        $struct_name:ident, $raw_name:ident, [$($disc:expr),*], { $($arg_name:ident :
        $arg_ty:ty),* }, remaining
    ) => {
        pub struct $struct_name { pub authority : ::quasar_lang::prelude::Address, pub
        old_data : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* pub
        remaining_accounts : ::alloc::vec::Vec < ::quasar_lang::client::AccountMeta >, }
        #[doc = r" Every account spelled out, including the ones the input builder"]
        #[doc = r" resolves for you. Build it from `$struct_name` and replace an"] #[doc
        = r" address the client would otherwise derive."] pub struct $raw_name { pub
        authority : ::quasar_lang::prelude::Address, pub old_data :
        ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* pub
        remaining_accounts : ::alloc::vec::Vec < ::quasar_lang::client::AccountMeta >, }
        impl From < $struct_name > for $raw_name { #[allow(unused_variables)] fn from(ix
        : $struct_name) -> $raw_name { $raw_name { authority : ix.authority, old_data :
        ix.old_data, $($arg_name : ix. $arg_name,)* remaining_accounts : ix
        .remaining_accounts, } } } impl From < $struct_name > for
        ::quasar_lang::client::Instruction { #[inline] fn from(ix : $struct_name) ->
        ::quasar_lang::client::Instruction { < $raw_name as ::core::convert::Into <
        ::quasar_lang::client::Instruction >> ::into(< $struct_name as
        ::core::convert::Into < $raw_name >> ::into(ix),) } } impl From < $raw_name > for
        ::quasar_lang::client::Instruction { #[allow(unused_variables)] fn from(ix :
        $raw_name) -> ::quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.authority, true),
        ::quasar_lang::client::AccountMeta::new(ix.old_data, false),]; accounts.extend(ix
        .remaining_accounts); let data = { let mut _data = ::alloc::vec![$($disc),*];
        $(_data.extend_from_slice(& < $arg_ty as ::quasar_lang::client::SerializeArg >
        ::serialize_arg(& ix. $arg_name));)* _data }; ::quasar_lang::client::Instruction
        { program_id : $crate::ID, accounts, data, } } }
    };
    (
        $struct_name:ident, $raw_name:ident, [$($disc:expr),*], { $($arg_name:ident :
        $arg_ty:ty),* }, compact, remaining
    ) => {
        pub struct $struct_name { pub authority : ::quasar_lang::prelude::Address, pub
        old_data : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* pub
        remaining_accounts : ::alloc::vec::Vec < ::quasar_lang::client::AccountMeta >, }
        #[doc = r" Every account spelled out, including the ones the input builder"]
        #[doc = r" resolves for you. Build it from `$struct_name` and replace an"] #[doc
        = r" address the client would otherwise derive."] pub struct $raw_name { pub
        authority : ::quasar_lang::prelude::Address, pub old_data :
        ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* pub
        remaining_accounts : ::alloc::vec::Vec < ::quasar_lang::client::AccountMeta >, }
        impl From < $struct_name > for $raw_name { #[allow(unused_variables)] fn from(ix
        : $struct_name) -> $raw_name { $raw_name { authority : ix.authority, old_data :
        ix.old_data, $($arg_name : ix. $arg_name,)* remaining_accounts : ix
        .remaining_accounts, } } } impl From < $struct_name > for
        ::quasar_lang::client::Instruction { #[inline] fn from(ix : $struct_name) ->
        ::quasar_lang::client::Instruction { < $raw_name as ::core::convert::Into <
        ::quasar_lang::client::Instruction >> ::into(< $struct_name as
        ::core::convert::Into < $raw_name >> ::into(ix),) } } impl From < $raw_name > for
        ::quasar_lang::client::Instruction { #[allow(unused_variables)] fn from(ix :
        $raw_name) -> ::quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.authority, true),
        ::quasar_lang::client::AccountMeta::new(ix.old_data, false),]; accounts.extend(ix
        .remaining_accounts); let data = { let mut _data = ::alloc::vec![$($disc),*];
        $(_data.extend_from_slice(& < $arg_ty as
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
    (::quasar_lang::idl_build::s("CloseAccounts"),
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::AccountsMetaEntry::Node(::quasar_lang::idl_build::__reexport::IdlAccountNode
    { name : ::quasar_lang::idl_build::s("authority"), optional : false, writable :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), signer :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), resolver :
    ::quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    ::quasar_lang::idl_build::Vec::new(), }),
    ::quasar_lang::idl_build::AccountsMetaEntry::Node(::quasar_lang::idl_build::__reexport::IdlAccountNode
    { name : ::quasar_lang::idl_build::s("oldData"), optional : false, writable :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), signer :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), resolver :
    ::quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    ::quasar_lang::idl_build::Vec::new(), })],) })
}
#[cfg(feature = "idl-build")]
::quasar_lang::__private_inventory::submit! {
    ::quasar_lang::idl_build::AccountsValidationFragment(|| {
    (::quasar_lang::idl_build::s("CloseAccounts"),
    ::quasar_lang::idl_build::__reexport::IdlAccountsValidation { rent :
    ::quasar_lang::idl_build::s("NotNeeded"), accounts :
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::__reexport::IdlAccountValidation
    { name : ::quasar_lang::idl_build::s("authority"), account_type :
    ::quasar_lang::idl_build::s("Signer"), wrapper :
    ::quasar_lang::idl_build::s("Signer"), writable : true, signer : true, optional :
    false, allow_duplicate : false, load :
    ::quasar_lang::idl_build::s("Fixed(validates=[])"), pre_load :
    ::quasar_lang::idl_build::vec![], post_load : ::quasar_lang::idl_build::vec![],
    epilogue : ::quasar_lang::idl_build::vec![], },
    ::quasar_lang::idl_build::__reexport::IdlAccountValidation { name :
    ::quasar_lang::idl_build::s("oldData"), account_type :
    ::quasar_lang::idl_build::s("Account < OldData >"), wrapper :
    ::quasar_lang::idl_build::s("Account"), writable : true, signer : false, optional :
    false, allow_duplicate : false, load :
    ::quasar_lang::idl_build::s("Fixed(validates=[])"), pre_load :
    ::quasar_lang::idl_build::vec![], post_load : ::quasar_lang::idl_build::vec![],
    epilogue :
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::s("ProgramClose(destination_field=authority)")],
    }], },) })
}
