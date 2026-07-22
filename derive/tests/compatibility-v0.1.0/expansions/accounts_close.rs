#[derive(Copy, Clone)]
pub struct CloseAccountsBumps;
impl CloseAccounts {}
impl ::quasar_lang::traits::AccountBumps for CloseAccounts {
    type Bumps = CloseAccountsBumps;
}
impl ::quasar_lang::traits::AccountGroup for CloseAccounts {}
impl<'input> ::quasar_lang::traits::ParseAccounts<'input> for CloseAccounts {
    type Bumps = CloseAccountsBumps;
    const HAS_EPILOGUE: bool = false || true;
    #[inline(always)]
    fn parse(
        accounts: &'input mut [::quasar_lang::__internal::AccountView],
        program_id: &::quasar_lang::prelude::Address,
    ) -> Result<
        (Self, Self::Bumps),
        ::quasar_lang::__solana_program_error::ProgramError,
    > {
        ::quasar_lang::traits::check_account_count(accounts.len(), Self::COUNT)?;
        unsafe {
            <Self as ::quasar_lang::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
                accounts,
                &[],
                program_id,
            )
        }
    }
    #[inline(always)]
    fn parse_with_instruction_data(
        accounts: &'input mut [::quasar_lang::__internal::AccountView],
        __ix_data: &[u8],
        __program_id: &::quasar_lang::prelude::Address,
    ) -> Result<
        (Self, Self::Bumps),
        ::quasar_lang::__solana_program_error::ProgramError,
    > {
        ::quasar_lang::traits::check_account_count(accounts.len(), Self::COUNT)?;
        unsafe {
            <Self as ::quasar_lang::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
                accounts,
                __ix_data,
                __program_id,
            )
        }
    }
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
    #[inline(always)]
    #[allow(unused_variables)]
    fn epilogue_with_context(
        &mut self,
        __bumps: &Self::Bumps,
        __ix_data: &[u8],
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
    unsafe fn parse_unchecked(
        accounts: &'input mut [::quasar_lang::__internal::AccountView],
        program_id: &::quasar_lang::prelude::Address,
    ) -> Result<
        (Self, Self::Bumps),
        ::quasar_lang::__solana_program_error::ProgramError,
    > {
        <Self as ::quasar_lang::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
            accounts,
            &[],
            program_id,
        )
    }
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
impl CloseAccounts {
    #[inline(always)]
    #[doc(hidden)]
    pub unsafe fn parse_accounts(
        mut input: *mut u8,
        buf: &mut core::mem::MaybeUninit<
            [::quasar_lang::__internal::AccountView; 2usize],
        >,
        __program_id: &::quasar_lang::prelude::Address,
    ) -> Result<*mut u8, ::quasar_lang::__solana_program_error::ProgramError> {
        let base = buf.as_mut_ptr() as *mut ::quasar_lang::__internal::AccountView;
        {
            const __EXPECTED: u32 = ::quasar_lang::__internal::header_expected(
                <Signer as ::quasar_lang::account_load::AccountLoad>::IS_SIGNER,
                true,
                <Signer as ::quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE,
            );
            const __MASK: u32 = ::quasar_lang::__internal::header_mask(
                <Signer as ::quasar_lang::account_load::AccountLoad>::IS_SIGNER,
                true,
                <Signer as ::quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE,
            );
            input = unsafe {
                ::quasar_lang::__internal::parse_account(
                    input,
                    base,
                    0usize,
                    __EXPECTED,
                    __MASK,
                )?
            };
            ::quasar_lang::debug_log!(
                concat!("Account '", stringify!(authority), "' (index ", "0usize",
                "): validation passed")
            );
        }
        {
            const __EXPECTED: u32 = ::quasar_lang::__internal::header_expected(
                <Account<
                    OldData,
                > as ::quasar_lang::account_load::AccountLoad>::IS_SIGNER,
                true,
                <Account<
                    OldData,
                > as ::quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE,
            );
            const __MASK: u32 = ::quasar_lang::__internal::header_mask(
                <Account<
                    OldData,
                > as ::quasar_lang::account_load::AccountLoad>::IS_SIGNER,
                true,
                <Account<
                    OldData,
                > as ::quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE,
            );
            input = unsafe {
                ::quasar_lang::__internal::parse_account(
                    input,
                    base,
                    1usize,
                    __EXPECTED,
                    __MASK,
                )?
            };
            ::quasar_lang::debug_log!(
                concat!("Account '", stringify!(old_data), "' (index ", "1usize",
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
        __program_id: &::quasar_lang::prelude::Address,
    ) -> Result<
        (Self, CloseAccountsBumps),
        ::quasar_lang::__solana_program_error::ProgramError,
    > {
        let mut __buf = core::mem::MaybeUninit::<
            [::quasar_lang::__internal::AccountView; 2usize],
        >::uninit();
        let _ = Self::parse_accounts(input, &mut __buf, __program_id)?;
        let mut __accounts = unsafe { __buf.assume_init() };
        let accounts = &mut __accounts;
        let __parsed_result: Result<
            (Self, <Self as ::quasar_lang::traits::ParseAccounts>::Bumps),
            ::quasar_lang::__solana_program_error::ProgramError,
        > = {
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
        };
        let (__parsed_accounts, __parsed_bumps) = __parsed_result?;
        Ok((__parsed_accounts, __parsed_bumps))
    }
}
unsafe impl ::quasar_lang::traits::ParseAccountsRaw for CloseAccounts {
    #[inline(always)]
    unsafe fn parse_accounts_raw(
        input: *mut u8,
        base: *mut ::quasar_lang::__internal::AccountView,
        offset: usize,
        __program_id: &::quasar_lang::prelude::Address,
    ) -> Result<*mut u8, ::quasar_lang::__solana_program_error::ProgramError> {
        let mut __inner_buf = core::mem::MaybeUninit::<
            [::quasar_lang::__internal::AccountView; 2usize],
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
impl<'input> ::quasar_lang::remaining::RemainingItem<'input> for CloseAccounts {
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
mod __close_accounts_client_macro {
    #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
    #[macro_export]
    macro_rules! __close_accounts_instruction {
        (
            $struct_name:ident, $raw_struct_name:ident, [$($disc:expr),*], {
            $($arg_name:ident : $arg_ty:ty),* }
        ) => {
            pub struct $struct_name { pub authority : ::quasar_lang::prelude::Address,
            pub old_data : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)*
            } impl From < $struct_name > for ::quasar_lang::client::Instruction {
            #[allow(unused_variables)] fn from(ix : $struct_name) ->
            ::quasar_lang::client::Instruction { let accounts =
            ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.authority, true),
            ::quasar_lang::client::AccountMeta::new(ix.old_data, false),]; let data = {
            let mut _data = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& <
            $arg_ty as ::quasar_lang::client::SerializeArg > ::serialize_arg(& ix.
            $arg_name));)* _data }; ::quasar_lang::client::Instruction { program_id :
            $crate::ID, accounts, data, } } }
        };
        (
            $struct_name:ident, $raw_struct_name:ident, [$($disc:expr),*], {
            $($arg_name:ident : $arg_ty:ty),* }, compact
        ) => {
            pub struct $struct_name { pub authority : ::quasar_lang::prelude::Address,
            pub old_data : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)*
            } impl From < $struct_name > for ::quasar_lang::client::Instruction {
            #[allow(unused_variables)] fn from(ix : $struct_name) ->
            ::quasar_lang::client::Instruction { let accounts =
            ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.authority, true),
            ::quasar_lang::client::AccountMeta::new(ix.old_data, false),]; let data = {
            let mut _data = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& <
            $arg_ty as ::quasar_lang::client::CompactSerializeArg > ::compact_header(& ix
            . $arg_name));)* $(_data.extend_from_slice(& < $arg_ty as
            ::quasar_lang::client::CompactSerializeArg > ::compact_tail(& ix.
            $arg_name));)* _data }; ::quasar_lang::client::Instruction { program_id :
            $crate::ID, accounts, data, } } }
        };
        (
            $struct_name:ident, $raw_struct_name:ident, [$($disc:expr),*], {
            $($arg_name:ident : $arg_ty:ty),* }, remaining
        ) => {
            pub struct $struct_name { pub authority : ::quasar_lang::prelude::Address,
            pub old_data : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)*
            pub remaining_accounts : ::alloc::vec::Vec <
            ::quasar_lang::client::AccountMeta >, } impl From < $struct_name > for
            ::quasar_lang::client::Instruction { #[allow(unused_variables)] fn from(ix :
            $struct_name) -> ::quasar_lang::client::Instruction { let mut accounts =
            ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.authority, true),
            ::quasar_lang::client::AccountMeta::new(ix.old_data, false),]; accounts
            .extend(ix.remaining_accounts); let data = { let mut _data =
            ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
            ::quasar_lang::client::SerializeArg > ::serialize_arg(& ix. $arg_name));)*
            _data }; ::quasar_lang::client::Instruction { program_id : $crate::ID,
            accounts, data, } } }
        };
        (
            $struct_name:ident, $raw_struct_name:ident, [$($disc:expr),*], {
            $($arg_name:ident : $arg_ty:ty),* }, compact, remaining
        ) => {
            pub struct $struct_name { pub authority : ::quasar_lang::prelude::Address,
            pub old_data : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)*
            pub remaining_accounts : ::alloc::vec::Vec <
            ::quasar_lang::client::AccountMeta >, } impl From < $struct_name > for
            ::quasar_lang::client::Instruction { #[allow(unused_variables)] fn from(ix :
            $struct_name) -> ::quasar_lang::client::Instruction { let mut accounts =
            ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.authority, true),
            ::quasar_lang::client::AccountMeta::new(ix.old_data, false),]; accounts
            .extend(ix.remaining_accounts); let data = { let mut _data =
            ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
            ::quasar_lang::client::CompactSerializeArg > ::compact_header(& ix.
            $arg_name));)* $(_data.extend_from_slice(& < $arg_ty as
            ::quasar_lang::client::CompactSerializeArg > ::compact_tail(& ix.
            $arg_name));)* _data }; ::quasar_lang::client::Instruction { program_id :
            $crate::ID, accounts, data, } } }
        };
    }
}
#[cfg(feature = "idl-build")]
::quasar_lang::__private_inventory::submit! {
    ::quasar_lang::idl_build::AccountsMetaFragment(|| {
    (::quasar_lang::idl_build::s("CloseAccounts"),
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::__reexport::IdlAccountNode {
    name : ::quasar_lang::idl_build::s("authority"), optional : false, writable :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), signer :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), resolver :
    ::quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    ::quasar_lang::idl_build::Vec::new(), },
    ::quasar_lang::idl_build::__reexport::IdlAccountNode { name :
    ::quasar_lang::idl_build::s("oldData"), optional : false, writable :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), signer :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), resolver :
    ::quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    ::quasar_lang::idl_build::Vec::new(), }],) })
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
