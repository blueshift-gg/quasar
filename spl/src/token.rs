use solana_address::Address;

#[repr(C)]
pub struct TokenAccountState {
    mint: Address,
    owner: Address,
    amount: [u8; 8],
    delegate_flag: [u8; 4],
    delegate: Address,
    state: u8,
    is_native: [u8; 4],
    native_amount: [u8; 8],
    delegated_amount: [u8; 8],
    close_authority_flag: [u8; 4],
    close_authority: Address,
}

impl TokenAccountState {
    pub const LEN: usize = core::mem::size_of::<TokenAccountState>();

    #[inline(always)]
    pub fn mint(&self) -> &Address {
        &self.mint
    }

    #[inline(always)]
    pub fn owner(&self) -> &Address {
        &self.owner
    }

    #[inline(always)]
    pub fn amount(&self) -> u64 {
        u64::from_le_bytes(self.amount)
    }

    #[inline(always)]
    pub fn has_delegate(&self) -> bool {
        self.delegate_flag[0] == 1
    }

    #[inline(always)]
    pub fn delegate(&self) -> Option<&Address> {
        if self.has_delegate() {
            Some(&self.delegate)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn delegate_unchecked(&self) -> &Address {
        &self.delegate
    }

    #[inline(always)]
    pub fn is_native(&self) -> bool {
        self.is_native[0] == 1
    }

    #[inline(always)]
    pub fn native_amount(&self) -> Option<u64> {
        if self.is_native() {
            Some(u64::from_le_bytes(self.native_amount))
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn delegated_amount(&self) -> u64 {
        u64::from_le_bytes(self.delegated_amount)
    }

    #[inline(always)]
    pub fn has_close_authority(&self) -> bool {
        self.close_authority_flag[0] == 1
    }

    #[inline(always)]
    pub fn close_authority(&self) -> Option<&Address> {
        if self.has_close_authority() {
            Some(&self.close_authority)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn close_authority_unchecked(&self) -> &Address {
        &self.close_authority
    }

    #[inline(always)]
    pub fn is_initialized(&self) -> bool {
        self.state != 0
    }

    #[inline(always)]
    pub fn is_frozen(&self) -> bool {
        self.state == 2
    }
}

const _ASSERT_TOKEN_ACCOUNT_LEN: () = assert!(TokenAccountState::LEN == 165);
