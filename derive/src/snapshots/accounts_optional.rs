#[derive(Copy, Clone)]
pub struct OptionalAccountsBumps;
impl OptionalAccounts {}
impl quasar_lang::traits::AccountBumps for OptionalAccounts {
    type Bumps = OptionalAccountsBumps;
}
impl quasar_lang::traits::AccountGroup for OptionalAccounts {}
impl<'input> ParseAccounts<'input> for OptionalAccounts {
    type Bumps = OptionalAccountsBumps;
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
for OptionalAccounts {
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
        let [authority, config] = accounts else {
            unsafe { core::hint::unreachable_unchecked() }
        };
        let authority = <Signer as quasar_lang::account_load::AccountLoad>::load(
            authority,
        )?;
        let config = if quasar_lang::keys_eq(config.address(), __program_id) {
            None
        } else {
            Some(
                <Account<
                    Config,
                > as quasar_lang::account_load::AccountLoad>::load(config)?,
            )
        };
        if let Some(ref config) = config {
            quasar_lang::validation::check_address_match(
                &config.authority,
                authority.to_account_view().address(),
                QuasarError::HasOneMismatch.into(),
            )?;
        }
        Ok((Self { authority, config }, OptionalAccountsBumps))
    }
}
impl AccountCount for OptionalAccounts {
    const COUNT: usize = 2usize;
    const NEEDS_EVENT_CPI: bool = false || false || false;
}
impl OptionalAccounts {
    #[inline(always)]
    #[doc(hidden)]
    pub unsafe fn parse_accounts(
        mut input: *mut u8,
        buf: &mut core::mem::MaybeUninit<[quasar_lang::__internal::AccountView; 2usize]>,
        __program_id: &quasar_lang::prelude::Address,
    ) -> Result<*mut u8, ProgramError> {
        let base = buf.as_mut_ptr() as *mut quasar_lang::__internal::AccountView;
        {
            const __EXPECTED: u32 = {
                const __S: bool = <Signer as quasar_lang::account_load::AccountLoad>::IS_SIGNER;
                const __E: bool = <Signer as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE;
                0xFFu32 | (__S as u32) << 8 | 0u32 | (__E as u32) << 24
            };
            const __MASK: u32 = {
                const __S: bool = <Signer as quasar_lang::account_load::AccountLoad>::IS_SIGNER;
                const __E: bool = <Signer as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE;
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
                concat!("Account '", stringify!(authority), "' (index ", "0usize",
                "): validation passed")
            );
        }
        {
            const __EXPECTED: u32 = {
                const __S: bool = <Account<
                    Config,
                > as quasar_lang::account_load::AccountLoad>::IS_SIGNER;
                const __E: bool = <Account<
                    Config,
                > as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE;
                0xFFu32 | (__S as u32) << 8 | 0u32 | (__E as u32) << 24
            };
            const __MASK: u32 = {
                const __S: bool = <Account<
                    Config,
                > as quasar_lang::account_load::AccountLoad>::IS_SIGNER;
                const __E: bool = <Account<
                    Config,
                > as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE;
                0xFFu32 | (if __S { 0xFFu32 << 8 } else { 0u32 }) | 0u32
                    | (if __E { 0xFFu32 << 24 } else { 0u32 })
            };
            const __FLAG_MASK: u32 = {
                const __S: bool = <Account<
                    Config,
                > as quasar_lang::account_load::AccountLoad>::IS_SIGNER;
                const __E: bool = <Account<
                    Config,
                > as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE;
                (if __S { 0xFFu32 << 8 } else { 0u32 }) | 0u32
                    | (if __E { 0xFFu32 << 24 } else { 0u32 })
            };
            input = unsafe {
                quasar_lang::__internal::parse_account_dup(
                    input,
                    base,
                    0usize + 1usize,
                    __program_id,
                    quasar_lang::__internal::ParseFlags {
                        expected: __EXPECTED,
                        mask: __MASK,
                        flag_mask: __FLAG_MASK,
                        is_optional: true,
                        is_ref_mut: false,
                        allow_dup: false,
                    },
                )?
            };
            quasar_lang::debug_log!(
                concat!("Account '", stringify!(config), "' (index ", "0usize + 1usize",
                "): parsed (dup-aware)")
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
    ) -> Result<(Self, OptionalAccountsBumps), ProgramError> {
        let mut __buf = core::mem::MaybeUninit::<
            [quasar_lang::__internal::AccountView; 2usize],
        >::uninit();
        let _ = Self::parse_accounts(input, &mut __buf, __program_id)?;
        let mut __accounts = unsafe { __buf.assume_init() };
        let accounts = &mut __accounts;
        let __parsed_result: Result<
            (Self, <Self as quasar_lang::traits::ParseAccounts>::Bumps),
            ProgramError,
        > = {
            let [authority, config] = accounts else {
                unsafe { core::hint::unreachable_unchecked() }
            };
            let authority = <Signer as quasar_lang::account_load::AccountLoad>::load(
                authority,
            )?;
            let config = if quasar_lang::keys_eq(config.address(), __program_id) {
                None
            } else {
                Some(
                    <Account<
                        Config,
                    > as quasar_lang::account_load::AccountLoad>::load(config)?,
                )
            };
            if let Some(ref config) = config {
                quasar_lang::validation::check_address_match(
                    &config.authority,
                    authority.to_account_view().address(),
                    QuasarError::HasOneMismatch.into(),
                )?;
            }
            Ok((Self { authority, config }, OptionalAccountsBumps))
        };
        let (__parsed_accounts, __parsed_bumps) = __parsed_result?;
        Ok((__parsed_accounts, __parsed_bumps))
    }
}
unsafe impl quasar_lang::traits::ParseAccountsRaw for OptionalAccounts {
    #[inline(always)]
    unsafe fn parse_accounts_raw(
        input: *mut u8,
        base: *mut quasar_lang::__internal::AccountView,
        offset: usize,
        __program_id: &quasar_lang::prelude::Address,
    ) -> Result<*mut u8, ProgramError> {
        let mut __inner_buf = core::mem::MaybeUninit::<
            [quasar_lang::__internal::AccountView; 2usize],
        >::uninit();
        let input = Self::parse_accounts(input, &mut __inner_buf, __program_id)?;
        let __inner = core::mem::ManuallyDrop::new(__inner_buf.assume_init());
        let mut __j = 0usize;
        while __j < 2usize {
            core::ptr::write(
                base.add(offset + __j),
                core::ptr::read(__inner.as_ptr().add(__j)),
            );
            __j += 1;
        }
        Ok(input)
    }
}
impl<'input> quasar_lang::remaining::RemainingItem<'input> for OptionalAccounts {
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
macro_rules! __optional_accounts_instruction {
    ($struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* }) => {
        pub struct $struct_name { pub authority : quasar_lang::prelude::Address, pub
        config : quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* } impl From
        < $struct_name > for quasar_lang::client::Instruction { fn from(ix :
        $struct_name) -> quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new_readonly(ix.authority, true),
        quasar_lang::client::AccountMeta::new_readonly(ix.config, false),]; let data = {
        let mut _data = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty
        as quasar_lang::client::SerializeArg > ::serialize_arg(& ix. $arg_name));)* _data
        }; quasar_lang::client::Instruction { program_id : $crate::ID, accounts, data, }
        } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        compact
    ) => {
        pub struct $struct_name { pub authority : quasar_lang::prelude::Address, pub
        config : quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* } impl From
        < $struct_name > for quasar_lang::client::Instruction { fn from(ix :
        $struct_name) -> quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new_readonly(ix.authority, true),
        quasar_lang::client::AccountMeta::new_readonly(ix.config, false),]; let data = {
        let mut _data = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty
        as quasar_lang::client::CompactSerializeArg > ::compact_header(& ix.
        $arg_name));)* $(_data.extend_from_slice(& < $arg_ty as
        quasar_lang::client::CompactSerializeArg > ::compact_tail(& ix. $arg_name));)*
        _data }; quasar_lang::client::Instruction { program_id : $crate::ID, accounts,
        data, } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        remaining
    ) => {
        pub struct $struct_name { pub authority : quasar_lang::prelude::Address, pub
        config : quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* pub
        remaining_accounts : ::alloc::vec::Vec < quasar_lang::client::AccountMeta >, }
        impl From < $struct_name > for quasar_lang::client::Instruction { fn from(ix :
        $struct_name) -> quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new_readonly(ix.authority, true),
        quasar_lang::client::AccountMeta::new_readonly(ix.config, false),]; accounts
        .extend(ix.remaining_accounts); let data = { let mut _data =
        ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
        quasar_lang::client::SerializeArg > ::serialize_arg(& ix. $arg_name));)* _data };
        quasar_lang::client::Instruction { program_id : $crate::ID, accounts, data, } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        compact, remaining
    ) => {
        pub struct $struct_name { pub authority : quasar_lang::prelude::Address, pub
        config : quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* pub
        remaining_accounts : ::alloc::vec::Vec < quasar_lang::client::AccountMeta >, }
        impl From < $struct_name > for quasar_lang::client::Instruction { fn from(ix :
        $struct_name) -> quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new_readonly(ix.authority, true),
        quasar_lang::client::AccountMeta::new_readonly(ix.config, false),]; accounts
        .extend(ix.remaining_accounts); let data = { let mut _data =
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
    (quasar_lang::idl_build::s("OptionalAccounts"),
    quasar_lang::idl_build::vec![quasar_lang::idl_build::__reexport::IdlAccountNode {
    name : quasar_lang::idl_build::s("authority"), client_type : None, optional : false,
    writable : quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), signer :
    quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), resolver :
    quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    quasar_lang::idl_build::Vec::new(), },
    quasar_lang::idl_build::__reexport::IdlAccountNode { name :
    quasar_lang::idl_build::s("config"), client_type : None, optional : true, writable :
    quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), signer :
    quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), resolver :
    quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    quasar_lang::idl_build::Vec::new(), }],) })
}
