use crate::errors::EscrowError;
use pinocchio::{
    account::{Ref, RefMut},
    error::ProgramError,
    AccountView,
};

#[repr(C)]
pub struct EscrowAccount {
    discriminator: [u8; 1],
    maker: [u8; 32],
    mint_a: [u8; 32],
    mint_b: [u8; 32],
    maker_ta_b: [u8; 32],
    receive: [u8; 8],
    bump: [u8; 1],
}

const _: () = assert!(core::mem::size_of::<EscrowAccount>() == EscrowAccount::LEN);

impl EscrowAccount {
    pub const DISCRIMINATOR: u8 = 1;
    pub const LEN: usize = 1 + 32 + 32 + 32 + 32 + 8 + 1; // 138
    pub const SEEDS_PREFIX: &'static [u8] = b"escrow";

    // --- Getters ---

    #[inline(always)]
    pub fn maker(&self) -> &[u8; 32] {
        &self.maker
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn mint_a(&self) -> &[u8; 32] {
        &self.mint_a
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn mint_b(&self) -> &[u8; 32] {
        &self.mint_b
    }

    #[inline(always)]
    pub fn maker_ta_b(&self) -> &[u8; 32] {
        &self.maker_ta_b
    }

    #[inline(always)]
    pub fn receive(&self) -> u64 {
        u64::from_le_bytes(self.receive)
    }

    #[inline(always)]
    pub fn bump(&self) -> u8 {
        self.bump[0]
    }

    // --- Setters ---

    #[inline(always)]
    pub fn set_maker(&mut self, v: &[u8]) {
        self.maker.copy_from_slice(v);
    }

    #[inline(always)]
    pub fn set_mint_a(&mut self, v: &[u8]) {
        self.mint_a.copy_from_slice(v);
    }

    #[inline(always)]
    pub fn set_mint_b(&mut self, v: &[u8]) {
        self.mint_b.copy_from_slice(v);
    }

    #[inline(always)]
    pub fn set_maker_ta_b(&mut self, v: &[u8]) {
        self.maker_ta_b.copy_from_slice(v);
    }

    #[inline(always)]
    pub fn set_receive(&mut self, v: u64) {
        self.receive = v.to_le_bytes();
    }

    #[inline(always)]
    pub fn set_bump(&mut self, v: u8) {
        self.bump = [v];
    }

    // --- Account Loading ---

    /// Load an existing escrow account with safe borrow tracking.
    ///
    /// # Safety argument
    /// The pointer cast in `Ref::map` is sound because:
    /// - Length is verified to equal `size_of::<EscrowAccount>()`
    /// - `EscrowAccount` is `#[repr(C)]` with all alignment-1 byte array fields
    /// - Discriminator is validated before the cast
    #[inline(always)]
    pub fn load(account: &AccountView) -> Result<Ref<'_, Self>, ProgramError> {
        if !account.owned_by(&crate::ID) {
            return Err(EscrowError::InvalidAccountOwner.into());
        }
        let data = account.try_borrow()?;
        if data.len() != Self::LEN {
            return Err(EscrowError::InvalidEscrowState.into());
        }
        if data[0] != Self::DISCRIMINATOR {
            return Err(EscrowError::InvalidEscrowState.into());
        }
        Ok(Ref::map(data, |d| unsafe { &*(d.as_ptr() as *const Self) }))
    }

    /// Load an existing escrow account mutably with safe borrow tracking.
    ///
    /// # Safety argument
    /// Same as `load` — the pointer cast is sound due to `#[repr(C)]`,
    /// verified length, and alignment-1 fields.
    #[allow(dead_code)]
    #[inline(always)]
    pub fn load_mut(account: &AccountView) -> Result<RefMut<'_, Self>, ProgramError> {
        if !account.owned_by(&crate::ID) {
            return Err(EscrowError::InvalidAccountOwner.into());
        }
        let data = account.try_borrow_mut()?;
        if data.len() != Self::LEN {
            return Err(EscrowError::InvalidEscrowState.into());
        }
        if data[0] != Self::DISCRIMINATOR {
            return Err(EscrowError::InvalidEscrowState.into());
        }
        Ok(RefMut::map(data, |d| unsafe {
            &mut *(d.as_mut_ptr() as *mut Self)
        }))
    }

    /// Initialize a freshly-created escrow account.
    /// Sets the discriminator and returns a mutable reference for field population.
    ///
    /// Re-initialization protection: fails if `data[0] != 0` (account already initialized).
    ///
    /// # Safety argument
    /// Same as `load` — the pointer cast is sound due to `#[repr(C)]`,
    /// verified length, and alignment-1 fields.
    #[inline(always)]
    pub fn init(account: &AccountView) -> Result<RefMut<'_, Self>, ProgramError> {
        if account.data_len() != Self::LEN {
            return Err(EscrowError::InvalidEscrowState.into());
        }
        let mut data = account.try_borrow_mut()?;
        if data[0] != 0 {
            return Err(EscrowError::InvalidEscrowState.into());
        }
        data[0] = Self::DISCRIMINATOR;
        Ok(RefMut::map(data, |d| unsafe {
            &mut *(d.as_mut_ptr() as *mut Self)
        }))
    }
}
