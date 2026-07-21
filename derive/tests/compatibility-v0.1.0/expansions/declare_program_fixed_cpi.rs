pub mod external_vault {
    pub const ID: ::quasar_lang::prelude::Address = ::quasar_lang::prelude::address!(
        "11111111111111111111111111111111"
    );
    ::quasar_lang::define_account!(
        pub struct ExternalVault => [::quasar_lang::checks::Executable,
        ::quasar_lang::checks::Address]
    );
    impl ::quasar_lang::traits::Id for ExternalVault {
        const ID: ::quasar_lang::prelude::Address = ID;
    }
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct VaultConfig {
        pub limits: [u64; 2usize],
        pub windows: [Window; 2usize],
        pub enabled: bool,
    }
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Window {
        pub min: u32,
        pub max: u32,
    }
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ForeignVault {
        pub authority: ::quasar_lang::prelude::Address,
        pub config: VaultConfig,
        pub matrix: [[u16; 2usize]; 2usize],
    }
    impl ForeignVault {
        /// Foreign account discriminator from the declared IDL.
        pub const ACCOUNT_DISCRIMINATOR: &'static [u8] = &[7u8, 8u8];
        /// Minimum fixed account-data length, including the discriminator.
        pub const ACCOUNT_DATA_LEN: usize = 75usize;
        /// Validate and decode an owned snapshot of this foreign account.
        #[inline(always)]
        pub fn read_account(
            view: &::quasar_lang::prelude::AccountView,
        ) -> Result<Self, ::quasar_lang::prelude::ProgramError> {
            if !view.owned_by(&ID) {
                return Err(::quasar_lang::prelude::ProgramError::IllegalOwner);
            }
            if view.data_len() < Self::ACCOUNT_DATA_LEN {
                return Err(::quasar_lang::prelude::ProgramError::AccountDataTooSmall);
            }
            let __data = view.try_borrow()?;
            if __data[0usize] != 7u8 || __data[1usize] != 8u8 {
                return Err(::quasar_lang::prelude::ProgramError::InvalidAccountData);
            }
            Ok(Self {
                authority: ::quasar_lang::prelude::Address::new_from_array([
                    __data[2usize],
                    __data[3usize],
                    __data[4usize],
                    __data[5usize],
                    __data[6usize],
                    __data[7usize],
                    __data[8usize],
                    __data[9usize],
                    __data[10usize],
                    __data[11usize],
                    __data[12usize],
                    __data[13usize],
                    __data[14usize],
                    __data[15usize],
                    __data[16usize],
                    __data[17usize],
                    __data[18usize],
                    __data[19usize],
                    __data[20usize],
                    __data[21usize],
                    __data[22usize],
                    __data[23usize],
                    __data[24usize],
                    __data[25usize],
                    __data[26usize],
                    __data[27usize],
                    __data[28usize],
                    __data[29usize],
                    __data[30usize],
                    __data[31usize],
                    __data[32usize],
                    __data[33usize],
                ]),
                config: VaultConfig {
                    limits: [
                        u64::from_le_bytes([
                            __data[34usize],
                            __data[35usize],
                            __data[36usize],
                            __data[37usize],
                            __data[38usize],
                            __data[39usize],
                            __data[40usize],
                            __data[41usize],
                        ]),
                        u64::from_le_bytes([
                            __data[42usize],
                            __data[43usize],
                            __data[44usize],
                            __data[45usize],
                            __data[46usize],
                            __data[47usize],
                            __data[48usize],
                            __data[49usize],
                        ]),
                    ],
                    windows: [
                        Window {
                            min: u32::from_le_bytes([
                                __data[50usize],
                                __data[51usize],
                                __data[52usize],
                                __data[53usize],
                            ]),
                            max: u32::from_le_bytes([
                                __data[54usize],
                                __data[55usize],
                                __data[56usize],
                                __data[57usize],
                            ]),
                        },
                        Window {
                            min: u32::from_le_bytes([
                                __data[58usize],
                                __data[59usize],
                                __data[60usize],
                                __data[61usize],
                            ]),
                            max: u32::from_le_bytes([
                                __data[62usize],
                                __data[63usize],
                                __data[64usize],
                                __data[65usize],
                            ]),
                        },
                    ],
                    enabled: match __data[66usize] {
                        0 => false,
                        1 => true,
                        _ => {
                            return Err(
                                ::quasar_lang::prelude::ProgramError::InvalidAccountData,
                            );
                        }
                    },
                },
                matrix: [
                    [
                        u16::from_le_bytes([__data[67usize], __data[68usize]]),
                        u16::from_le_bytes([__data[69usize], __data[70usize]]),
                    ],
                    [
                        u16::from_le_bytes([__data[71usize], __data[72usize]]),
                        u16::from_le_bytes([__data[73usize], __data[74usize]]),
                    ],
                ],
            })
        }
    }
    #[inline(always)]
    pub fn set_vault<'a>(
        __program: &'a ::quasar_lang::prelude::AccountView,
        payer: &'a ::quasar_lang::prelude::AccountView,
        vault: &'a ::quasar_lang::prelude::AccountView,
        authority: &'a ::quasar_lang::prelude::AccountView,
        system_program: &'a ::quasar_lang::prelude::AccountView,
        config: VaultConfig,
        nonce: u16,
        beneficiary: &::quasar_lang::prelude::Address,
        signers: [::quasar_lang::prelude::Address; 2usize],
    ) -> ::quasar_lang::cpi::CpiCall<'a, 4usize, 133usize> {
        let __data = {
            let mut __buf = core::mem::MaybeUninit::<[u8; 133usize]>::uninit();
            let __ptr = __buf.as_mut_ptr() as *mut u8;
            unsafe {
                core::ptr::write(__ptr.add(0usize), 9u8);
                core::ptr::write(__ptr.add(1usize), 4u8);
                core::ptr::copy_nonoverlapping(
                    config.limits[0usize].to_le_bytes().as_ptr(),
                    __ptr.add(2usize),
                    8usize,
                );
                core::ptr::copy_nonoverlapping(
                    config.limits[1usize].to_le_bytes().as_ptr(),
                    __ptr.add(10usize),
                    8usize,
                );
                core::ptr::copy_nonoverlapping(
                    config.windows[0usize].min.to_le_bytes().as_ptr(),
                    __ptr.add(18usize),
                    4usize,
                );
                core::ptr::copy_nonoverlapping(
                    config.windows[0usize].max.to_le_bytes().as_ptr(),
                    __ptr.add(22usize),
                    4usize,
                );
                core::ptr::copy_nonoverlapping(
                    config.windows[1usize].min.to_le_bytes().as_ptr(),
                    __ptr.add(26usize),
                    4usize,
                );
                core::ptr::copy_nonoverlapping(
                    config.windows[1usize].max.to_le_bytes().as_ptr(),
                    __ptr.add(30usize),
                    4usize,
                );
                core::ptr::write(__ptr.add(34usize), config.enabled as u8);
                core::ptr::copy_nonoverlapping(
                    nonce.to_le_bytes().as_ptr(),
                    __ptr.add(35usize),
                    2usize,
                );
                core::ptr::copy_nonoverlapping(
                    beneficiary.as_ref().as_ptr(),
                    __ptr.add(37usize),
                    32usize,
                );
                core::ptr::copy_nonoverlapping(
                    signers[0usize].as_ref().as_ptr(),
                    __ptr.add(69usize),
                    32usize,
                );
                core::ptr::copy_nonoverlapping(
                    signers[1usize].as_ref().as_ptr(),
                    __ptr.add(101usize),
                    32usize,
                );
                __buf.assume_init()
            }
        };
        ::quasar_lang::cpi::CpiCall::new(
            __program.address(),
            [
                ::quasar_lang::cpi::InstructionAccount::writable_signer(payer.address()),
                ::quasar_lang::cpi::InstructionAccount::writable(vault.address()),
                ::quasar_lang::cpi::InstructionAccount::readonly_signer(
                    authority.address(),
                ),
                ::quasar_lang::cpi::InstructionAccount::readonly(
                    system_program.address(),
                ),
            ],
            [payer, vault, authority, system_program],
            __data,
        )
    }
    impl ExternalVault {
        #[inline(always)]
        pub fn set_vault<'a>(
            &'a self,
            payer: &'a impl ::quasar_lang::traits::AsAccountView,
            vault: &'a impl ::quasar_lang::traits::AsAccountView,
            authority: &'a impl ::quasar_lang::traits::AsAccountView,
            system_program: &'a impl ::quasar_lang::traits::AsAccountView,
            config: VaultConfig,
            nonce: u16,
            beneficiary: &::quasar_lang::prelude::Address,
            signers: [::quasar_lang::prelude::Address; 2usize],
        ) -> ::quasar_lang::cpi::CpiCall<'a, 4usize, 133usize> {
            set_vault(
                ::quasar_lang::traits::AsAccountView::to_account_view(self),
                ::quasar_lang::traits::AsAccountView::to_account_view(payer),
                ::quasar_lang::traits::AsAccountView::to_account_view(vault),
                ::quasar_lang::traits::AsAccountView::to_account_view(authority),
                ::quasar_lang::traits::AsAccountView::to_account_view(system_program),
                config,
                nonce,
                beneficiary,
                signers,
            )
        }
    }
}
