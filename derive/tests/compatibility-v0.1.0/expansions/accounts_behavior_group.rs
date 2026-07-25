#[derive(Copy, Clone)]
pub struct UseCustomBehaviorBumps;
impl ::quasar_lang::traits::AccountBumps for UseCustomBehavior {
    type Bumps = UseCustomBehaviorBumps;
}
impl<'input> ::quasar_lang::traits::ParseAccounts<'input> for UseCustomBehavior {
    type Bumps = UseCustomBehaviorBumps;
    const HAS_EPILOGUE: bool = false;
}
unsafe impl<'input> ::quasar_lang::traits::ParseAccountsUnchecked<'input>
for UseCustomBehavior {
    #[inline(always)]
    unsafe fn parse_with_instruction_data_unchecked(
        accounts: &'input mut [::quasar_lang::__internal::AccountView],
        __ix_data: &[u8],
        __program_id: &::quasar_lang::prelude::Address,
    ) -> Result<
        (Self, Self::Bumps),
        ::quasar_lang::__solana_program_error::ProgramError,
    > {
        let [data] = accounts else { unsafe { core::hint::unreachable_unchecked() } };
        const _: () = assert!(
            ! < min_value::Behavior as ::quasar_lang::account_behavior::AccountBehavior <
            Account < MyData > >> ::REQUIRES_MUT,
            "behavior `min_value` requires `#[account(mut)]` on field `data`",
        );
        const _: () = assert!(
            ! < min_value::Behavior as ::quasar_lang::account_behavior::AccountBehavior <
            Account < MyData > >> ::VALIDATES_ACCOUNT_DATA || < min_value::Behavior as
            ::quasar_lang::account_behavior::AccountBehavior < Account < MyData > >>
            ::RUN_CHECK,
            "behavior `min_value` sets VALIDATES_ACCOUNT_DATA and must keep RUN_CHECK = true",
        );
        const _: () = assert!(
            ! < min_value::Behavior as ::quasar_lang::account_behavior::AccountBehavior <
            Account < MyData > >> ::RUN_AFTER_INIT,
            "behavior `min_value` runs after_init and requires `#[account(init, ...)]` on field `data`",
        );
        let data = if <min_value::Behavior as ::quasar_lang::account_behavior::AccountBehavior<
            Account<MyData>,
        >>::VALIDATES_ACCOUNT_DATA {
            unsafe {
                <Account<
                    MyData,
                > as ::quasar_lang::account_load::AccountLoad>::load_intrinsic(data)?
            }
        } else {
            <Account<MyData> as ::quasar_lang::account_load::AccountLoad>::load(data)?
        };
        if <min_value::Behavior as ::quasar_lang::account_behavior::AccountBehavior<
            Account<MyData>,
        >>::RUN_CHECK {
            let __bhv_builder = min_value::Args::builder();
            let __bhv_builder = if <min_value::Behavior as ::quasar_lang::account_behavior::AccountBehavior<
                Account<MyData>,
            >>::uses_arg::<
                { ::quasar_lang::account_behavior::ARG_PHASE_CHECK },
                { ::quasar_lang::account_behavior::behavior_arg_key_hash("min") },
            >() {
                __bhv_builder.min(10u64)
            } else {
                __bhv_builder
            };
            Self::__assert_builder(&__bhv_builder);
            let __bhv_args = ::quasar_lang::account_behavior::BehaviorArgsBuilder::build_check(
                __bhv_builder,
            )?;
            <min_value::Behavior as ::quasar_lang::account_behavior::AccountBehavior<
                Account<MyData>,
            >>::check(&data, &__bhv_args)?;
        }
        Ok((Self { data }, UseCustomBehaviorBumps))
    }
}
impl ::quasar_lang::traits::AccountCount for UseCustomBehavior {
    const COUNT: usize = 1usize;
    const NEEDS_EVENT_CPI: bool = false;
}
impl UseCustomBehavior {
    #[inline(always)]
    fn __assert_builder<__B: ::quasar_lang::account_behavior::BehaviorArgsBuilder>(
        _: &__B,
    ) {}
}
unsafe impl ::quasar_lang::traits::ParseAccountsRaw for UseCustomBehavior {
    #[inline(always)]
    unsafe fn parse_accounts_raw(
        mut input: *mut u8,
        base: *mut ::quasar_lang::__internal::AccountView,
        __offset: usize,
        __program_id: &::quasar_lang::prelude::Address,
    ) -> Result<*mut u8, ::quasar_lang::__solana_program_error::ProgramError> {
        {
            const __HEADER: ::quasar_lang::__internal::HeaderSpec = ::quasar_lang::__internal::HeaderSpec::of::<
                Account<MyData>,
            >(false);
            input = unsafe {
                ::quasar_lang::__internal::parse_account(
                    input,
                    base,
                    __offset + 0usize,
                    __HEADER.expected,
                    __HEADER.mask,
                )?
            };
            ::quasar_lang::debug_log!(
                concat!("Account '", stringify!(data), "' (index ", "0",
                "): validation passed")
            );
        }
        Ok(input)
    }
}
impl<'input> ::quasar_lang::remaining::RemainingItem<'input> for UseCustomBehavior {
    const COUNT: usize = <Self as ::quasar_lang::traits::AccountCount>::COUNT;
    #[inline(always)]
    unsafe fn parse_remaining_chunk(
        accounts: &'input mut [::quasar_lang::__internal::AccountView],
        program_id: Option<&::quasar_lang::prelude::Address>,
        data: &[u8],
    ) -> Result<Self, ::quasar_lang::__solana_program_error::ProgramError> {
        let program_id = program_id
            .ok_or(
                ::quasar_lang::__solana_program_error::ProgramError::InvalidInstructionData,
            )?;
        let (item, _bumps) = <Self as ::quasar_lang::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
            accounts,
            data,
            program_id,
        )?;
        Ok(item)
    }
}
#[doc(hidden)]
#[allow(unexpected_cfgs)]
#[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
#[macro_export]
macro_rules! __use_custom_behavior_instruction {
    ($struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* }) => {
        pub struct $struct_name { pub data : ::quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* } impl From < $struct_name > for
        ::quasar_lang::client::Instruction { #[allow(unused_variables)] fn from(ix :
        $struct_name) -> ::quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new_readonly(ix.data, false),];
        let data = { let mut _data = ::alloc::vec![$($disc),*]; $(_data
        .extend_from_slice(& < $arg_ty as ::quasar_lang::client::SerializeArg >
        ::serialize_arg(& ix. $arg_name));)* _data }; ::quasar_lang::client::Instruction
        { program_id : $crate::ID, accounts, data, } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        compact
    ) => {
        pub struct $struct_name { pub data : ::quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* } impl From < $struct_name > for
        ::quasar_lang::client::Instruction { #[allow(unused_variables)] fn from(ix :
        $struct_name) -> ::quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new_readonly(ix.data, false),];
        let data = { let mut _data = ::alloc::vec![$($disc),*]; $(_data
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
        pub struct $struct_name { pub data : ::quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* pub remaining_accounts : ::alloc::vec::Vec <
        ::quasar_lang::client::AccountMeta >, } impl From < $struct_name > for
        ::quasar_lang::client::Instruction { #[allow(unused_variables)] fn from(ix :
        $struct_name) -> ::quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new_readonly(ix.data, false),];
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
        pub struct $struct_name { pub data : ::quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* pub remaining_accounts : ::alloc::vec::Vec <
        ::quasar_lang::client::AccountMeta >, } impl From < $struct_name > for
        ::quasar_lang::client::Instruction { #[allow(unused_variables)] fn from(ix :
        $struct_name) -> ::quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new_readonly(ix.data, false),];
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
    (::quasar_lang::idl_build::s("UseCustomBehavior"),
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::AccountsMetaEntry::Node(::quasar_lang::idl_build::__reexport::IdlAccountNode
    { name : ::quasar_lang::idl_build::s("data"), optional : false, writable :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), signer :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), resolver :
    ::quasar_lang::idl_build::one_behavior_resolver("data",
    [::quasar_lang::idl_build::behavior_resolver(< min_value::Behavior as
    ::quasar_lang::account_behavior::AccountBehavior < Account < MyData > >>
    ::IDL_RESOLVER, & [], & ["data"],)],).unwrap_or_else(|| {
    ::quasar_lang::idl_build::__reexport::IdlResolver::Input {} }), docs :
    ::quasar_lang::idl_build::Vec::new(), })],) })
}
#[cfg(feature = "idl-build")]
::quasar_lang::__private_inventory::submit! {
    ::quasar_lang::idl_build::AccountsValidationFragment(|| {
    (::quasar_lang::idl_build::s("UseCustomBehavior"),
    ::quasar_lang::idl_build::__reexport::IdlAccountsValidation { rent :
    ::quasar_lang::idl_build::s("NotNeeded"), accounts :
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::__reexport::IdlAccountValidation
    { name : ::quasar_lang::idl_build::s("data"), account_type :
    ::quasar_lang::idl_build::s("Account < MyData >"), wrapper :
    ::quasar_lang::idl_build::s("Account"), writable : false, signer : false, optional :
    false, allow_duplicate : false, load :
    ::quasar_lang::idl_build::s("Fixed(validates=[min_value])"), pre_load :
    ::quasar_lang::idl_build::vec![], post_load :
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::s("Behavior(path=`min_value` phase=Check args=[min=Expr(`10u64`)])")],
    epilogue : ::quasar_lang::idl_build::vec![], }], },) })
}
