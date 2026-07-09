#[derive(Copy, Clone)]
pub struct UseCustomBehaviorBumps;
impl UseCustomBehavior {}
impl quasar_lang::traits::AccountBumps for UseCustomBehavior {
    type Bumps = UseCustomBehaviorBumps;
}
impl quasar_lang::traits::AccountGroup for UseCustomBehavior {}
impl<'input> ParseAccounts<'input> for UseCustomBehavior {
    type Bumps = UseCustomBehaviorBumps;
    const HAS_EPILOGUE: bool = false;
    #[inline(always)]
    fn parse(
        accounts: &'input mut [AccountView],
        program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError> {
        quasar_lang::traits::check_account_count(accounts.len(), Self::COUNT)?;
        unsafe {
            <Self as quasar_lang::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
                accounts,
                &[],
                program_id,
            )
        }
    }
    #[inline(always)]
    fn parse_with_instruction_data(
        accounts: &'input mut [AccountView],
        __ix_data: &[u8],
        __program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError> {
        quasar_lang::traits::check_account_count(accounts.len(), Self::COUNT)?;
        unsafe {
            <Self as quasar_lang::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
                accounts,
                __ix_data,
                __program_id,
            )
        }
    }
}
unsafe impl<'input> quasar_lang::traits::ParseAccountsUnchecked<'input>
for UseCustomBehavior {
    #[inline(always)]
    unsafe fn parse_unchecked(
        accounts: &'input mut [AccountView],
        program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError> {
        <Self as quasar_lang::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
            accounts,
            &[],
            program_id,
        )
    }
    #[inline(always)]
    unsafe fn parse_with_instruction_data_unchecked(
        accounts: &'input mut [AccountView],
        __ix_data: &[u8],
        __program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError> {
        let [data] = accounts else { unsafe { core::hint::unreachable_unchecked() } };
        const _: () = assert!(
            ! < min_value::Behavior as quasar_lang::account_behavior::AccountBehavior <
            Account < MyData > >> ::REQUIRES_MUT,
            "behavior `min_value` requires `#[account(mut)]` on field `data`",
        );
        const _: () = assert!(
            ! < min_value::Behavior as quasar_lang::account_behavior::AccountBehavior <
            Account < MyData > >> ::VALIDATES_ACCOUNT_DATA || < min_value::Behavior as
            quasar_lang::account_behavior::AccountBehavior < Account < MyData > >>
            ::RUN_CHECK,
            "behavior `min_value` sets VALIDATES_ACCOUNT_DATA and must keep RUN_CHECK = true",
        );
        const _: () = assert!(
            ! < min_value::Behavior as quasar_lang::account_behavior::AccountBehavior <
            Account < MyData > >> ::RUN_AFTER_INIT,
            "behavior `min_value` runs after_init and requires `#[account(init, ...)]` on field `data`",
        );
        let data = if false
            || <min_value::Behavior as quasar_lang::account_behavior::AccountBehavior<
                Account<MyData>,
            >>::VALIDATES_ACCOUNT_DATA
        {
            unsafe {
                <Account<
                    MyData,
                > as quasar_lang::account_load::AccountLoad>::load_intrinsic(data)?
            }
        } else {
            <Account<MyData> as quasar_lang::account_load::AccountLoad>::load(data)?
        };
        if <min_value::Behavior as quasar_lang::account_behavior::AccountBehavior<
            Account<MyData>,
        >>::RUN_CHECK && true
        {
            let __bhv_builder = min_value::Args::builder();
            let __bhv_builder = if <min_value::Behavior as quasar_lang::account_behavior::AccountBehavior<
                Account<MyData>,
            >>::uses_arg::<
                { quasar_lang::account_behavior::ARG_PHASE_CHECK },
                { quasar_lang::account_behavior::behavior_arg_key_hash("min") },
            >() {
                __bhv_builder.min(10u64)
            } else {
                __bhv_builder
            };
            fn __assert_builder<__B: quasar_lang::account_behavior::BehaviorArgsBuilder>(
                _: &__B,
            ) {}
            __assert_builder(&__bhv_builder);
            let __bhv_args = quasar_lang::account_behavior::BehaviorArgsBuilder::build_check(
                __bhv_builder,
            )?;
            <min_value::Behavior as quasar_lang::account_behavior::AccountBehavior<
                Account<MyData>,
            >>::check(&data, &__bhv_args)?;
        }
        Ok((Self { data }, UseCustomBehaviorBumps))
    }
}
impl AccountCount for UseCustomBehavior {
    const COUNT: usize = 1usize;
    const NEEDS_EVENT_CPI: bool = false || false;
}
impl UseCustomBehavior {
    #[inline(always)]
    #[doc(hidden)]
    pub unsafe fn parse_accounts(
        mut input: *mut u8,
        buf: &mut core::mem::MaybeUninit<[quasar_lang::__internal::AccountView; 1usize]>,
        __program_id: &quasar_lang::prelude::Address,
    ) -> Result<*mut u8, ProgramError> {
        let base = buf.as_mut_ptr() as *mut quasar_lang::__internal::AccountView;
        {
            const __EXPECTED: u32 = {
                const __S: bool = <Account<
                    MyData,
                > as quasar_lang::account_load::AccountLoad>::IS_SIGNER;
                const __E: bool = <Account<
                    MyData,
                > as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE;
                0xFFu32 | (__S as u32) << 8 | 0u32 | (__E as u32) << 24
            };
            const __MASK: u32 = {
                const __S: bool = <Account<
                    MyData,
                > as quasar_lang::account_load::AccountLoad>::IS_SIGNER;
                const __E: bool = <Account<
                    MyData,
                > as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE;
                0xFFu32 | (if __S { 0xFFu32 << 8 } else { 0u32 }) | 0u32
                    | (if __E { 0xFFu32 << 24 } else { 0u32 })
            };
            input = unsafe {
                quasar_lang::__internal::parse_account(
                    input,
                    base,
                    0usize,
                    __EXPECTED,
                    __MASK,
                )?
            };
            quasar_lang::debug_log!(
                concat!("Account '", stringify!(data), "' (index ", "0usize",
                "): validation passed")
            );
        }
        Ok(input)
    }
    #[inline(always)]
    #[doc(hidden)]
    pub unsafe fn parse_direct_with_instruction_data_unchecked(
        mut input: *mut u8,
        __ix_data: &[u8],
        __program_id: &quasar_lang::prelude::Address,
    ) -> Result<(Self, UseCustomBehaviorBumps), ProgramError> {
        let mut __buf = core::mem::MaybeUninit::<
            [quasar_lang::__internal::AccountView; 1usize],
        >::uninit();
        let _ = Self::parse_accounts(input, &mut __buf, __program_id)?;
        let mut __accounts = unsafe { __buf.assume_init() };
        let accounts = &mut __accounts;
        let __parsed_result: Result<
            (Self, <Self as quasar_lang::traits::ParseAccounts>::Bumps),
            ProgramError,
        > = {
            let [data] = accounts else {
                unsafe { core::hint::unreachable_unchecked() }
            };
            let data = if false
                || <min_value::Behavior as quasar_lang::account_behavior::AccountBehavior<
                    Account<MyData>,
                >>::VALIDATES_ACCOUNT_DATA
            {
                unsafe {
                    <Account<
                        MyData,
                    > as quasar_lang::account_load::AccountLoad>::load_intrinsic(data)?
                }
            } else {
                <Account<MyData> as quasar_lang::account_load::AccountLoad>::load(data)?
            };
            if <min_value::Behavior as quasar_lang::account_behavior::AccountBehavior<
                Account<MyData>,
            >>::RUN_CHECK && true
            {
                let __bhv_builder = min_value::Args::builder();
                let __bhv_builder = if <min_value::Behavior as quasar_lang::account_behavior::AccountBehavior<
                    Account<MyData>,
                >>::uses_arg::<
                    { quasar_lang::account_behavior::ARG_PHASE_CHECK },
                    { quasar_lang::account_behavior::behavior_arg_key_hash("min") },
                >() {
                    __bhv_builder.min(10u64)
                } else {
                    __bhv_builder
                };
                fn __assert_builder<
                    __B: quasar_lang::account_behavior::BehaviorArgsBuilder,
                >(_: &__B) {}
                __assert_builder(&__bhv_builder);
                let __bhv_args = quasar_lang::account_behavior::BehaviorArgsBuilder::build_check(
                    __bhv_builder,
                )?;
                <min_value::Behavior as quasar_lang::account_behavior::AccountBehavior<
                    Account<MyData>,
                >>::check(&data, &__bhv_args)?;
            }
            Ok((Self { data }, UseCustomBehaviorBumps))
        };
        let (__parsed_accounts, __parsed_bumps) = __parsed_result?;
        Ok((__parsed_accounts, __parsed_bumps))
    }
}
unsafe impl quasar_lang::traits::ParseAccountsRaw for UseCustomBehavior {
    #[inline(always)]
    unsafe fn parse_accounts_raw(
        input: *mut u8,
        base: *mut quasar_lang::__internal::AccountView,
        offset: usize,
        __program_id: &quasar_lang::prelude::Address,
    ) -> Result<*mut u8, ProgramError> {
        let mut __inner_buf = core::mem::MaybeUninit::<
            [quasar_lang::__internal::AccountView; 1usize],
        >::uninit();
        let input = Self::parse_accounts(input, &mut __inner_buf, __program_id)?;
        let __inner = core::mem::ManuallyDrop::new(__inner_buf.assume_init());
        let mut __j = 0usize;
        while __j < 1usize {
            core::ptr::write(
                base.add(offset + __j),
                core::ptr::read(__inner.as_ptr().add(__j)),
            );
            __j += 1;
        }
        Ok(input)
    }
}
impl<'input> quasar_lang::remaining::RemainingItem<'input> for UseCustomBehavior {
    const COUNT: usize = <Self as quasar_lang::traits::AccountCount>::COUNT;
    #[inline(always)]
    unsafe fn parse_remaining_chunk(
        accounts: &'input mut [quasar_lang::__internal::AccountView],
        program_id: Option<&quasar_lang::prelude::Address>,
        data: &[u8],
    ) -> Result<Self, ProgramError> {
        let program_id = program_id.ok_or(ProgramError::InvalidInstructionData)?;
        let (item, _bumps) = <Self as quasar_lang::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
            accounts,
            data,
            program_id,
        )?;
        Ok(item)
    }
}
#[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __use_custom_behavior_instruction {
    ($struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* }) => {
        pub struct $struct_name { pub data : quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* } impl From < $struct_name > for
        quasar_lang::client::Instruction { fn from(ix : $struct_name) ->
        quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new_readonly(ix.data, false),];
        let data = { let mut _data = ::alloc::vec![$($disc),*]; $(_data
        .extend_from_slice(& < $arg_ty as quasar_lang::client::SerializeArg >
        ::serialize_arg(& ix. $arg_name));)* _data }; quasar_lang::client::Instruction {
        program_id : $crate::ID, accounts, data, } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        compact
    ) => {
        pub struct $struct_name { pub data : quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* } impl From < $struct_name > for
        quasar_lang::client::Instruction { fn from(ix : $struct_name) ->
        quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new_readonly(ix.data, false),];
        let data = { let mut _data = ::alloc::vec![$($disc),*]; $(_data
        .extend_from_slice(& < $arg_ty as quasar_lang::client::CompactSerializeArg >
        ::compact_header(& ix. $arg_name));)* $(_data.extend_from_slice(& < $arg_ty as
        quasar_lang::client::CompactSerializeArg > ::compact_tail(& ix. $arg_name));)*
        _data }; quasar_lang::client::Instruction { program_id : $crate::ID, accounts,
        data, } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        remaining
    ) => {
        pub struct $struct_name { pub data : quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* pub remaining_accounts : ::alloc::vec::Vec <
        quasar_lang::client::AccountMeta >, } impl From < $struct_name > for
        quasar_lang::client::Instruction { fn from(ix : $struct_name) ->
        quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new_readonly(ix.data, false),];
        accounts.extend(ix.remaining_accounts); let data = { let mut _data =
        ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
        quasar_lang::client::SerializeArg > ::serialize_arg(& ix. $arg_name));)* _data };
        quasar_lang::client::Instruction { program_id : $crate::ID, accounts, data, } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        compact, remaining
    ) => {
        pub struct $struct_name { pub data : quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* pub remaining_accounts : ::alloc::vec::Vec <
        quasar_lang::client::AccountMeta >, } impl From < $struct_name > for
        quasar_lang::client::Instruction { fn from(ix : $struct_name) ->
        quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new_readonly(ix.data, false),];
        accounts.extend(ix.remaining_accounts); let data = { let mut _data =
        ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
        quasar_lang::client::CompactSerializeArg > ::compact_header(& ix. $arg_name));)*
        $(_data.extend_from_slice(& < $arg_ty as quasar_lang::client::CompactSerializeArg
        > ::compact_tail(& ix. $arg_name));)* _data }; quasar_lang::client::Instruction {
        program_id : $crate::ID, accounts, data, } } }
    };
}
#[cfg(feature = "idl-build")]
quasar_lang::__private_inventory::submit! {
    quasar_lang::idl_build::AccountsMetaFragment(|| {
    (quasar_lang::idl_build::s("UseCustomBehavior"),
    quasar_lang::idl_build::vec![quasar_lang::idl_build::__reexport::IdlAccountNode {
    name : quasar_lang::idl_build::s("data"), optional : false, writable :
    quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), signer :
    quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), resolver :
    quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    quasar_lang::idl_build::Vec::new(), }],) })
}
