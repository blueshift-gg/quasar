#[derive(Copy, Clone)]
pub struct ConstPdaBumps {
    pub config: u8,
    pub registry: u8,
}
impl ConstPda {
    #[inline(always)]
    pub fn config_signer<'__quasar_seed>(
        &'__quasar_seed self,
        bumps: &'__quasar_seed ConstPdaBumps,
    ) -> <Config as ::quasar_lang::traits::HasSeeds>::WithBump<'__quasar_seed> {
        Config::seeds().with_bump(bumps.config)
    }
    #[inline(always)]
    pub fn registry_signer<'__quasar_seed>(
        &'__quasar_seed self,
        bumps: &'__quasar_seed ConstPdaBumps,
    ) -> <Registry as ::quasar_lang::traits::HasSeeds>::WithBump<'__quasar_seed> {
        Registry::seeds().with_bump(bumps.registry)
    }
}
impl ::quasar_lang::traits::AccountBumps for ConstPda {
    type Bumps = ConstPdaBumps;
}
impl<'input> ::quasar_lang::traits::ParseAccounts<'input> for ConstPda {
    type Bumps = ConstPdaBumps;
}
unsafe impl<'input> ::quasar_lang::traits::ParseAccountsUnchecked<'input> for ConstPda {
    #[inline(always)]
    unsafe fn parse_with_instruction_data_unchecked(
        accounts: &'input mut [::quasar_lang::__internal::AccountView],
        __ix_data: &[u8],
        __program_id: &::quasar_lang::prelude::Address,
    ) -> Result<
        (Self, Self::Bumps),
        ::quasar_lang::__solana_program_error::ProgramError,
    > {
        let [payer, config, registry, system_program] = accounts else {
            unsafe { core::hint::unreachable_unchecked() }
        };
        const _: () = assert!(
            < Account < Config > as ::quasar_lang::account_init::AccountInit >
            ::DEFAULT_INIT_PARAMS_VALID,
            "field `config` requires an init-param behavior (e.g., token(...) or mint(...))",
        );
        let __bumps_config: u8;
        let __bumps_registry: u8;
        let __rent_ctx = ::quasar_lang::ops::OpCtx::new(
            unsafe { &*(__program_id as *const ::quasar_lang::prelude::Address) },
            ::quasar_lang::ops::RentResolver::fetch_once(),
        );
        let mut payer = <Signer as ::quasar_lang::account_load::AccountLoad>::load_mut(
            payer,
        )?;
        let registry = <Account<
            Registry,
        > as ::quasar_lang::account_load::AccountLoad>::load(registry)?;
        let system_program = <Program<
            SystemProgram,
        > as ::quasar_lang::account_load::AccountLoad>::load(system_program)?;
        let __addr_config = Config::seeds();
        __bumps_config = {
            let (__expected_addr, __expected_bump) = const {
                ::quasar_lang::pda::find_program_address_const(
                    &(Config::seeds()).as_slices(),
                    &crate::ID,
                )
            };
            if ::quasar_lang::keys_eq(config.address(), &__expected_addr) {
                Ok(__expected_bump)
            } else {
                Err(
                    ::quasar_lang::prelude::ProgramError::from(
                        ::quasar_lang::error::QuasarError::InvalidPda,
                    ),
                )
            }
        }?;
        {
            let __bump_ref: &[u8] = &[__bumps_config];
            ::quasar_lang::address::AddressVerify::with_signer_seeds(
                &__addr_config,
                __bump_ref,
                |__signers| -> Result<(), ::quasar_lang::prelude::ProgramError> {
                    let __init_params = ();
                    let __init_op = ::quasar_lang::ops::init::Op {
                        payer: payer.to_account_view(),
                        space: <Account<Config> as ::quasar_lang::traits::Space>::SPACE
                            as u64,
                        signers: __signers,
                        params: __init_params,
                        idempotent: false,
                    };
                    __init_op.apply::<Account<Config>, _>(config, &__rent_ctx)?;
                    Ok(())
                },
            )?;
        }
        let mut config = <Account<
            Config,
        > as ::quasar_lang::account_load::AccountLoad>::load_mut(config)?;
        {
            let (__expected_addr, __expected_bump) = const {
                ::quasar_lang::pda::find_program_address_const(
                    &(Registry::seeds()).as_slices(),
                    &crate::ID,
                )
            };
            __bumps_registry = if ::quasar_lang::keys_eq(
                registry.to_account_view().address(),
                &__expected_addr,
            ) {
                Ok(__expected_bump)
            } else {
                Err(
                    ::quasar_lang::prelude::ProgramError::from(
                        ::quasar_lang::error::QuasarError::InvalidPda,
                    ),
                )
            }?;
        }
        Ok((
            Self {
                payer,
                config,
                registry,
                system_program,
            },
            ConstPdaBumps {
                config: __bumps_config,
                registry: __bumps_registry,
            },
        ))
    }
}
impl ::quasar_lang::traits::AccountCount for ConstPda {
    const COUNT: usize = 4usize;
    const NEEDS_EVENT_CPI: bool = false;
}
unsafe impl ::quasar_lang::traits::ParseAccountsRaw for ConstPda {
    #[inline]
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
                Account<Config>,
                true,
            >(input, base, __offset + 1usize)?
        };
        ::quasar_lang::debug_log!("account config @1: validation passed");
        input = unsafe {
            ::quasar_lang::__internal::parse_account::<
                Account<Registry>,
                false,
            >(input, base, __offset + 2usize)?
        };
        ::quasar_lang::debug_log!("account registry @2: validation passed");
        input = unsafe {
            ::quasar_lang::__internal::parse_account::<
                Program<SystemProgram>,
                false,
            >(input, base, __offset + 3usize)?
        };
        ::quasar_lang::debug_log!("account system_program @3: validation passed");
        Ok(input)
    }
}
impl<'input> ::quasar_lang::remaining::RemainingItem<'input> for ConstPda {
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
macro_rules! __const_pda_instruction {
    (
        $struct_name:ident, $raw_name:ident, [$($disc:expr),*], { $($arg_name:ident :
        $arg_ty:ty),* }
    ) => {
        pub struct $struct_name { pub payer : ::quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* } #[doc =
        r" Every account spelled out, including the ones the input builder"] #[doc =
        r" resolves for you. Build it from `$struct_name` and replace an"] #[doc =
        r" address the client would otherwise derive."] pub struct $raw_name { pub payer
        : ::quasar_lang::prelude::Address, pub config : ::quasar_lang::prelude::Address,
        pub registry : ::quasar_lang::prelude::Address, pub system_program :
        ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* } impl From <
        $struct_name > for $raw_name { #[allow(unused_variables)] fn from(ix :
        $struct_name) -> $raw_name { $raw_name { payer : ix.payer, config :
        super::ConstPda::__quasar_pda_config(& $crate::ID), registry :
        super::ConstPda::__quasar_pda_registry(& $crate::ID), system_program :
        super::ConstPda::__QUASAR_FIXED_ADDRESS_system_program, $($arg_name : ix.
        $arg_name,)* } } } impl From < $struct_name > for
        ::quasar_lang::client::Instruction { #[inline] fn from(ix : $struct_name) ->
        ::quasar_lang::client::Instruction { < $raw_name as ::core::convert::Into <
        ::quasar_lang::client::Instruction >> ::into(< $struct_name as
        ::core::convert::Into < $raw_name >> ::into(ix),) } } impl From < $raw_name > for
        ::quasar_lang::client::Instruction { #[allow(unused_variables)] fn from(ix :
        $raw_name) -> ::quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.payer, true),
        ::quasar_lang::client::AccountMeta::new(ix.config, false),
        ::quasar_lang::client::AccountMeta::new_readonly(ix.registry, false),
        ::quasar_lang::client::AccountMeta::new_readonly(ix.system_program, false),]; let
        data = { let mut _data = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& <
        $arg_ty as ::quasar_lang::client::SerializeArg > ::serialize_arg(& ix.
        $arg_name));)* _data }; ::quasar_lang::client::Instruction { program_id :
        $crate::ID, accounts, data, } } }
    };
    (
        $struct_name:ident, $raw_name:ident, [$($disc:expr),*], { $($arg_name:ident :
        $arg_ty:ty),* }, compact
    ) => {
        pub struct $struct_name { pub payer : ::quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* } #[doc =
        r" Every account spelled out, including the ones the input builder"] #[doc =
        r" resolves for you. Build it from `$struct_name` and replace an"] #[doc =
        r" address the client would otherwise derive."] pub struct $raw_name { pub payer
        : ::quasar_lang::prelude::Address, pub config : ::quasar_lang::prelude::Address,
        pub registry : ::quasar_lang::prelude::Address, pub system_program :
        ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* } impl From <
        $struct_name > for $raw_name { #[allow(unused_variables)] fn from(ix :
        $struct_name) -> $raw_name { $raw_name { payer : ix.payer, config :
        super::ConstPda::__quasar_pda_config(& $crate::ID), registry :
        super::ConstPda::__quasar_pda_registry(& $crate::ID), system_program :
        super::ConstPda::__QUASAR_FIXED_ADDRESS_system_program, $($arg_name : ix.
        $arg_name,)* } } } impl From < $struct_name > for
        ::quasar_lang::client::Instruction { #[inline] fn from(ix : $struct_name) ->
        ::quasar_lang::client::Instruction { < $raw_name as ::core::convert::Into <
        ::quasar_lang::client::Instruction >> ::into(< $struct_name as
        ::core::convert::Into < $raw_name >> ::into(ix),) } } impl From < $raw_name > for
        ::quasar_lang::client::Instruction { #[allow(unused_variables)] fn from(ix :
        $raw_name) -> ::quasar_lang::client::Instruction { let accounts =
        ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.payer, true),
        ::quasar_lang::client::AccountMeta::new(ix.config, false),
        ::quasar_lang::client::AccountMeta::new_readonly(ix.registry, false),
        ::quasar_lang::client::AccountMeta::new_readonly(ix.system_program, false),]; let
        data = { let mut _data = ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& <
        $arg_ty as ::quasar_lang::client::CompactSerializeArg > ::compact_header(& ix.
        $arg_name));)* $(_data.extend_from_slice(& < $arg_ty as
        ::quasar_lang::client::CompactSerializeArg > ::compact_tail(& ix. $arg_name));)*
        _data }; ::quasar_lang::client::Instruction { program_id : $crate::ID, accounts,
        data, } } }
    };
    (
        $struct_name:ident, $raw_name:ident, [$($disc:expr),*], { $($arg_name:ident :
        $arg_ty:ty),* }, remaining
    ) => {
        pub struct $struct_name { pub payer : ::quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* pub remaining_accounts : ::alloc::vec::Vec <
        ::quasar_lang::client::AccountMeta >, } #[doc =
        r" Every account spelled out, including the ones the input builder"] #[doc =
        r" resolves for you. Build it from `$struct_name` and replace an"] #[doc =
        r" address the client would otherwise derive."] pub struct $raw_name { pub payer
        : ::quasar_lang::prelude::Address, pub config : ::quasar_lang::prelude::Address,
        pub registry : ::quasar_lang::prelude::Address, pub system_program :
        ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* pub
        remaining_accounts : ::alloc::vec::Vec < ::quasar_lang::client::AccountMeta >, }
        impl From < $struct_name > for $raw_name { #[allow(unused_variables)] fn from(ix
        : $struct_name) -> $raw_name { $raw_name { payer : ix.payer, config :
        super::ConstPda::__quasar_pda_config(& $crate::ID), registry :
        super::ConstPda::__quasar_pda_registry(& $crate::ID), system_program :
        super::ConstPda::__QUASAR_FIXED_ADDRESS_system_program, $($arg_name : ix.
        $arg_name,)* remaining_accounts : ix.remaining_accounts, } } } impl From <
        $struct_name > for ::quasar_lang::client::Instruction { #[inline] fn from(ix :
        $struct_name) -> ::quasar_lang::client::Instruction { < $raw_name as
        ::core::convert::Into < ::quasar_lang::client::Instruction >> ::into(<
        $struct_name as ::core::convert::Into < $raw_name >> ::into(ix),) } } impl From <
        $raw_name > for ::quasar_lang::client::Instruction { #[allow(unused_variables)]
        fn from(ix : $raw_name) -> ::quasar_lang::client::Instruction { let mut accounts
        = ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.payer, true),
        ::quasar_lang::client::AccountMeta::new(ix.config, false),
        ::quasar_lang::client::AccountMeta::new_readonly(ix.registry, false),
        ::quasar_lang::client::AccountMeta::new_readonly(ix.system_program, false),];
        accounts.extend(ix.remaining_accounts); let data = { let mut _data =
        ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
        ::quasar_lang::client::SerializeArg > ::serialize_arg(& ix. $arg_name));)* _data
        }; ::quasar_lang::client::Instruction { program_id : $crate::ID, accounts, data,
        } } }
    };
    (
        $struct_name:ident, $raw_name:ident, [$($disc:expr),*], { $($arg_name:ident :
        $arg_ty:ty),* }, compact, remaining
    ) => {
        pub struct $struct_name { pub payer : ::quasar_lang::prelude::Address, $(pub
        $arg_name : $arg_ty,)* pub remaining_accounts : ::alloc::vec::Vec <
        ::quasar_lang::client::AccountMeta >, } #[doc =
        r" Every account spelled out, including the ones the input builder"] #[doc =
        r" resolves for you. Build it from `$struct_name` and replace an"] #[doc =
        r" address the client would otherwise derive."] pub struct $raw_name { pub payer
        : ::quasar_lang::prelude::Address, pub config : ::quasar_lang::prelude::Address,
        pub registry : ::quasar_lang::prelude::Address, pub system_program :
        ::quasar_lang::prelude::Address, $(pub $arg_name : $arg_ty,)* pub
        remaining_accounts : ::alloc::vec::Vec < ::quasar_lang::client::AccountMeta >, }
        impl From < $struct_name > for $raw_name { #[allow(unused_variables)] fn from(ix
        : $struct_name) -> $raw_name { $raw_name { payer : ix.payer, config :
        super::ConstPda::__quasar_pda_config(& $crate::ID), registry :
        super::ConstPda::__quasar_pda_registry(& $crate::ID), system_program :
        super::ConstPda::__QUASAR_FIXED_ADDRESS_system_program, $($arg_name : ix.
        $arg_name,)* remaining_accounts : ix.remaining_accounts, } } } impl From <
        $struct_name > for ::quasar_lang::client::Instruction { #[inline] fn from(ix :
        $struct_name) -> ::quasar_lang::client::Instruction { < $raw_name as
        ::core::convert::Into < ::quasar_lang::client::Instruction >> ::into(<
        $struct_name as ::core::convert::Into < $raw_name >> ::into(ix),) } } impl From <
        $raw_name > for ::quasar_lang::client::Instruction { #[allow(unused_variables)]
        fn from(ix : $raw_name) -> ::quasar_lang::client::Instruction { let mut accounts
        = ::alloc::vec![::quasar_lang::client::AccountMeta::new(ix.payer, true),
        ::quasar_lang::client::AccountMeta::new(ix.config, false),
        ::quasar_lang::client::AccountMeta::new_readonly(ix.registry, false),
        ::quasar_lang::client::AccountMeta::new_readonly(ix.system_program, false),];
        accounts.extend(ix.remaining_accounts); let data = { let mut _data =
        ::alloc::vec![$($disc),*]; $(_data.extend_from_slice(& < $arg_ty as
        ::quasar_lang::client::CompactSerializeArg > ::compact_header(& ix.
        $arg_name));)* $(_data.extend_from_slice(& < $arg_ty as
        ::quasar_lang::client::CompactSerializeArg > ::compact_tail(& ix. $arg_name));)*
        _data }; ::quasar_lang::client::Instruction { program_id : $crate::ID, accounts,
        data, } } }
    };
}
#[allow(non_upper_case_globals)]
impl ConstPda {
    #[doc(hidden)]
    pub const __QUASAR_FIXED_ADDRESS_system_program: ::quasar_lang::prelude::Address = <SystemProgram as ::quasar_lang::traits::Id>::ID;
}
impl ConstPda {
    #[doc(hidden)]
    #[inline]
    pub fn __quasar_pda_config(
        program_id: &::quasar_lang::prelude::Address,
    ) -> ::quasar_lang::prelude::Address {
        let seeds = <Config>::seeds();
        ::quasar_lang::pda::find_program_address_const(&seeds.as_slices(), program_id).0
    }
    #[doc(hidden)]
    #[inline]
    pub fn __quasar_pda_registry(
        program_id: &::quasar_lang::prelude::Address,
    ) -> ::quasar_lang::prelude::Address {
        let seeds = <Registry>::seeds();
        ::quasar_lang::pda::find_program_address_const(&seeds.as_slices(), program_id).0
    }
}
#[cfg(feature = "idl-build")]
::quasar_lang::__private_inventory::submit! {
    ::quasar_lang::idl_build::AccountsMetaFragment(|| {
    (::quasar_lang::idl_build::s("ConstPda"),
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::AccountsMetaEntry::Node(::quasar_lang::idl_build::__reexport::IdlAccountNode
    { name : ::quasar_lang::idl_build::s("payer"), optional : false, writable :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), signer :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), resolver :
    ::quasar_lang::idl_build::__reexport::IdlResolver::Input {}, docs :
    ::quasar_lang::idl_build::Vec::new(), }),
    ::quasar_lang::idl_build::AccountsMetaEntry::Node(::quasar_lang::idl_build::__reexport::IdlAccountNode
    { name : ::quasar_lang::idl_build::s("config"), optional : false, writable :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(true), signer :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), resolver :
    ::quasar_lang::idl_build::__reexport::IdlResolver::Pda { program :
    ::quasar_lang::idl_build::__reexport::IdlPdaProgram::ProgramId {}, seeds : { let mut
    seeds = ::quasar_lang::idl_build::Vec::new(); if < Config as
    ::quasar_lang::traits::HasSeeds > ::HAS_SEED_PREFIX { seeds
    .push(::quasar_lang::idl_build::__reexport::IdlPdaSeed::Const { value :
    ::quasar_lang::idl_build::Vec::from(< Config as ::quasar_lang::traits::HasSeeds >
    ::SEED_PREFIX), }); } seeds }, }, docs : ::quasar_lang::idl_build::Vec::new(), }),
    ::quasar_lang::idl_build::AccountsMetaEntry::Node(::quasar_lang::idl_build::__reexport::IdlAccountNode
    { name : ::quasar_lang::idl_build::s("registry"), optional : false, writable :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), signer :
    ::quasar_lang::idl_build::__reexport::AccountFlag::Fixed(false), resolver :
    ::quasar_lang::idl_build::__reexport::IdlResolver::Pda { program :
    ::quasar_lang::idl_build::__reexport::IdlPdaProgram::ProgramId {}, seeds : { let mut
    seeds = ::quasar_lang::idl_build::Vec::new(); if < Registry as
    ::quasar_lang::traits::HasSeeds > ::HAS_SEED_PREFIX { seeds
    .push(::quasar_lang::idl_build::__reexport::IdlPdaSeed::Const { value :
    ::quasar_lang::idl_build::Vec::from(< Registry as ::quasar_lang::traits::HasSeeds >
    ::SEED_PREFIX), }); } seeds }, }, docs : ::quasar_lang::idl_build::Vec::new(), }),
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
    (::quasar_lang::idl_build::s("ConstPda"),
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
    ::quasar_lang::idl_build::s("config"), account_type :
    ::quasar_lang::idl_build::s("Account < Config >"), wrapper :
    ::quasar_lang::idl_build::s("Account"), writable : true, signer : false, optional :
    false, allow_duplicate : false, load :
    ::quasar_lang::idl_build::s("Fixed(validates=[])"), pre_load :
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::s("VerifyAddress(expr=`Config :: seeds ()` error=None)"),
    ::quasar_lang::idl_build::s("Init::Program(payer=payer space_ty=`Account < Config >` idempotent=false verified_address=Some(expr=`Config :: seeds ()` error=None))")],
    post_load : ::quasar_lang::idl_build::vec![], epilogue :
    ::quasar_lang::idl_build::vec![], },
    ::quasar_lang::idl_build::__reexport::IdlAccountValidation { name :
    ::quasar_lang::idl_build::s("registry"), account_type :
    ::quasar_lang::idl_build::s("Account < Registry >"), wrapper :
    ::quasar_lang::idl_build::s("Account"), writable : false, signer : false, optional :
    false, allow_duplicate : false, load :
    ::quasar_lang::idl_build::s("Fixed(validates=[])"), pre_load :
    ::quasar_lang::idl_build::vec![], post_load :
    ::quasar_lang::idl_build::vec![::quasar_lang::idl_build::s("VerifyExistingAddress(expr=`Registry :: seeds ()` error=None)")],
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
