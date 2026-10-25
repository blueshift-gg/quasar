#![feature(prelude_import)]
//! A vault this program owns: deposits arrive through the System Program, and
//! withdrawals take lamports straight out, with no call. Blueshift Quasar's
//! vault example, written here.
#![no_std]
extern crate core;
#[prelude_import]
use core::prelude::rust_2024::*;
use quasar_lang::prelude::*;
pub const ID: ::quasar_lang::Address = ::quasar_lang::Address::from_str_const(
    "33333333333333333333333333333333333333333333",
);
/// Carries each account kind's `UniqueDiscriminator` impl.
#[doc(hidden)]
pub struct __Kinds;
/// This program, for `Program<..>`; `#[program]` names it.
#[doc(hidden)]
pub struct __Program;
impl ::quasar_lang::wrapper::ProgramId for __Program {
    const ID: ::quasar_lang::Address = ID;
}
#[seeds(b"vault", user:Address)]
pub struct VaultPda;
impl VaultPda {
    pub fn seeds<'a>(
        user: &'a impl ::quasar_lang::pda::Seed<Address>,
    ) -> ::quasar_lang::pda::Seeds<'a, 2usize> {
        ::quasar_lang::pda::Seeds::new([
            b"vault" as &[u8],
            ::quasar_lang::pda::Seed::seed(user),
        ])
    }
}
impl VaultPda {
    /// The seeds of `account`'s address, each from its field of that
    /// name or else the account of that name.
    pub fn own_seeds<
        'a,
        __Named: ::quasar_lang::pda::Named<{ ::quasar_lang::pda::name("user") }> + ?Sized,
    >(
        account: &'a impl ?Sized,
        accounts: &'a __Named,
    ) -> ::quasar_lang::pda::Seeds<'a, 2usize> {
        let _ = (account, accounts);
        Self::seeds(
            <__Named as ::quasar_lang::pda::Named<
                { ::quasar_lang::pda::name("user") },
            >>::named(accounts),
        )
    }
}
/// Lamports to move.
pub struct Amount {
    pub amount: u64,
}
const _: usize = ::quasar_lang::data::field_size::<u64>();
impl ::quasar_lang::data::InstructionData for Amount {
    const SIZE: usize = 0 + ::quasar_lang::data::field_size::<u64>();
    #[inline(always)]
    fn decode(bytes: &[u8]) -> ::quasar_lang::Result<Self> {
        if bytes.len() != Self::SIZE {
            return ::core::result::Result::Err(
                ::quasar_lang::ProgramError::InvalidInstructionData,
            );
        }
        let mut at = 0;
        ::core::result::Result::Ok(Self {
            amount: ::quasar_lang::data::take::<u64>(bytes, &mut at)?,
        })
    }
    fn encode(&self, out: &mut [u8]) {
        let mut at = 0;
        ::quasar_lang::data::put(&self.amount, out, &mut at);
    }
    const IDL: &'static [::quasar_lang::client::IdlField] = &[
        ::quasar_lang::client::IdlField {
            name: "amount",
            ty: "u64",
        },
    ];
}
pub struct Deposit {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, seeds)]
    pub vault: Pda<UncheckedAccount, VaultPda>,
    pub system_program: Program<SystemProgram>,
}
const _: () = {
    ::quasar_lang::field::required(
        &[
            <Signer as ::quasar_lang::field::Field>::INFO,
            <Pda<UncheckedAccount, VaultPda> as ::quasar_lang::field::Field>::INFO,
            <Program<SystemProgram> as ::quasar_lang::field::Field>::INFO,
        ],
    );
    ::quasar_lang::field::not_pda::<Signer>();
    ::quasar_lang::field::not_remaining::<Pda<UncheckedAccount, VaultPda>>();
    ::quasar_lang::field::not_pda::<Program<SystemProgram>>();
};
impl Deposit {
    /// What the instruction checks of each account `cpi_accounts`
    /// takes, so a handler that always calls it need not.
    pub const ENFORCES: ::quasar_lang::cpi::Enforces = &[
        ::quasar_lang::cpi::Argument::checked(
            &[
                ::quasar_lang::cpi::enforced(
                    <Signer as ::quasar_lang::field::Field>::CHECK,
                    ::core::option::Option::None,
                ),
                ::quasar_lang::cpi::enforced(
                    <Signer as ::quasar_lang::field::Field>::SIGNS,
                    ::core::option::Option::None,
                ),
                ::quasar_lang::cpi::enforced(
                    ::core::option::Option::Some(::quasar_lang::cpi::MUTABLE_CHECK),
                    ::core::option::Option::None,
                ),
            ],
        ),
        ::quasar_lang::cpi::Argument::checked(
            &[
                ::quasar_lang::cpi::enforced(
                    <<Pda<
                        UncheckedAccount,
                        VaultPda,
                    > as ::quasar_lang::pda::Seeded>::Account as ::quasar_lang::field::Field>::CHECK,
                    ::core::option::Option::None,
                ),
                ::quasar_lang::cpi::enforced(
                    <<Pda<
                        UncheckedAccount,
                        VaultPda,
                    > as ::quasar_lang::pda::Seeded>::Account as ::quasar_lang::field::Field>::SIGNS,
                    ::core::option::Option::None,
                ),
                ::quasar_lang::cpi::enforced(
                    ::core::option::Option::Some(::quasar_lang::cpi::MUTABLE_CHECK),
                    ::core::option::Option::None,
                ),
            ],
        ),
        ::quasar_lang::cpi::Argument::checked(
            &[
                ::quasar_lang::cpi::enforced(
                    <Program<SystemProgram> as ::quasar_lang::field::Field>::CHECK,
                    ::core::option::Option::None,
                ),
                ::quasar_lang::cpi::enforced(
                    <Program<SystemProgram> as ::quasar_lang::field::Field>::SIGNS,
                    ::core::option::Option::None,
                ),
                ::quasar_lang::cpi::enforced(
                    ::core::option::Option::None,
                    ::core::option::Option::None,
                ),
            ],
        ),
    ];
    /// This instruction's accounts, for a program that calls it:
    /// writable and signing as the instruction declares them.
    #[allow(clippy::too_many_arguments)]
    pub fn cpi_accounts<'a>(
        user: &'a mut impl ::quasar_lang::wrapper::Wrapper,
        vault: &'a mut impl ::quasar_lang::wrapper::Wrapper,
        system_program: &'a impl ::quasar_lang::wrapper::Wrapper,
    ) -> [::quasar_lang::cpi::CpiAccount<'a>; 3usize] {
        [
            ::quasar_lang::cpi::CpiAccount::writable_field::<
                Signer,
                _,
            >(user, &crate::ID),
            ::quasar_lang::cpi::CpiAccount::writable_field::<
                Pda<UncheckedAccount, VaultPda>,
                _,
            >(vault, &crate::ID),
            ::quasar_lang::cpi::CpiAccount::readonly_field::<
                Program<SystemProgram>,
                _,
            >(system_program),
        ]
    }
}
impl<__Data> ::quasar_lang::context::Accounts<__Data> for Deposit {
    type Deferred = (
        ::core::option::Option<::quasar_lang::effects::Settle>,
        ::core::option::Option<::quasar_lang::effects::Settle>,
    );
    const FIELDS: &'static [::quasar_lang::field::FieldInfo] = &[
        <Signer as ::quasar_lang::field::Field>::INFO,
        <Pda<UncheckedAccount, VaultPda> as ::quasar_lang::field::Field>::INFO,
        <Program<SystemProgram> as ::quasar_lang::field::Field>::INFO,
    ];
    #[inline(always)]
    fn from_cursor<__Vouches: ::quasar_lang::cpi::Vouches>(
        cursor: &mut ::quasar_lang::input::Cursor,
        _: &__Data,
    ) -> ::quasar_lang::Result<Self> {
        struct __Named<'a, T0, T1, T2>(&'a T0, &'a T1, &'a T2);
        impl<T0, T1, T2> ::quasar_lang::pda::Named<{ ::quasar_lang::pda::name("user") }>
        for __Named<'_, T0, T1, T2>
        where
            T0: ::quasar_lang::wrapper::Wrapper,
        {
            fn named(&self) -> &::quasar_lang::Address {
                ::quasar_lang::wrapper::Wrapper::address(self.0)
            }
        }
        impl<T0, T1, T2> ::quasar_lang::pda::Named<{ ::quasar_lang::pda::name("vault") }>
        for __Named<'_, T0, T1, T2>
        where
            T1: ::quasar_lang::wrapper::Wrapper,
        {
            fn named(&self) -> &::quasar_lang::Address {
                ::quasar_lang::wrapper::Wrapper::address(self.1)
            }
        }
        impl<
            T0,
            T1,
            T2,
        > ::quasar_lang::pda::Named<{ ::quasar_lang::pda::name("system_program") }>
        for __Named<'_, T0, T1, T2>
        where
            T2: ::quasar_lang::wrapper::Wrapper,
        {
            fn named(&self) -> &::quasar_lang::Address {
                ::quasar_lang::wrapper::Wrapper::address(self.2)
            }
        }
        cursor
            .require(const {
                ::quasar_lang::field::required(
                    <Self as ::quasar_lang::context::Accounts<()>>::FIELDS,
                )
            })?;
        let user = <Signer as ::quasar_lang::field::Field>::load_vouched(
            cursor,
            true,
            const {
                #[allow(unused_imports)]
                use ::quasar_lang::cpi::Unkeyed as _;
                ::quasar_lang::cpi::waived_field::<
                    Signer,
                >(<__Vouches as ::quasar_lang::cpi::Vouches>::VOUCHES, "user")
            },
            &::quasar_lang::error::At {
                name: "user",
                lines: &[
                    "    --> examples/lamport-vault/src/lib.rs:23:15",
                    "     |",
                    "  23 |     pub user: Signer,",
                    "     |               ^^^^^^ account #0 of the instruction, which the program writes to",
                    "     |",
                ],
            },
        )?;
        let vault = <<Pda<
            UncheckedAccount,
            VaultPda,
        > as ::quasar_lang::pda::Seeded>::Account as ::quasar_lang::field::Field>::load_vouched(
            cursor,
            true,
            const {
                #[allow(unused_imports)]
                use ::quasar_lang::cpi::Unkeyed as _;
                ::quasar_lang::cpi::waived_field::<
                    <Pda<
                        UncheckedAccount,
                        VaultPda,
                    > as ::quasar_lang::pda::Seeded>::Account,
                >(<__Vouches as ::quasar_lang::cpi::Vouches>::VOUCHES, "vault")
            },
            &::quasar_lang::error::At {
                name: "vault",
                lines: &[
                    "    --> examples/lamport-vault/src/lib.rs:25:16",
                    "     |",
                    "  25 |     pub vault: Pda<UncheckedAccount, VaultPda>,",
                    "     |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ account #1 of the instruction, which the program writes to",
                    "     |",
                ],
            },
        )?;
        let system_program = <Program<
            SystemProgram,
        > as ::quasar_lang::field::Field>::load_vouched(
            cursor,
            false,
            const {
                #[allow(unused_imports)]
                use ::quasar_lang::cpi::Unkeyed as _;
                ::quasar_lang::cpi::waived_field::<
                    Program<SystemProgram>,
                >(<__Vouches as ::quasar_lang::cpi::Vouches>::VOUCHES, "system_program")
            },
            &::quasar_lang::error::At {
                name: "system_program",
                lines: &[
                    "    --> examples/lamport-vault/src/lib.rs:26:25",
                    "     |",
                    "  26 |     pub system_program: Program<SystemProgram>,",
                    "     |                         ^^^^^^^^^^^^^^^^^^^^^^ account #2 of the instruction",
                    "     |",
                ],
            },
        )?;
        let vault = {
            let bump = {
                let named = __Named(&user, &vault, &system_program);
                <<Pda<
                    UncheckedAccount,
                    VaultPda,
                > as ::quasar_lang::pda::Seeded>::Seeds>::own_seeds(&vault, &named)
                    .bump_of(&vault, &crate::ID)
            };
            let bump = ::quasar_lang::error::explain(
                bump,
                "error[InvalidSeeds]: account `vault` is not at the address its seeds derive",
                &::quasar_lang::error::At {
                    name: "vault",
                    lines: &[
                        "    --> examples/lamport-vault/src/lib.rs:24:20",
                        "     |",
                        "  24 |     #[account(mut, seeds)]",
                        "     |                    ^^^^^ account #1 of the instruction, `vault`, does not satisfy this",
                        "     |",
                    ],
                },
                "derive the address with `find_program_address` from the seeds its type declares in `#[seeds(..)]`, the values given here, and this program's id; a stored bump must be the one the account was created with",
            )?;
            <Pda<
                UncheckedAccount,
                VaultPda,
            > as ::quasar_lang::pda::Seeded>::found(vault, bump)
        };
        ::core::result::Result::Ok(Self {
            user,
            vault,
            system_program,
        })
    }
    fn deferred(&self) -> Self::Deferred {
        (
            <Signer as ::quasar_lang::field::Field>::settle(&self.user),
            <Pda<
                UncheckedAccount,
                VaultPda,
            > as ::quasar_lang::field::Field>::settle(&self.vault),
        )
    }
    fn run_deferred(deferred: Self::Deferred) -> ::quasar_lang::Result {
        if let ::core::option::Option::Some(effect) = deferred.0 {
            effect.run()?;
        }
        if let ::core::option::Option::Some(effect) = deferred.1 {
            effect.run()?;
        }
        ::core::result::Result::Ok(())
    }
}
impl ::quasar_lang::client::ClientAccounts for Deposit {
    type Keys = (
        <Signer as ::quasar_lang::field::Field>::Key,
        <Pda<UncheckedAccount, VaultPda> as ::quasar_lang::field::Field>::Key,
        <Program<SystemProgram> as ::quasar_lang::field::Field>::Key,
    );
    fn metas(
        (user, vault, system_program): Self::Keys,
        program: ::quasar_lang::Address,
    ) -> ::quasar_lang::client::Vec<::quasar_lang::client::AccountMeta> {
        let mut metas = ::quasar_lang::client::Vec::new();
        <Signer as ::quasar_lang::field::Field>::metas(user, true, program, &mut metas);
        <Pda<
            UncheckedAccount,
            VaultPda,
        > as ::quasar_lang::field::Field>::metas(vault, true, program, &mut metas);
        <Program<
            SystemProgram,
        > as ::quasar_lang::field::Field>::metas(
            system_program,
            false,
            program,
            &mut metas,
        );
        metas
    }
    fn idl_accounts() -> ::quasar_lang::client::Vec<::quasar_lang::client::IdlAccount> {
        ::quasar_lang::client::Vec::from([
            <Signer as ::quasar_lang::field::Field>::idl("user", true),
            <Pda<
                UncheckedAccount,
                VaultPda,
            > as ::quasar_lang::field::Field>::idl("vault", true),
            <Program<
                SystemProgram,
            > as ::quasar_lang::field::Field>::idl("system_program", false),
        ])
    }
    fn idl_kinds() -> ::quasar_lang::client::Vec<::quasar_lang::client::IdlKind> {
        [
            <Signer as ::quasar_lang::field::Field>::idl_kind(),
            <Pda<UncheckedAccount, VaultPda> as ::quasar_lang::field::Field>::idl_kind(),
            <Program<SystemProgram> as ::quasar_lang::field::Field>::idl_kind(),
        ]
            .into_iter()
            .flatten()
            .collect()
    }
}
impl Deposit {
    /// The checks the calls in `__Vouches` make, which loading
    /// these accounts for that handler leaves out.
    pub fn elided<__Vouches: ::quasar_lang::cpi::Vouches>() -> ::quasar_lang::client::Vec<
        ::quasar_lang::client::String,
    > {
        let mut elided = ::quasar_lang::client::Vec::new();
        if const {
            #[allow(unused_imports)]
            use ::quasar_lang::cpi::Unkeyed as _;
            ::quasar_lang::cpi::waived_field::<
                Signer,
            >(<__Vouches as ::quasar_lang::cpi::Vouches>::VOUCHES, "user")
        } {
            let check = <Signer as ::quasar_lang::field::Field>::CHECK
                .unwrap_or_default();
            elided
                .push(
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("{0}: {1}", "user", check))
                    }),
                );
        }
        if const {
            #[allow(unused_imports)]
            use ::quasar_lang::cpi::Unkeyed as _;
            ::quasar_lang::cpi::waived_field::<
                <Pda<UncheckedAccount, VaultPda> as ::quasar_lang::pda::Seeded>::Account,
            >(<__Vouches as ::quasar_lang::cpi::Vouches>::VOUCHES, "vault")
        } {
            let check = <<Pda<
                UncheckedAccount,
                VaultPda,
            > as ::quasar_lang::pda::Seeded>::Account as ::quasar_lang::field::Field>::CHECK
                .unwrap_or_default();
            elided
                .push(
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("{0}: {1}", "vault", check))
                    }),
                );
        }
        if const {
            #[allow(unused_imports)]
            use ::quasar_lang::cpi::Unkeyed as _;
            ::quasar_lang::cpi::waived_field::<
                Program<SystemProgram>,
            >(<__Vouches as ::quasar_lang::cpi::Vouches>::VOUCHES, "system_program")
        } {
            let check = <Program<SystemProgram> as ::quasar_lang::field::Field>::CHECK
                .unwrap_or_default();
            elided
                .push(
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(
                            format_args!("{0}: {1}", "system_program", check),
                        )
                    }),
                );
        }
        elided
    }
}
pub struct Withdraw {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, seeds)]
    pub vault: Pda<UncheckedAccount, VaultPda>,
}
const _: () = {
    ::quasar_lang::field::required(
        &[
            <Signer as ::quasar_lang::field::Field>::INFO,
            <Pda<UncheckedAccount, VaultPda> as ::quasar_lang::field::Field>::INFO,
        ],
    );
    ::quasar_lang::field::not_pda::<Signer>();
    ::quasar_lang::field::not_remaining::<Pda<UncheckedAccount, VaultPda>>();
};
impl Withdraw {
    /// What the instruction checks of each account `cpi_accounts`
    /// takes, so a handler that always calls it need not.
    pub const ENFORCES: ::quasar_lang::cpi::Enforces = &[
        ::quasar_lang::cpi::Argument::checked(
            &[
                ::quasar_lang::cpi::enforced(
                    <Signer as ::quasar_lang::field::Field>::CHECK,
                    ::core::option::Option::None,
                ),
                ::quasar_lang::cpi::enforced(
                    <Signer as ::quasar_lang::field::Field>::SIGNS,
                    ::core::option::Option::None,
                ),
                ::quasar_lang::cpi::enforced(
                    ::core::option::Option::Some(::quasar_lang::cpi::MUTABLE_CHECK),
                    ::core::option::Option::None,
                ),
            ],
        ),
        ::quasar_lang::cpi::Argument::checked(
            &[
                ::quasar_lang::cpi::enforced(
                    <<Pda<
                        UncheckedAccount,
                        VaultPda,
                    > as ::quasar_lang::pda::Seeded>::Account as ::quasar_lang::field::Field>::CHECK,
                    ::core::option::Option::None,
                ),
                ::quasar_lang::cpi::enforced(
                    <<Pda<
                        UncheckedAccount,
                        VaultPda,
                    > as ::quasar_lang::pda::Seeded>::Account as ::quasar_lang::field::Field>::SIGNS,
                    ::core::option::Option::None,
                ),
                ::quasar_lang::cpi::enforced(
                    ::core::option::Option::Some(::quasar_lang::cpi::MUTABLE_CHECK),
                    ::core::option::Option::None,
                ),
            ],
        ),
    ];
    /// This instruction's accounts, for a program that calls it:
    /// writable and signing as the instruction declares them.
    #[allow(clippy::too_many_arguments)]
    pub fn cpi_accounts<'a>(
        user: &'a mut impl ::quasar_lang::wrapper::Wrapper,
        vault: &'a mut impl ::quasar_lang::wrapper::Wrapper,
    ) -> [::quasar_lang::cpi::CpiAccount<'a>; 2usize] {
        [
            ::quasar_lang::cpi::CpiAccount::writable_field::<
                Signer,
                _,
            >(user, &crate::ID),
            ::quasar_lang::cpi::CpiAccount::writable_field::<
                Pda<UncheckedAccount, VaultPda>,
                _,
            >(vault, &crate::ID),
        ]
    }
}
impl<__Data> ::quasar_lang::context::Accounts<__Data> for Withdraw {
    type Deferred = (
        ::core::option::Option<::quasar_lang::effects::Settle>,
        ::core::option::Option<::quasar_lang::effects::Settle>,
    );
    const FIELDS: &'static [::quasar_lang::field::FieldInfo] = &[
        <Signer as ::quasar_lang::field::Field>::INFO,
        <Pda<UncheckedAccount, VaultPda> as ::quasar_lang::field::Field>::INFO,
    ];
    #[inline(always)]
    fn from_cursor<__Vouches: ::quasar_lang::cpi::Vouches>(
        cursor: &mut ::quasar_lang::input::Cursor,
        _: &__Data,
    ) -> ::quasar_lang::Result<Self> {
        struct __Named<'a, T0, T1>(&'a T0, &'a T1);
        impl<T0, T1> ::quasar_lang::pda::Named<{ ::quasar_lang::pda::name("user") }>
        for __Named<'_, T0, T1>
        where
            T0: ::quasar_lang::wrapper::Wrapper,
        {
            fn named(&self) -> &::quasar_lang::Address {
                ::quasar_lang::wrapper::Wrapper::address(self.0)
            }
        }
        impl<T0, T1> ::quasar_lang::pda::Named<{ ::quasar_lang::pda::name("vault") }>
        for __Named<'_, T0, T1>
        where
            T1: ::quasar_lang::wrapper::Wrapper,
        {
            fn named(&self) -> &::quasar_lang::Address {
                ::quasar_lang::wrapper::Wrapper::address(self.1)
            }
        }
        cursor
            .require(const {
                ::quasar_lang::field::required(
                    <Self as ::quasar_lang::context::Accounts<()>>::FIELDS,
                )
            })?;
        let user = <Signer as ::quasar_lang::field::Field>::load_vouched(
            cursor,
            true,
            const {
                #[allow(unused_imports)]
                use ::quasar_lang::cpi::Unkeyed as _;
                ::quasar_lang::cpi::waived_field::<
                    Signer,
                >(<__Vouches as ::quasar_lang::cpi::Vouches>::VOUCHES, "user")
            },
            &::quasar_lang::error::At {
                name: "user",
                lines: &[
                    "    --> examples/lamport-vault/src/lib.rs:32:15",
                    "     |",
                    "  32 |     pub user: Signer,",
                    "     |               ^^^^^^ account #0 of the instruction, which the program writes to",
                    "     |",
                ],
            },
        )?;
        let vault = <<Pda<
            UncheckedAccount,
            VaultPda,
        > as ::quasar_lang::pda::Seeded>::Account as ::quasar_lang::field::Field>::load_vouched(
            cursor,
            true,
            const {
                #[allow(unused_imports)]
                use ::quasar_lang::cpi::Unkeyed as _;
                ::quasar_lang::cpi::waived_field::<
                    <Pda<
                        UncheckedAccount,
                        VaultPda,
                    > as ::quasar_lang::pda::Seeded>::Account,
                >(<__Vouches as ::quasar_lang::cpi::Vouches>::VOUCHES, "vault")
            },
            &::quasar_lang::error::At {
                name: "vault",
                lines: &[
                    "    --> examples/lamport-vault/src/lib.rs:34:16",
                    "     |",
                    "  34 |     pub vault: Pda<UncheckedAccount, VaultPda>,",
                    "     |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ account #1 of the instruction, which the program writes to",
                    "     |",
                ],
            },
        )?;
        let vault = {
            let bump = {
                let named = __Named(&user, &vault);
                <<Pda<
                    UncheckedAccount,
                    VaultPda,
                > as ::quasar_lang::pda::Seeded>::Seeds>::own_seeds(&vault, &named)
                    .bump_of(&vault, &crate::ID)
            };
            let bump = ::quasar_lang::error::explain(
                bump,
                "error[InvalidSeeds]: account `vault` is not at the address its seeds derive",
                &::quasar_lang::error::At {
                    name: "vault",
                    lines: &[
                        "    --> examples/lamport-vault/src/lib.rs:33:20",
                        "     |",
                        "  33 |     #[account(mut, seeds)]",
                        "     |                    ^^^^^ account #1 of the instruction, `vault`, does not satisfy this",
                        "     |",
                    ],
                },
                "derive the address with `find_program_address` from the seeds its type declares in `#[seeds(..)]`, the values given here, and this program's id; a stored bump must be the one the account was created with",
            )?;
            <Pda<
                UncheckedAccount,
                VaultPda,
            > as ::quasar_lang::pda::Seeded>::found(vault, bump)
        };
        ::core::result::Result::Ok(Self { user, vault })
    }
    fn deferred(&self) -> Self::Deferred {
        (
            <Signer as ::quasar_lang::field::Field>::settle(&self.user),
            <Pda<
                UncheckedAccount,
                VaultPda,
            > as ::quasar_lang::field::Field>::settle(&self.vault),
        )
    }
    fn run_deferred(deferred: Self::Deferred) -> ::quasar_lang::Result {
        if let ::core::option::Option::Some(effect) = deferred.0 {
            effect.run()?;
        }
        if let ::core::option::Option::Some(effect) = deferred.1 {
            effect.run()?;
        }
        ::core::result::Result::Ok(())
    }
}
impl ::quasar_lang::client::ClientAccounts for Withdraw {
    type Keys = (
        <Signer as ::quasar_lang::field::Field>::Key,
        <Pda<UncheckedAccount, VaultPda> as ::quasar_lang::field::Field>::Key,
    );
    fn metas(
        (user, vault): Self::Keys,
        program: ::quasar_lang::Address,
    ) -> ::quasar_lang::client::Vec<::quasar_lang::client::AccountMeta> {
        let mut metas = ::quasar_lang::client::Vec::new();
        <Signer as ::quasar_lang::field::Field>::metas(user, true, program, &mut metas);
        <Pda<
            UncheckedAccount,
            VaultPda,
        > as ::quasar_lang::field::Field>::metas(vault, true, program, &mut metas);
        metas
    }
    fn idl_accounts() -> ::quasar_lang::client::Vec<::quasar_lang::client::IdlAccount> {
        ::quasar_lang::client::Vec::from([
            <Signer as ::quasar_lang::field::Field>::idl("user", true),
            <Pda<
                UncheckedAccount,
                VaultPda,
            > as ::quasar_lang::field::Field>::idl("vault", true),
        ])
    }
    fn idl_kinds() -> ::quasar_lang::client::Vec<::quasar_lang::client::IdlKind> {
        [
            <Signer as ::quasar_lang::field::Field>::idl_kind(),
            <Pda<UncheckedAccount, VaultPda> as ::quasar_lang::field::Field>::idl_kind(),
        ]
            .into_iter()
            .flatten()
            .collect()
    }
}
impl Withdraw {
    /// The checks the calls in `__Vouches` make, which loading
    /// these accounts for that handler leaves out.
    pub fn elided<__Vouches: ::quasar_lang::cpi::Vouches>() -> ::quasar_lang::client::Vec<
        ::quasar_lang::client::String,
    > {
        let mut elided = ::quasar_lang::client::Vec::new();
        if const {
            #[allow(unused_imports)]
            use ::quasar_lang::cpi::Unkeyed as _;
            ::quasar_lang::cpi::waived_field::<
                Signer,
            >(<__Vouches as ::quasar_lang::cpi::Vouches>::VOUCHES, "user")
        } {
            let check = <Signer as ::quasar_lang::field::Field>::CHECK
                .unwrap_or_default();
            elided
                .push(
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("{0}: {1}", "user", check))
                    }),
                );
        }
        if const {
            #[allow(unused_imports)]
            use ::quasar_lang::cpi::Unkeyed as _;
            ::quasar_lang::cpi::waived_field::<
                <Pda<UncheckedAccount, VaultPda> as ::quasar_lang::pda::Seeded>::Account,
            >(<__Vouches as ::quasar_lang::cpi::Vouches>::VOUCHES, "vault")
        } {
            let check = <<Pda<
                UncheckedAccount,
                VaultPda,
            > as ::quasar_lang::pda::Seeded>::Account as ::quasar_lang::field::Field>::CHECK
                .unwrap_or_default();
            elided
                .push(
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("{0}: {1}", "vault", check))
                    }),
                );
        }
        elided
    }
}
mod lamport_vault {
    use super::*;
    #[inline(always)]
    pub fn deposit(mut ctx: Ctx<Deposit, Amount>) -> Result<()> {
        let Deposit { user, vault, system_program } = &mut ctx.accounts;
        system_program.transfer(user, vault, ctx.data.amount).invoke()
    }
    ///The `deposit` instruction: its discriminator, and how to build or call it.
    pub mod deposit {
        use super::*;
        pub const DISCRIMINATOR: [u8; 1usize] = [0u8];
        /// What the handler's calls check, so the accounts' loads need not.
        pub struct Vouched;
        impl ::quasar_lang::cpi::Vouches for Vouched {
            const VOUCHES: &'static [(
                &'static str,
                &'static [::quasar_lang::cpi::Call],
            )] = &[];
        }
        /// Its accounts' fields, for `#[program]` to check.
        pub const FIELDS: &[::quasar_lang::field::FieldInfo] = <Deposit as ::quasar_lang::context::Accounts<
            Amount,
        >>::FIELDS;
        /// Calls it from another program, with the accounts in the order
        /// its accounts struct declares them.
        pub fn cpi<'a, const N: usize>(
            program: &'a ::quasar_lang::wrapper::Program<crate::__Program>,
            accounts: [::quasar_lang::cpi::CpiAccount<'a>; N],
            data: Amount,
        ) -> ::quasar_lang::cpi::Cpi<
            'a,
            crate::__Program,
            N,
            { 1usize + <Amount as ::quasar_lang::data::InstructionData>::SIZE },
        > {
            let data = ::quasar_lang::data::encode(&DISCRIMINATOR, &data);
            ::quasar_lang::cpi::Cpi::new(program, accounts, data)
        }
        /// The instruction, for a client or a test.
        pub fn instruction(
            accounts: <Deposit as ::quasar_lang::client::ClientAccounts>::Keys,
            data: Amount,
        ) -> ::quasar_lang::client::Instruction {
            let bytes: [u8; {
                1usize + <Amount as ::quasar_lang::data::InstructionData>::SIZE
            }] = ::quasar_lang::data::encode(&DISCRIMINATOR, &data);
            ::quasar_lang::client::Instruction {
                program_id: crate::ID,
                accounts: <Deposit as ::quasar_lang::client::ClientAccounts>::metas(
                    accounts,
                    crate::ID,
                ),
                data: ::quasar_lang::client::Vec::from(bytes),
            }
        }
        pub fn idl() -> ::quasar_lang::client::IdlInstruction {
            ::quasar_lang::client::IdlInstruction {
                name: "deposit",
                discriminator: &DISCRIMINATOR,
                accounts: <Deposit as ::quasar_lang::client::ClientAccounts>::idl_accounts(),
                args: ::quasar_lang::client::Vec::from(
                    <Amount as ::quasar_lang::data::InstructionData>::IDL,
                ),
            }
        }
        /// The checks loading its accounts leaves to its calls.
        pub fn elided() -> ::quasar_lang::client::Vec<::quasar_lang::client::String> {
            <Deposit>::elided::<Vouched>()
        }
        pub fn idl_kinds() -> ::quasar_lang::client::Vec<
            ::quasar_lang::client::IdlKind,
        > {
            <Deposit as ::quasar_lang::client::ClientAccounts>::idl_kinds()
        }
    }
    #[inline(always)]
    pub fn withdraw(mut ctx: Ctx<Withdraw, Amount>) -> Result<()> {
        let Withdraw { user, vault } = &mut ctx.accounts;
        vault.sub_lamports(ctx.data.amount)?;
        user.add_lamports(ctx.data.amount)
    }
    ///The `withdraw` instruction: its discriminator, and how to build or call it.
    pub mod withdraw {
        use super::*;
        pub const DISCRIMINATOR: [u8; 1usize] = [1u8];
        /// What the handler's calls check, so the accounts' loads need not.
        pub struct Vouched;
        impl ::quasar_lang::cpi::Vouches for Vouched {
            const VOUCHES: &'static [(
                &'static str,
                &'static [::quasar_lang::cpi::Call],
            )] = &[];
        }
        /// Its accounts' fields, for `#[program]` to check.
        pub const FIELDS: &[::quasar_lang::field::FieldInfo] = <Withdraw as ::quasar_lang::context::Accounts<
            Amount,
        >>::FIELDS;
        /// Calls it from another program, with the accounts in the order
        /// its accounts struct declares them.
        pub fn cpi<'a, const N: usize>(
            program: &'a ::quasar_lang::wrapper::Program<crate::__Program>,
            accounts: [::quasar_lang::cpi::CpiAccount<'a>; N],
            data: Amount,
        ) -> ::quasar_lang::cpi::Cpi<
            'a,
            crate::__Program,
            N,
            { 1usize + <Amount as ::quasar_lang::data::InstructionData>::SIZE },
        > {
            let data = ::quasar_lang::data::encode(&DISCRIMINATOR, &data);
            ::quasar_lang::cpi::Cpi::new(program, accounts, data)
        }
        /// The instruction, for a client or a test.
        pub fn instruction(
            accounts: <Withdraw as ::quasar_lang::client::ClientAccounts>::Keys,
            data: Amount,
        ) -> ::quasar_lang::client::Instruction {
            let bytes: [u8; {
                1usize + <Amount as ::quasar_lang::data::InstructionData>::SIZE
            }] = ::quasar_lang::data::encode(&DISCRIMINATOR, &data);
            ::quasar_lang::client::Instruction {
                program_id: crate::ID,
                accounts: <Withdraw as ::quasar_lang::client::ClientAccounts>::metas(
                    accounts,
                    crate::ID,
                ),
                data: ::quasar_lang::client::Vec::from(bytes),
            }
        }
        pub fn idl() -> ::quasar_lang::client::IdlInstruction {
            ::quasar_lang::client::IdlInstruction {
                name: "withdraw",
                discriminator: &DISCRIMINATOR,
                accounts: <Withdraw as ::quasar_lang::client::ClientAccounts>::idl_accounts(),
                args: ::quasar_lang::client::Vec::from(
                    <Amount as ::quasar_lang::data::InstructionData>::IDL,
                ),
            }
        }
        /// The checks loading its accounts leaves to its calls.
        pub fn elided() -> ::quasar_lang::client::Vec<::quasar_lang::client::String> {
            <Withdraw>::elided::<Vouched>()
        }
        pub fn idl_kinds() -> ::quasar_lang::client::Vec<
            ::quasar_lang::client::IdlKind,
        > {
            <Withdraw as ::quasar_lang::client::ClientAccounts>::idl_kinds()
        }
    }
}
pub use lamport_vault::*;
/// This program, for `Program<program::Name>` in a program calling it.
pub mod program {
    pub type LamportVault = crate::__Program;
}
const _: () = ::quasar_lang::account::one_width(
    &[lamport_vault::deposit::FIELDS, lamport_vault::withdraw::FIELDS],
);
/// This program's interface, from the constants it dispatches on.
pub fn idl() -> ::quasar_lang::client::Idl {
    let mut kinds = ::quasar_lang::client::Vec::new();
    for kind in [
        lamport_vault::deposit::idl_kinds(),
        lamport_vault::withdraw::idl_kinds(),
    ]
        .into_iter()
        .flatten()
    {
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    ::quasar_lang::client::Idl {
        address: crate::ID,
        name: "lamport_vault",
        instructions: ::quasar_lang::client::Vec::from([
            lamport_vault::deposit::idl(),
            lamport_vault::withdraw::idl(),
        ]),
        kinds,
    }
}
/// Each check an instruction leaves to a call it always makes,
/// one per line: `instruction.account: check`.
pub fn elisions() -> ::quasar_lang::client::String {
    let mut lines = ::quasar_lang::client::String::new();
    for elided in lamport_vault::deposit::elided() {
        lines.push_str("deposit");
        lines.push('.');
        lines.push_str(&elided);
        lines.push('\n');
    }
    for elided in lamport_vault::withdraw::elided() {
        lines.push_str("withdraw");
        lines.push('.');
        lines.push_str(&elided);
        lines.push('\n');
    }
    lines
}
#[allow(unexpected_cfgs)]
const _: () = {
    fn dispatch(
        discriminator: [u8; lamport_vault::deposit::DISCRIMINATOR.len()],
        data: &[u8],
        cursor: ::quasar_lang::input::Cursor,
    ) -> ::quasar_lang::Result {
        match discriminator {
            lamport_vault::deposit::DISCRIMINATOR => {
                ::quasar_lang::context::run::<
                    _,
                    _,
                    _,
                    lamport_vault::deposit::Vouched,
                >(cursor, data, lamport_vault::deposit)
            }
            lamport_vault::withdraw::DISCRIMINATOR => {
                ::quasar_lang::context::run::<
                    _,
                    _,
                    _,
                    lamport_vault::withdraw::Vouched,
                >(cursor, data, lamport_vault::withdraw)
            }
            _ => {
                ::core::result::Result::Err(
                    ::quasar_lang::ProgramError::InvalidInstructionData,
                )
            }
        }
    }
    #[unsafe(export_name = "lamport_vault__quasar_entrypoint")]
    /// # Safety
    /// Called only by the runtime, with its serialized input and, in r2,
    /// the instruction data.
    pub unsafe extern "C" fn entrypoint(input: *mut u8, data: *const u8) -> u64 {
        unsafe { ::quasar_lang::context::entrypoint(input, data, &crate::ID, dispatch) }
    }
};
