#[derive(Copy, Clone)]
pub struct ReallocAccountsBumps;
impl ::quasar_lang::traits::AccountBumps for ReallocAccounts {
    type Bumps = ReallocAccountsBumps;
}
impl<'input> ::quasar_lang::traits::ParseAccounts<'input> for ReallocAccounts {
    type Bumps = ReallocAccountsBumps;
    const HAS_EPILOGUE: bool = false;
}
unsafe impl<'input> ::quasar_lang::traits::ParseAccountsUnchecked<'input>
for ReallocAccounts {
    #[inline(always)]
    unsafe fn parse_with_instruction_data_unchecked(
        accounts: &'input mut [::quasar_lang::__internal::AccountView],
        __ix_data: &[u8],
        __program_id: &::quasar_lang::prelude::Address,
    ) -> Result<
        (Self, Self::Bumps),
        ::quasar_lang::__solana_program_error::ProgramError,
    > {
        let [payer, data, system_program] = accounts else {
            unsafe { core::hint::unreachable_unchecked() }
        };
        let __rent_ctx = ::quasar_lang::ops::OpCtx::new(
            unsafe { &*(__program_id as *const ::quasar_lang::prelude::Address) },
            ::quasar_lang::ops::RentResolver::fetch_once(),
        );
        let mut payer = <Signer as ::quasar_lang::account_load::AccountLoad>::load_mut(
            payer,
        )?;
        let mut data = <Account<
            MyData,
        > as ::quasar_lang::account_load::AccountLoad>::load_mut(data)?;
        let system_program = <Program<
            SystemProgram,
        > as ::quasar_lang::account_load::AccountLoad>::load(system_program)?;
        {
            let __realloc_op = ::quasar_lang::ops::realloc::Op {
                space: (200) as usize,
                payer: payer.to_account_view(),
            };
            __realloc_op.apply::<Account<MyData>, _>(&mut data, &__rent_ctx)?;
        }
        Ok((
            Self {
                payer,
                data,
                system_program,
            },
            ReallocAccountsBumps,
        ))
    }
}
impl ::quasar_lang::traits::AccountCount for ReallocAccounts {
    const COUNT: usize = 3usize;
    const NEEDS_EVENT_CPI: bool = false;
}
unsafe impl ::quasar_lang::traits::ParseAccountsRaw for ReallocAccounts {
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
            >(input, base, __offset + 0usize)?
        };
        ::quasar_lang::debug_log!("account payer @0: validation passed");
        input = unsafe {
            ::quasar_lang::__internal::parse_account::<
                Account<MyData>,
                true,
            >(input, base, __offset + 1usize)?
        };
        ::quasar_lang::debug_log!("account data @1: validation passed");
        input = unsafe {
            ::quasar_lang::__internal::parse_account::<
                Program<SystemProgram>,
                false,
            >(input, base, __offset + 2usize)?
        };
        ::quasar_lang::debug_log!("account system_program @2: validation passed");
        Ok(input)
    }
}
impl<'input> ::quasar_lang::remaining::RemainingItem<'input> for ReallocAccounts {
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
macro_rules! __realloc_accounts_instruction {
    ($struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* }) => {
        pub struct $struct_name { pub payer : ::quasar_lang::prelude::Address, pub data :
        ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* } impl From <
        $struct_name > for ::quasar_lang::client::Instruction {
        #[allow(unused_variables)] fn from(ix : $struct_name) ->
        ::quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.payer, true),
        ::quasar_lang::client::AccountMeta::new(ix.data, false),
        ::quasar_lang::client::AccountMeta::new_readonly(super::ReallocAccounts::__QUASAR_FIXED_ADDRESS_system_program,
        false),]; let data = { let mut _data = ::alloc::vec![$($disc),*]; $(_data
        .extend_from_slice(& < $arg_ty as ::quasar_lang::client::SerializeArg >
        ::serialize_arg(& ix. $arg_name));)* _data }; ::quasar_lang::client::Instruction
        { program_id : $crate::ID, accounts, data, } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        compact
    ) => {
        pub struct $struct_name { pub payer : ::quasar_lang::prelude::Address, pub data :
        ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* } impl From <
        $struct_name > for ::quasar_lang::client::Instruction {
        #[allow(unused_variables)] fn from(ix : $struct_name) ->
        ::quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.payer, true),
        ::quasar_lang::client::AccountMeta::new(ix.data, false),
        ::quasar_lang::client::AccountMeta::new_readonly(super::ReallocAccounts::__QUASAR_FIXED_ADDRESS_system_program,
        false),]; let data = { let mut _data = ::alloc::vec![$($disc),*]; $(_data
        .extend_from_slice(& < $arg_ty as ::quasar_lang::client::CompactSerializeArg >
        ::compact_header(& ix. $arg_name));)* $(_data.extend_from_slice(& < $arg_ty as
        ::quasar_lang::client::CompactSerializeArg > ::compact_tail(& ix. $arg_name));)*
        _data }; ::quasar_lang::client::Instruction { program_id : $crate::ID, accounts,
        data, } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        remaining
    ) => {
        pub struct $struct_name { pub payer : ::quasar_lang::prelude::Address, pub data :
        ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* pub
        remaining_accounts : ::alloc::vec::Vec < ::quasar_lang::client::AccountMeta >, }
        impl From < $struct_name > for ::quasar_lang::client::Instruction {
        #[allow(unused_variables)] fn from(ix : $struct_name) ->
        ::quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.payer, true),
        ::quasar_lang::client::AccountMeta::new(ix.data, false),
        ::quasar_lang::client::AccountMeta::new_readonly(super::ReallocAccounts::__QUASAR_FIXED_ADDRESS_system_program,
        false),]; accounts.extend(ix.remaining_accounts); let data = { let mut _data =
        ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
        ::quasar_lang::client::SerializeArg > ::serialize_arg(& ix. $arg_name));)* _data
        }; ::quasar_lang::client::Instruction { program_id : $crate::ID, accounts, data,
        } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        compact, remaining
    ) => {
        pub struct $struct_name { pub payer : ::quasar_lang::prelude::Address, pub data :
        ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* pub
        remaining_accounts : ::alloc::vec::Vec < ::quasar_lang::client::AccountMeta >, }
        impl From < $struct_name > for ::quasar_lang::client::Instruction {
        #[allow(unused_variables)] fn from(ix : $struct_name) ->
        ::quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.payer, true),
        ::quasar_lang::client::AccountMeta::new(ix.data, false),
        ::quasar_lang::client::AccountMeta::new_readonly(super::ReallocAccounts::__QUASAR_FIXED_ADDRESS_system_program,
        false),]; accounts.extend(ix.remaining_accounts); let data = { let mut _data =
        ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
        ::quasar_lang::client::CompactSerializeArg > ::compact_header(& ix.
        $arg_name));)* $(_data.extend_from_slice(& < $arg_ty as
        ::quasar_lang::client::CompactSerializeArg > ::compact_tail(& ix. $arg_name));)*
        _data }; ::quasar_lang::client::Instruction { program_id : $crate::ID, accounts,
        data, } } }
    };
}
#[allow(non_upper_case_globals)]
impl ReallocAccounts {
    #[doc(hidden)]
    pub const __QUASAR_FIXED_ADDRESS_system_program: ::quasar_lang::prelude::Address = <SystemProgram as ::quasar_lang::traits::Id>::ID;
}
#[cfg(feature = "idl-build")]
::quasar_lang::__private_inventory::submit! {
    ::quasar_lang::idl_build::AccountsMetaFragment(|| {
    (::quasar_lang::idl_build::s("ReallocAccounts"),
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::AccountsMetaEntry::Node(::quasar_lang::idl_build::__reexport::IdlAccountNode
    { name : ::quasar_lang::idl_build::s("payer"), optional : false, writable :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), signer :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), resolver :
    ::quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    ::quasar_lang::idl_build::Vec::new(), }),
    ::quasar_lang::idl_build::AccountsMetaEntry::Node(::quasar_lang::idl_build::__reexport::IdlAccountNode
    { name : ::quasar_lang::idl_build::s("data"), optional : false, writable :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), signer :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), resolver :
    ::quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    ::quasar_lang::idl_build::Vec::new(), }),
    ::quasar_lang::idl_build::AccountsMetaEntry::Node(::quasar_lang::idl_build::__reexport::IdlAccountNode
    { name : ::quasar_lang::idl_build::s("systemProgram"), optional : false, writable :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), signer :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), resolver :
    ::quasar_lang::idl_build::__reexport::IdlResolver::Const { address :
    ::quasar_lang::idl_build::address_to_base58(& < SystemProgram as
    ::quasar_lang::traits::Id > ::ID), }, docs : ::quasar_lang::idl_build::Vec::new(),
    })],) })
}
#[cfg(feature = "idl-build")]
::quasar_lang::__private_inventory::submit! {
    ::quasar_lang::idl_build::AccountsValidationFragment(|| {
    (::quasar_lang::idl_build::s("ReallocAccounts"),
    ::quasar_lang::idl_build::__reexport::IdlAccountsValidation { rent :
    ::quasar_lang::idl_build::s("FetchOnce"), accounts :
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::__reexport::IdlAccountValidation
    { name : ::quasar_lang::idl_build::s("payer"), account_type :
    ::quasar_lang::idl_build::s("Signer"), wrapper :
    ::quasar_lang::idl_build::s("Signer"), writable : true, signer : true, optional :
    false, allow_duplicate : false, load :
    ::quasar_lang::idl_build::s("Fixed(validates=[])"), pre_load :
    ::quasar_lang::idl_build::vec![], post_load : ::quasar_lang::idl_build::vec![],
    epilogue : ::quasar_lang::idl_build::vec![], },
    ::quasar_lang::idl_build::__reexport::IdlAccountValidation { name :
    ::quasar_lang::idl_build::s("data"), account_type :
    ::quasar_lang::idl_build::s("Account < MyData >"), wrapper :
    ::quasar_lang::idl_build::s("Account"), writable : true, signer : false, optional :
    false, allow_duplicate : false, load :
    ::quasar_lang::idl_build::s("Fixed(validates=[])"), pre_load :
    ::quasar_lang::idl_build::vec![], post_load :
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::s("Realloc(new_space=`200` payer=payer)")],
    epilogue : ::quasar_lang::idl_build::vec![], },
    ::quasar_lang::idl_build::__reexport::IdlAccountValidation { name :
    ::quasar_lang::idl_build::s("systemProgram"), account_type :
    ::quasar_lang::idl_build::s("Program < SystemProgram >"), wrapper :
    ::quasar_lang::idl_build::s("Program"), writable : false, signer : false, optional :
    false, allow_duplicate : false, load :
    ::quasar_lang::idl_build::s("Fixed(validates=[])"), pre_load :
    ::quasar_lang::idl_build::vec![], post_load : ::quasar_lang::idl_build::vec![],
    epilogue : ::quasar_lang::idl_build::vec![], }], },) })
}
