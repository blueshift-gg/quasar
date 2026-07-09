#[derive(Copy, Clone)]
pub struct HeaderDupReadonlyBumps;
impl HeaderDupReadonly {}
impl quasar_lang::traits::AccountBumps for HeaderDupReadonly {
    type Bumps = HeaderDupReadonlyBumps;
}
impl quasar_lang::traits::AccountGroup for HeaderDupReadonly {}
impl<'input> ParseAccounts<'input> for HeaderDupReadonly {
    type Bumps = HeaderDupReadonlyBumps;
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
for HeaderDupReadonly {
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
        let [source, destination] = accounts else {
            unsafe { core::hint::unreachable_unchecked() }
        };
        let source = <Signer as quasar_lang::account_load::AccountLoad>::load(source)?;
        let destination = <UncheckedAccount as quasar_lang::account_load::AccountLoad>::load_checked(
            destination,
        )?;
        Ok((Self { source, destination }, HeaderDupReadonlyBumps))
    }
}
impl AccountCount for HeaderDupReadonly {
    const COUNT: usize = 2usize;
    const NEEDS_EVENT_CPI: bool = false;
}
impl HeaderDupReadonly {
    #[inline(always)]
    #[doc(hidden)]
    pub unsafe fn parse_accounts(
        mut input: *mut u8,
        buf: &mut core::mem::MaybeUninit<[quasar_lang::__internal::AccountView; 2usize]>,
        __program_id: &quasar_lang::prelude::Address,
    ) -> Result<*mut u8, ProgramError> {
        let base = buf.as_mut_ptr() as *mut quasar_lang::__internal::AccountView;
        {
            const __EXPECTED: u32 = quasar_lang::__internal::header_expected(
                <Signer as quasar_lang::account_load::AccountLoad>::IS_SIGNER,
                false,
                <Signer as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE,
            );
            const __MASK: u32 = quasar_lang::__internal::header_mask(
                <Signer as quasar_lang::account_load::AccountLoad>::IS_SIGNER,
                false,
                <Signer as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE,
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
                concat!("Account '", stringify!(source), "' (index ", "0usize",
                "): validation passed")
            );
        }
        {
            const __EXPECTED: u32 = quasar_lang::__internal::header_expected(
                <UncheckedAccount as quasar_lang::account_load::AccountLoad>::IS_SIGNER,
                false,
                <UncheckedAccount as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE,
            );
            const __MASK: u32 = quasar_lang::__internal::header_mask(
                <UncheckedAccount as quasar_lang::account_load::AccountLoad>::IS_SIGNER,
                false,
                <UncheckedAccount as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE,
            );
            const __FLAG_MASK: u32 = quasar_lang::__internal::header_flag_mask(
                <UncheckedAccount as quasar_lang::account_load::AccountLoad>::IS_SIGNER,
                false,
                <UncheckedAccount as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE,
            );
            input = unsafe {
                quasar_lang::__internal::parse_account_dup(
                    input,
                    base,
                    1usize,
                    __program_id,
                    quasar_lang::__internal::ParseFlags {
                        expected: __EXPECTED,
                        mask: __MASK,
                        flag_mask: __FLAG_MASK,
                        is_optional: false,
                        is_ref_mut: false,
                        allow_dup: true,
                    },
                )?
            };
            quasar_lang::debug_log!(
                concat!("Account '", stringify!(destination), "' (index ", "1usize",
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
    ) -> Result<(Self, HeaderDupReadonlyBumps), ProgramError> {
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
            let [source, destination] = accounts else {
                unsafe { core::hint::unreachable_unchecked() }
            };
            let source = <Signer as quasar_lang::account_load::AccountLoad>::load(
                source,
            )?;
            let destination = <UncheckedAccount as quasar_lang::account_load::AccountLoad>::load_checked(
                destination,
            )?;
            Ok((Self { source, destination }, HeaderDupReadonlyBumps))
        };
        let (__parsed_accounts, __parsed_bumps) = __parsed_result?;
        Ok((__parsed_accounts, __parsed_bumps))
    }
}
unsafe impl quasar_lang::traits::ParseAccountsRaw for HeaderDupReadonly {
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
impl<'input> quasar_lang::remaining::RemainingItem<'input> for HeaderDupReadonly {
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
macro_rules! __header_dup_readonly_instruction {
    ($struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* }) => {
        pub struct $struct_name { pub source : quasar_lang::prelude::Address, pub
        destination : quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* } impl
        From < $struct_name > for quasar_lang::client::Instruction { fn from(ix :
        $struct_name) -> quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new_readonly(ix.source, true),
        quasar_lang::client::AccountMeta::new_readonly(ix.destination, false),]; let data
        = { let mut _data = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& <
        $arg_ty as quasar_lang::client::SerializeArg > ::serialize_arg(& ix.
        $arg_name));)* _data }; quasar_lang::client::Instruction { program_id :
        $crate::ID, accounts, data, } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        compact
    ) => {
        pub struct $struct_name { pub source : quasar_lang::prelude::Address, pub
        destination : quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* } impl
        From < $struct_name > for quasar_lang::client::Instruction { fn from(ix :
        $struct_name) -> quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new_readonly(ix.source, true),
        quasar_lang::client::AccountMeta::new_readonly(ix.destination, false),]; let data
        = { let mut _data = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& <
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
        pub struct $struct_name { pub source : quasar_lang::prelude::Address, pub
        destination : quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* pub
        remaining_accounts : ::alloc::vec::Vec < quasar_lang::client::AccountMeta >, }
        impl From < $struct_name > for quasar_lang::client::Instruction { fn from(ix :
        $struct_name) -> quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new_readonly(ix.source, true),
        quasar_lang::client::AccountMeta::new_readonly(ix.destination, false),]; accounts
        .extend(ix.remaining_accounts); let data = { let mut _data =
        ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
        quasar_lang::client::SerializeArg > ::serialize_arg(& ix. $arg_name));)* _data };
        quasar_lang::client::Instruction { program_id : $crate::ID, accounts, data, } } }
    };
    (
        $struct_name:ident, [$($disc:expr),*], { $($arg_name:ident : $arg_ty:ty),* },
        compact, remaining
    ) => {
        pub struct $struct_name { pub source : quasar_lang::prelude::Address, pub
        destination : quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* pub
        remaining_accounts : ::alloc::vec::Vec < quasar_lang::client::AccountMeta >, }
        impl From < $struct_name > for quasar_lang::client::Instruction { fn from(ix :
        $struct_name) -> quasar_lang::client::Instruction { let mut accounts =
        ::alloc::vec![quasar_lang::client::AccountMeta::new_readonly(ix.source, true),
        quasar_lang::client::AccountMeta::new_readonly(ix.destination, false),]; accounts
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
    (quasar_lang::idl_build::s("HeaderDupReadonly"),
    quasar_lang::idl_build::vec![quasar_lang::idl_build::__reexport::IdlAccountNode {
    name : quasar_lang::idl_build::s("source"), optional : false, writable :
    quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), signer :
    quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), resolver :
    quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    quasar_lang::idl_build::Vec::new(), },
    quasar_lang::idl_build::__reexport::IdlAccountNode { name :
    quasar_lang::idl_build::s("destination"), optional : false, writable :
    quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), signer :
    quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), resolver :
    quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    quasar_lang::idl_build::vec![quasar_lang::idl_build::s("CHECK: test-only unchecked account used to validate duplicate readonly aliases.")],
    }],) })
}
