#[derive(Copy, Clone)]
pub struct InitEscrowBumps {
    pub escrow: u8,
}
impl InitEscrow {
    #[inline(always)]
    pub fn escrow_signer<'__quasar_seed>(
        &'__quasar_seed self,
        bumps: &'__quasar_seed InitEscrowBumps,
    ) -> <Escrow as ::quasar_lang::traits::HasSeeds>::WithBump<'__quasar_seed> {
        let payer = &self.payer;
        Escrow::seeds(payer.address()).with_bump(bumps.escrow)
    }
}
impl ::quasar_lang::traits::AccountBumps for InitEscrow {
    type Bumps = InitEscrowBumps;
}
impl<'input> ::quasar_lang::traits::ParseAccounts<'input> for InitEscrow {
    type Bumps = InitEscrowBumps;
}
unsafe impl<'input> ::quasar_lang::traits::ParseAccountsUnchecked<'input>
for InitEscrow {
    #[inline(always)]
    unsafe fn parse_with_instruction_data_unchecked(
        accounts: &'input mut [::quasar_lang::__internal::AccountView],
        __ix_data: &[u8],
        __program_id: &::quasar_lang::prelude::Address,
    ) -> Result<
        (Self, Self::Bumps),
        ::quasar_lang::__solana_program_error::ProgramError,
    > {
        let [payer, escrow, system_program] = accounts else {
            unsafe { core::hint::unreachable_unchecked() }
        };
        const _: () = assert!(
            < Account < Escrow > as ::quasar_lang::account_init::AccountInit >
            ::DEFAULT_INIT_PARAMS_VALID,
            "field `escrow` requires an init-param behavior (e.g., token(...) or mint(...))",
        );
        let __bumps_escrow: u8;
        let __rent_ctx = ::quasar_lang::ops::OpCtx::new(
            unsafe { &*(__program_id as *const ::quasar_lang::prelude::Address) },
            ::quasar_lang::ops::RentResolver::fetch_once(),
        );
        let mut payer = <Signer as ::quasar_lang::account_load::AccountLoad>::load_mut(
            payer,
        )?;
        let system_program = <Program<
            SystemProgram,
        > as ::quasar_lang::account_load::AccountLoad>::load(system_program)?;
        let __addr_escrow = Escrow::seeds(payer.address());
        __bumps_escrow = ::quasar_lang::address::AddressVerify::verify(
            &__addr_escrow,
            escrow.address(),
            __program_id,
        )?;
        {
            let __bump_ref: &[u8] = &[__bumps_escrow];
            ::quasar_lang::address::AddressVerify::with_signer_seeds(
                &__addr_escrow,
                __bump_ref,
                |__signers| -> Result<(), ::quasar_lang::prelude::ProgramError> {
                    let __init_params = ();
                    let __init_op = ::quasar_lang::ops::init::Op {
                        payer: payer.to_account_view(),
                        space: <Account<Escrow> as ::quasar_lang::traits::Space>::SPACE
                            as u64,
                        signers: __signers,
                        params: __init_params,
                        idempotent: false,
                    };
                    __init_op.apply::<Account<Escrow>, _>(escrow, &__rent_ctx)?;
                    Ok(())
                },
            )?;
        }
        let mut escrow = <Account<
            Escrow,
        > as ::quasar_lang::account_load::AccountLoad>::load_mut(escrow)?;
        Ok((
            Self {
                payer,
                escrow,
                system_program,
            },
            InitEscrowBumps {
                escrow: __bumps_escrow,
            },
        ))
    }
}
impl ::quasar_lang::traits::AccountCount for InitEscrow {
    const COUNT: usize = 3usize;
    const NEEDS_EVENT_CPI: bool = false;
}
unsafe impl ::quasar_lang::traits::ParseAccountsRaw for InitEscrow {
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
        ::quasar_lang::debug_log!("account payer @0: validation passed");
        input = unsafe {
            ::quasar_lang::__internal::parse_account::<
                Account<Escrow>,
                true,
            >(input, base, __offset + 1usize)?
        };
        ::quasar_lang::debug_log!("account escrow @1: validation passed");
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
impl<'input> ::quasar_lang::remaining::RemainingItem<'input> for InitEscrow {
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
mod __init_escrow_client_macro {
    #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
    #[macro_export]
    macro_rules! __init_escrow_instruction {
        (
            $struct_name:ident, $raw_struct_name:ident, [$($disc:expr),*], {
            $($arg_name:ident : $arg_ty:ty),* }
        ) => {
            pub struct $struct_name { pub payer : ::quasar_lang::prelude::Address, $(pub
            $arg_name : $arg_ty,)* } impl $struct_name { #[doc =
            r" The address this builder derives for this account, using"] #[doc =
            r" the same PDA/ATA recipe the instruction uses — so callers"] #[doc =
            r" can name it without re-deriving the seeds by hand."] pub fn
            escrow_address(& self) -> ::quasar_lang::prelude::Address {
            super::InitEscrow::__quasar_pda_escrow(& self.payer, & $crate::ID) } } #[doc
            = r" Explicit account-address builder for adversarial and negative tests."]
            pub struct $raw_struct_name { pub payer : ::quasar_lang::prelude::Address,
            pub escrow : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* }
            impl From < $struct_name > for $raw_struct_name { #[allow(unused_variables)]
            fn from(ix : $struct_name) -> Self { Self { payer : ix.payer, escrow :
            super::InitEscrow::__quasar_pda_escrow(& ix.payer, & $crate::ID), $($arg_name
            : ix. $arg_name,)* } } } impl From < $struct_name > for
            ::quasar_lang::client::Instruction { fn from(ix : $struct_name) ->
            ::quasar_lang::client::Instruction { $raw_struct_name ::from(ix).into() } }
            impl From < $raw_struct_name > for ::quasar_lang::client::Instruction {
            #[allow(unused_variables)] fn from(ix : $raw_struct_name) ->
            ::quasar_lang::client::Instruction { let accounts =
            ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.payer, true),
            ::quasar_lang::client::AccountMeta::new(ix.escrow, false),
            ::quasar_lang::client::AccountMeta::new_readonly(super::InitEscrow::__QUASAR_FIXED_ADDRESS_system_program,
            false),]; let data = { let mut _data = ::alloc::vec![$($disc),*]; $(_data
            .extend_from_slice(& < $arg_ty as ::quasar_lang::client::SerializeArg >
            ::serialize_arg(& ix. $arg_name));)* _data };
            ::quasar_lang::client::Instruction { program_id : $crate::ID, accounts, data,
            } } }
        };
        (
            $struct_name:ident, $raw_struct_name:ident, [$($disc:expr),*], {
            $($arg_name:ident : $arg_ty:ty),* }, compact
        ) => {
            pub struct $struct_name { pub payer : ::quasar_lang::prelude::Address, $(pub
            $arg_name : $arg_ty,)* } impl $struct_name { #[doc =
            r" The address this builder derives for this account, using"] #[doc =
            r" the same PDA/ATA recipe the instruction uses — so callers"] #[doc =
            r" can name it without re-deriving the seeds by hand."] pub fn
            escrow_address(& self) -> ::quasar_lang::prelude::Address {
            super::InitEscrow::__quasar_pda_escrow(& self.payer, & $crate::ID) } } #[doc
            = r" Explicit account-address builder for adversarial and negative tests."]
            pub struct $raw_struct_name { pub payer : ::quasar_lang::prelude::Address,
            pub escrow : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* }
            impl From < $struct_name > for $raw_struct_name { #[allow(unused_variables)]
            fn from(ix : $struct_name) -> Self { Self { payer : ix.payer, escrow :
            super::InitEscrow::__quasar_pda_escrow(& ix.payer, & $crate::ID), $($arg_name
            : ix. $arg_name,)* } } } impl From < $struct_name > for
            ::quasar_lang::client::Instruction { fn from(ix : $struct_name) ->
            ::quasar_lang::client::Instruction { $raw_struct_name ::from(ix).into() } }
            impl From < $raw_struct_name > for ::quasar_lang::client::Instruction {
            #[allow(unused_variables)] fn from(ix : $raw_struct_name) ->
            ::quasar_lang::client::Instruction { let accounts =
            ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.payer, true),
            ::quasar_lang::client::AccountMeta::new(ix.escrow, false),
            ::quasar_lang::client::AccountMeta::new_readonly(super::InitEscrow::__QUASAR_FIXED_ADDRESS_system_program,
            false),]; let data = { let mut _data = ::alloc::vec![$($disc),*]; $(_data
            .extend_from_slice(& < $arg_ty as ::quasar_lang::client::CompactSerializeArg
            > ::compact_header(& ix. $arg_name));)* $(_data.extend_from_slice(& < $arg_ty
            as ::quasar_lang::client::CompactSerializeArg > ::compact_tail(& ix.
            $arg_name));)* _data }; ::quasar_lang::client::Instruction { program_id :
            $crate::ID, accounts, data, } } }
        };
        (
            $struct_name:ident, $raw_struct_name:ident, [$($disc:expr),*], {
            $($arg_name:ident : $arg_ty:ty),* }, remaining
        ) => {
            pub struct $struct_name { pub payer : ::quasar_lang::prelude::Address, $(pub
            $arg_name : $arg_ty,)* pub remaining_accounts : ::alloc::vec::Vec <
            ::quasar_lang::client::AccountMeta >, } impl $struct_name { #[doc =
            r" The address this builder derives for this account, using"] #[doc =
            r" the same PDA/ATA recipe the instruction uses — so callers"] #[doc =
            r" can name it without re-deriving the seeds by hand."] pub fn
            escrow_address(& self) -> ::quasar_lang::prelude::Address {
            super::InitEscrow::__quasar_pda_escrow(& self.payer, & $crate::ID) } } #[doc
            = r" Explicit account-address builder for adversarial and negative tests."]
            pub struct $raw_struct_name { pub payer : ::quasar_lang::prelude::Address,
            pub escrow : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)*
            pub remaining_accounts : ::alloc::vec::Vec <
            ::quasar_lang::client::AccountMeta >, } impl From < $struct_name > for
            $raw_struct_name { #[allow(unused_variables)] fn from(ix : $struct_name) ->
            Self { Self { payer : ix.payer, escrow :
            super::InitEscrow::__quasar_pda_escrow(& ix.payer, & $crate::ID), $($arg_name
            : ix. $arg_name,)* remaining_accounts : ix.remaining_accounts, } } } impl
            From < $struct_name > for ::quasar_lang::client::Instruction { fn from(ix :
            $struct_name) -> ::quasar_lang::client::Instruction { $raw_struct_name
            ::from(ix).into() } } impl From < $raw_struct_name > for
            ::quasar_lang::client::Instruction { #[allow(unused_variables)] fn from(ix :
            $raw_struct_name) -> ::quasar_lang::client::Instruction { let mut accounts =
            ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.payer, true),
            ::quasar_lang::client::AccountMeta::new(ix.escrow, false),
            ::quasar_lang::client::AccountMeta::new_readonly(super::InitEscrow::__QUASAR_FIXED_ADDRESS_system_program,
            false),]; accounts.extend(ix.remaining_accounts); let data = { let mut _data
            = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
            ::quasar_lang::client::SerializeArg > ::serialize_arg(& ix. $arg_name));)*
            _data }; ::quasar_lang::client::Instruction { program_id : $crate::ID,
            accounts, data, } } }
        };
        (
            $struct_name:ident, $raw_struct_name:ident, [$($disc:expr),*], {
            $($arg_name:ident : $arg_ty:ty),* }, compact, remaining
        ) => {
            pub struct $struct_name { pub payer : ::quasar_lang::prelude::Address, $(pub
            $arg_name : $arg_ty,)* pub remaining_accounts : ::alloc::vec::Vec <
            ::quasar_lang::client::AccountMeta >, } impl $struct_name { #[doc =
            r" The address this builder derives for this account, using"] #[doc =
            r" the same PDA/ATA recipe the instruction uses — so callers"] #[doc =
            r" can name it without re-deriving the seeds by hand."] pub fn
            escrow_address(& self) -> ::quasar_lang::prelude::Address {
            super::InitEscrow::__quasar_pda_escrow(& self.payer, & $crate::ID) } } #[doc
            = r" Explicit account-address builder for adversarial and negative tests."]
            pub struct $raw_struct_name { pub payer : ::quasar_lang::prelude::Address,
            pub escrow : ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)*
            pub remaining_accounts : ::alloc::vec::Vec <
            ::quasar_lang::client::AccountMeta >, } impl From < $struct_name > for
            $raw_struct_name { #[allow(unused_variables)] fn from(ix : $struct_name) ->
            Self { Self { payer : ix.payer, escrow :
            super::InitEscrow::__quasar_pda_escrow(& ix.payer, & $crate::ID), $($arg_name
            : ix. $arg_name,)* remaining_accounts : ix.remaining_accounts, } } } impl
            From < $struct_name > for ::quasar_lang::client::Instruction { fn from(ix :
            $struct_name) -> ::quasar_lang::client::Instruction { $raw_struct_name
            ::from(ix).into() } } impl From < $raw_struct_name > for
            ::quasar_lang::client::Instruction { #[allow(unused_variables)] fn from(ix :
            $raw_struct_name) -> ::quasar_lang::client::Instruction { let mut accounts =
            ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.payer, true),
            ::quasar_lang::client::AccountMeta::new(ix.escrow, false),
            ::quasar_lang::client::AccountMeta::new_readonly(super::InitEscrow::__QUASAR_FIXED_ADDRESS_system_program,
            false),]; accounts.extend(ix.remaining_accounts); let data = { let mut _data
            = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
            ::quasar_lang::client::CompactSerializeArg > ::compact_header(& ix.
            $arg_name));)* $(_data.extend_from_slice(& < $arg_ty as
            ::quasar_lang::client::CompactSerializeArg > ::compact_tail(& ix.
            $arg_name));)* _data }; ::quasar_lang::client::Instruction { program_id :
            $crate::ID, accounts, data, } } }
        };
    }
}
#[allow(non_upper_case_globals)]
impl InitEscrow {
    #[doc(hidden)]
    pub const __QUASAR_FIXED_ADDRESS_system_program: ::quasar_lang::prelude::Address = <SystemProgram as ::quasar_lang::traits::Id>::ID;
}
impl InitEscrow {
    #[doc(hidden)]
    #[inline]
    pub fn __quasar_pda_escrow(
        payer: &::quasar_lang::prelude::Address,
        program_id: &::quasar_lang::prelude::Address,
    ) -> ::quasar_lang::prelude::Address {
        let seeds = <Escrow>::seeds(payer);
        ::quasar_lang::pda::find_program_address_const(&seeds.as_slices(), program_id).0
    }
}
#[cfg(feature = "idl-build")]
::quasar_lang::__private_inventory::submit! {
    ::quasar_lang::idl_build::AccountsMetaFragment(|| {
    (::quasar_lang::idl_build::s("InitEscrow"),
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::AccountsMetaEntry::Node(::quasar_lang::idl_build::__reexport::IdlAccountNode
    { name : ::quasar_lang::idl_build::s("payer"), optional : false, writable :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), signer :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), resolver :
    ::quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    ::quasar_lang::idl_build::Vec::new(), }),
    ::quasar_lang::idl_build::AccountsMetaEntry::Node(::quasar_lang::idl_build::__reexport::IdlAccountNode
    { name : ::quasar_lang::idl_build::s("escrow"), optional : false, writable :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), signer :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), resolver :
    ::quasar_lang::idl_build::__reexport::IdlResolver::Pda { program :
    ::quasar_lang::idl_build::__reexport::IdlPdaProgram::ProgramId {}, seeds : { let mut
    seeds = ::quasar_lang::idl_build::Vec::new(); if < Escrow as
    ::quasar_lang::traits::HasSeeds > ::HAS_SEED_PREFIX { seeds
    .push(::quasar_lang::idl_build::__reexport::IdlPdaSeed::Const { value :
    ::quasar_lang::idl_build::Vec::from(< Escrow as ::quasar_lang::traits::HasSeeds >
    ::SEED_PREFIX), }); } seeds
    .push(::quasar_lang::idl_build::__reexport::IdlPdaSeed::Account { path :
    ::quasar_lang::idl_build::s("payer"), }); seeds }, }, docs :
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
    (::quasar_lang::idl_build::s("InitEscrow"),
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
    ::quasar_lang::idl_build::s("escrow"), account_type :
    ::quasar_lang::idl_build::s("Account < Escrow >"), wrapper :
    ::quasar_lang::idl_build::s("Account"), writable : true, signer : false, optional :
    false, allow_duplicate : false, load :
    ::quasar_lang::idl_build::s("Fixed(validates=[])"), pre_load :
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::s("VerifyAddress(expr=`Escrow :: seeds (payer . address ())` error=None)"),
    ::quasar_lang::idl_build::s("Init::Program(payer=payer space_ty=`Account < Escrow >` idempotent=false verified_address=Some(expr=`Escrow :: seeds (payer . address ())` error=None))")],
    post_load : ::quasar_lang::idl_build::vec![], epilogue :
    ::quasar_lang::idl_build::vec![], },
    ::quasar_lang::idl_build::__reexport::IdlAccountValidation { name :
    ::quasar_lang::idl_build::s("systemProgram"), account_type :
    ::quasar_lang::idl_build::s("Program < SystemProgram >"), wrapper :
    ::quasar_lang::idl_build::s("Program"), writable : false, signer : false, optional :
    false, allow_duplicate : false, load :
    ::quasar_lang::idl_build::s("Fixed(validates=[])"), pre_load :
    ::quasar_lang::idl_build::vec![], post_load : ::quasar_lang::idl_build::vec![],
    epilogue : ::quasar_lang::idl_build::vec![], }], },) })
}
