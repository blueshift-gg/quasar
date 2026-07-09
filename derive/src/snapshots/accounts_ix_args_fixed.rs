#[derive(Copy, Clone)]
pub struct IxArgsFixedBumps;
impl IxArgsFixed {}
impl quasar_lang::traits::AccountBumps for IxArgsFixed {
    type Bumps = IxArgsFixedBumps;
}
impl quasar_lang::traits::AccountGroup for IxArgsFixed {}
impl<'input> ParseAccounts<'input> for IxArgsFixed {
    type Bumps = IxArgsFixedBumps;
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
unsafe impl<'input> quasar_lang::traits::ParseAccountsUnchecked<'input> for IxArgsFixed {
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
        let (amount, flag) = Self::__extract_ix_args(__ix_data)?;
        let [account] = accounts else { unsafe { core::hint::unreachable_unchecked() } };
        let mut account = <Account<
            SimpleAccount,
        > as quasar_lang::account_load::AccountLoad>::load_mut(account)?;
        quasar_lang::validation::check_constraint(
            amount > 0 && flag,
            QuasarError::ConstraintViolation.into(),
        )?;
        Ok((Self { account }, IxArgsFixedBumps))
    }
}
impl AccountCount for IxArgsFixed {
    const COUNT: usize = 1usize;
    const NEEDS_EVENT_CPI: bool = false;
}
impl IxArgsFixed {
    #[inline(always)]
    #[allow(unused_variables)]
    fn __extract_ix_args<'a>(__ix_data: &'a [u8]) -> Result<(u64, bool), ProgramError> {
        #[repr(C)]
        struct __IxArgsZc {
            amount: <u64 as quasar_lang::instruction_arg::InstructionArg>::Zc,
            flag: <bool as quasar_lang::instruction_arg::InstructionArg>::Zc,
        }
        const _: () = assert!(
            core::mem::align_of:: < __IxArgsZc > () == 1,
            "instruction args ZC struct must have alignment 1"
        );
        if __ix_data.len() < core::mem::size_of::<__IxArgsZc>() {
            return Err(ProgramError::InvalidInstructionData);
        }
        let __ix_zc = unsafe { &*(__ix_data.as_ptr() as *const __IxArgsZc) };
        <u64 as quasar_lang::instruction_arg::InstructionArg>::validate_zc(
            &__ix_zc.amount,
        )?;
        let amount = <u64 as quasar_lang::instruction_arg::InstructionArg>::from_zc(
            &__ix_zc.amount,
        );
        <bool as quasar_lang::instruction_arg::InstructionArg>::validate_zc(
            &__ix_zc.flag,
        )?;
        let flag = <bool as quasar_lang::instruction_arg::InstructionArg>::from_zc(
            &__ix_zc.flag,
        );
        Ok((amount, flag))
    }
    #[inline(always)]
    #[doc(hidden)]
    pub unsafe fn parse_accounts(
        mut input: *mut u8,
        buf: &mut core::mem::MaybeUninit<[quasar_lang::__internal::AccountView; 1usize]>,
        __program_id: &quasar_lang::prelude::Address,
    ) -> Result<*mut u8, ProgramError> {
        let base = buf.as_mut_ptr() as *mut quasar_lang::__internal::AccountView;
        {
            const __EXPECTED: u32 = quasar_lang::__internal::header_expected(
                <Account<
                    SimpleAccount,
                > as quasar_lang::account_load::AccountLoad>::IS_SIGNER,
                true,
                <Account<
                    SimpleAccount,
                > as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE,
            );
            const __MASK: u32 = quasar_lang::__internal::header_mask(
                <Account<
                    SimpleAccount,
                > as quasar_lang::account_load::AccountLoad>::IS_SIGNER,
                true,
                <Account<
                    SimpleAccount,
                > as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE,
            );
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
                concat!("Account '", stringify!(account), "' (index ", "0usize",
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
    ) -> Result<(Self, IxArgsFixedBumps), ProgramError> {
        let (amount, flag) = Self::__extract_ix_args(__ix_data)?;
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
            let [account] = accounts else {
                unsafe { core::hint::unreachable_unchecked() }
            };
            let mut account = <Account<
                SimpleAccount,
            > as quasar_lang::account_load::AccountLoad>::load_mut(account)?;
            quasar_lang::validation::check_constraint(
                amount > 0 && flag,
                QuasarError::ConstraintViolation.into(),
            )?;
            Ok((Self { account }, IxArgsFixedBumps))
        };
        let (__parsed_accounts, __parsed_bumps) = __parsed_result?;
        Ok((__parsed_accounts, __parsed_bumps))
    }
}
unsafe impl quasar_lang::traits::ParseAccountsRaw for IxArgsFixed {
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
impl<'input> quasar_lang::remaining::RemainingItem<'input> for IxArgsFixed {
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
macro_rules! __ix_args_fixed_instruction {
    ($struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* }) => {
        pub struct $struct_name { pub account : quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* } impl From < $struct_name > for
        quasar_lang::client::Instruction { fn from(ix : $struct_name) ->
        quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new(ix.account, false),]; let
        data = { let mut _data = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& <
        $arg_ty as quasar_lang::client::SerializeArg > ::serialize_arg(& ix.
        $arg_name));)* _data }; quasar_lang::client::Instruction { program_id :
        $crate::ID, accounts, data, } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        compact
    ) => {
        pub struct $struct_name { pub account : quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* } impl From < $struct_name > for
        quasar_lang::client::Instruction { fn from(ix : $struct_name) ->
        quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new(ix.account, false),]; let
        data = { let mut _data = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& <
        $arg_ty as quasar_lang::client::CompactSerializeArg > ::compact_header(& ix.
        $arg_name));)* $(_data.extend_from_slice(& < $arg_ty as
        quasar_lang::client::CompactSerializeArg > ::compact_tail(& ix. $arg_name));)*
        _data }; quasar_lang::client::Instruction { program_id : $crate::ID, accounts,
        data, } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        remaining
    ) => {
        pub struct $struct_name { pub account : quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* pub remaining_accounts : ::alloc::vec::Vec <
        quasar_lang::client::AccountMeta >, } impl From < $struct_name > for
        quasar_lang::client::Instruction { fn from(ix : $struct_name) ->
        quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new(ix.account, false),];
        accounts.extend(ix.remaining_accounts); let data = { let mut _data =
        ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
        quasar_lang::client::SerializeArg > ::serialize_arg(& ix. $arg_name));)* _data };
        quasar_lang::client::Instruction { program_id : $crate::ID, accounts, data, } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        compact, remaining
    ) => {
        pub struct $struct_name { pub account : quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* pub remaining_accounts : ::alloc::vec::Vec <
        quasar_lang::client::AccountMeta >, } impl From < $struct_name > for
        quasar_lang::client::Instruction { fn from(ix : $struct_name) ->
        quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new(ix.account, false),];
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
    (quasar_lang::idl_build::s("IxArgsFixed"),
    quasar_lang::idl_build::vec![quasar_lang::idl_build::__reexport::IdlAccountNode {
    name : quasar_lang::idl_build::s("account"), optional : false, writable :
    quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), signer :
    quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), resolver :
    quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    quasar_lang::idl_build::Vec::new(), }],) })
}
