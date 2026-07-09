impl quasar_lang::traits::HasSeeds for VaultPda {
    const SEED_PREFIX: &'static [u8] = &[118u8, 97u8, 117u8, 108u8, 116u8];
    const SEED_DYNAMIC_COUNT: usize = 1usize;
}
/// Zero-copy seed storage (without bump).
pub struct VaultPdaSeedSet<'__quasar_seed> {
    _authority: &'__quasar_seed quasar_lang::prelude::Address,
}
/// Seed set with explicit bump appended.
pub struct VaultPdaSeedSetWithBump<'__quasar_seed> {
    inner: VaultPdaSeedSet<'__quasar_seed>,
    _bump: [u8; 1],
}
impl VaultPda {
    #[inline(always)]
    pub fn seeds<'__quasar_seed>(
        authority: &'__quasar_seed quasar_lang::prelude::Address,
    ) -> VaultPdaSeedSet<'__quasar_seed> {
        VaultPdaSeedSet {
            _authority: authority,
        }
    }
}
impl<'__quasar_seed> VaultPdaSeedSet<'__quasar_seed> {
    #[inline(always)]
    pub fn with_bump(self, bump: u8) -> VaultPdaSeedSetWithBump<'__quasar_seed> {
        VaultPdaSeedSetWithBump {
            inner: self,
            _bump: [bump],
        }
    }
    #[inline(always)]
    pub fn as_slices(&self) -> [&[u8]; 2usize] {
        [b"vault", self._authority.as_ref()]
    }
}
impl<'__quasar_seed> VaultPdaSeedSetWithBump<'__quasar_seed> {
    #[inline(always)]
    pub fn as_slices(&self) -> [&[u8]; 3usize] {
        [b"vault", self.inner._authority.as_ref(), &self._bump]
    }
}
impl<'__quasar_seed> quasar_lang::cpi::CpiSignerSeeds
for VaultPdaSeedSetWithBump<'__quasar_seed> {
    #[inline(always)]
    fn with_signers<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&[quasar_lang::cpi::Signer<'_, '_>]) -> R,
    {
        let seeds = [
            quasar_lang::cpi::Seed::from(b"vault"),
            quasar_lang::cpi::Seed::from(self.inner._authority.as_ref()),
            quasar_lang::cpi::Seed::from(&self._bump),
        ];
        let signer = quasar_lang::cpi::Signer::from(&seeds);
        f(core::slice::from_ref(&signer))
    }
}
impl<'__quasar_seed> quasar_lang::address::AddressVerify
for VaultPdaSeedSet<'__quasar_seed> {
    #[inline(always)]
    fn verify(
        &self,
        actual: &quasar_lang::prelude::Address,
        program_id: &quasar_lang::prelude::Address,
    ) -> Result<u8, quasar_lang::prelude::ProgramError> {
        let slices = self.as_slices();
        quasar_lang::pda::verify_canonical_program_address(&slices, program_id, actual)
    }
    #[inline(always)]
    fn verify_existing(
        &self,
        actual: &quasar_lang::prelude::Address,
        program_id: &quasar_lang::prelude::Address,
    ) -> Result<u8, quasar_lang::prelude::ProgramError> {
        let slices = self.as_slices();
        let bump = quasar_lang::pda::find_bump_for_address(&slices, program_id, actual)
            .map_err(|_| quasar_lang::prelude::ProgramError::from(
                quasar_lang::error::QuasarError::InvalidPda,
            ))?;
        Ok(bump)
    }
    #[inline(always)]
    fn verify_existing_from_account(
        &self,
        actual: &quasar_lang::prelude::Address,
        program_id: &quasar_lang::prelude::Address,
        account: &quasar_lang::__internal::AccountView,
        bump_offset: usize,
    ) -> Result<u8, quasar_lang::prelude::ProgramError> {
        let bump = quasar_lang::pda::read_bump_from_account(account, bump_offset)?;
        let __bump_ref = [bump];
        let slices: [&[u8]; 3usize] = [
            b"vault",
            self._authority.as_ref(),
            __bump_ref.as_ref(),
        ];
        quasar_lang::pda::verify_program_address(&slices, program_id, actual)
            .map_err(|_| quasar_lang::prelude::ProgramError::from(
                quasar_lang::error::QuasarError::InvalidPda,
            ))?;
        Ok(bump)
    }
    #[inline(always)]
    fn with_signer_seeds<R>(
        &self,
        bump: &[u8],
        f: impl FnOnce(&[quasar_lang::cpi::Signer<'_, '_>]) -> R,
    ) -> R {
        let seeds = [
            quasar_lang::cpi::Seed::from(b"vault"),
            quasar_lang::cpi::Seed::from(self._authority.as_ref()),
            quasar_lang::cpi::Seed::from(bump),
        ];
        let signer = quasar_lang::cpi::Signer::from(&seeds);
        f(core::slice::from_ref(&signer))
    }
}
impl<'__quasar_seed> quasar_lang::address::AddressVerify
for VaultPdaSeedSetWithBump<'__quasar_seed> {
    #[inline(always)]
    fn verify(
        &self,
        actual: &quasar_lang::prelude::Address,
        program_id: &quasar_lang::prelude::Address,
    ) -> Result<u8, quasar_lang::prelude::ProgramError> {
        let slices = self.as_slices();
        quasar_lang::pda::verify_program_address(&slices, program_id, actual)?;
        Ok(self._bump[0])
    }
    #[inline(always)]
    fn with_signer_seeds<R>(
        &self,
        _bump: &[u8],
        f: impl FnOnce(&[quasar_lang::cpi::Signer<'_, '_>]) -> R,
    ) -> R {
        let seeds = [
            quasar_lang::cpi::Seed::from(b"vault"),
            quasar_lang::cpi::Seed::from(self.inner._authority.as_ref()),
            quasar_lang::cpi::Seed::from(&self._bump),
        ];
        let signer = quasar_lang::cpi::Signer::from(&seeds);
        f(core::slice::from_ref(&signer))
    }
}
