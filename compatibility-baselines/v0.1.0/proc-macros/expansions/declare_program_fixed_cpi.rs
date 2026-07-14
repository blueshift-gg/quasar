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
    #[derive(Clone, Copy)]
    pub struct VaultConfig {
        pub limit: u64,
        pub enabled: bool,
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
    ) -> ::quasar_lang::cpi::CpiCall<'a, 4usize, 45usize> {
        let __data = {
            let mut __buf = core::mem::MaybeUninit::<[u8; 45usize]>::uninit();
            let __ptr = __buf.as_mut_ptr() as *mut u8;
            unsafe {
                core::ptr::write(__ptr.add(0usize), 9u8);
                core::ptr::write(__ptr.add(1usize), 4u8);
                core::ptr::copy_nonoverlapping(
                    config.limit.to_le_bytes().as_ptr(),
                    __ptr.add(2usize),
                    8usize,
                );
                core::ptr::write(__ptr.add(10usize), config.enabled as u8);
                core::ptr::copy_nonoverlapping(
                    nonce.to_le_bytes().as_ptr(),
                    __ptr.add(11usize),
                    2usize,
                );
                core::ptr::copy_nonoverlapping(
                    beneficiary.as_ref().as_ptr(),
                    __ptr.add(13usize),
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
        ) -> ::quasar_lang::cpi::CpiCall<'a, 4usize, 45usize> {
            set_vault(
                self.to_account_view(),
                payer.to_account_view(),
                vault.to_account_view(),
                authority.to_account_view(),
                system_program.to_account_view(),
                config,
                nonce,
                beneficiary,
            )
        }
    }
}
