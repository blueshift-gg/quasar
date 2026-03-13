use solana_address::Address;

/// Zero-copy layout for SPL Token accounts (165 bytes).
///
/// Fields use raw byte arrays for alignment-1 access. The layout is identical
/// for both SPL Token and Token-2022 (base token data occupies the first 165
/// bytes).
///
/// ```text
/// offset  len  field
/// ──────  ───  ─────
///   0     32   mint
///  32     32   owner
///  64      8   amount              (u64 LE)
///  72      4   delegate_flag       (COption<> tag)
///  76     32   delegate
/// 108      1   state               (0=uninitialized, 1=initialized, 2=frozen)
/// 109      4   is_native           (COption<> tag)
/// 113      8   native_amount       (u64 LE)
/// 121      8   delegated_amount    (u64 LE)
/// 129      4   close_authority_flag (COption<> tag)
/// 133     32   close_authority
/// ──────  ───
/// total  165
/// ```
#[repr(C)]
pub struct TokenAccountState {
    /// Mint associated with this token account.
    mint: Address,

    /// Owner of this token account.
    owner: Address,

    /// Token balance (little-endian u64).
    amount: [u8; 8],

    /// `COption` tag: 1 if delegate is set.
    delegate_flag: [u8; 4],

    /// Approved delegate (valid only when `delegate_flag[0] == 1`).
    delegate: Address,

    /// Account state: 0 = uninitialized, 1 = initialized, 2 = frozen.
    state: u8,

    /// `COption` tag: 1 if this is a native SOL token account.
    is_native: [u8; 4],

    /// Native SOL amount (valid only when `is_native[0] == 1`).
    native_amount: [u8; 8],

    /// Amount currently delegated (little-endian u64).
    delegated_amount: [u8; 8],

    /// `COption` tag: 1 if close authority is set.
    close_authority_flag: [u8; 4],

    /// Close authority (valid only when `close_authority_flag[0] == 1`).
    close_authority: Address,
}

impl TokenAccountState {
    /// Total byte length of the token account data.
    pub const LEN: usize = core::mem::size_of::<TokenAccountState>();

    /// The mint associated with this token account.
    #[inline(always)]
    pub fn mint(&self) -> &Address {
        &self.mint
    }

    /// The owner of this token account.
    #[inline(always)]
    pub fn owner(&self) -> &Address {
        &self.owner
    }

    /// The token balance.
    #[inline(always)]
    pub fn amount(&self) -> u64 {
        u64::from_le_bytes(self.amount)
    }

    /// Whether a delegate is currently approved.
    #[inline(always)]
    pub fn has_delegate(&self) -> bool {
        self.delegate_flag[0] == 1
    }

    /// The approved delegate, if any.
    #[inline(always)]
    pub fn delegate(&self) -> Option<&Address> {
        if self.has_delegate() {
            Some(&self.delegate)
        } else {
            None
        }
    }

    /// The delegate address without checking the `COption` tag.
    #[inline(always)]
    pub fn delegate_unchecked(&self) -> &Address {
        &self.delegate
    }

    /// Whether this is a native SOL token account.
    #[inline(always)]
    pub fn is_native(&self) -> bool {
        self.is_native[0] == 1
    }

    /// The native SOL amount, if this is a native token account.
    #[inline(always)]
    pub fn native_amount(&self) -> Option<u64> {
        if self.is_native() {
            Some(u64::from_le_bytes(self.native_amount))
        } else {
            None
        }
    }

    /// The amount currently delegated.
    #[inline(always)]
    pub fn delegated_amount(&self) -> u64 {
        u64::from_le_bytes(self.delegated_amount)
    }

    /// Whether a close authority is set.
    #[inline(always)]
    pub fn has_close_authority(&self) -> bool {
        self.close_authority_flag[0] == 1
    }

    /// The close authority, if any.
    #[inline(always)]
    pub fn close_authority(&self) -> Option<&Address> {
        if self.has_close_authority() {
            Some(&self.close_authority)
        } else {
            None
        }
    }

    /// The close authority address without checking the `COption` tag.
    #[inline(always)]
    pub fn close_authority_unchecked(&self) -> &Address {
        &self.close_authority
    }

    /// Whether the account has been initialized (state ≠ 0).
    #[inline(always)]
    pub fn is_initialized(&self) -> bool {
        self.state != 0
    }

    /// Whether the account is frozen (state == 2).
    #[inline(always)]
    pub fn is_frozen(&self) -> bool {
        self.state == 2
    }
}

const _ASSERT_TOKEN_ACCOUNT_LEN: () = assert!(TokenAccountState::LEN == 165);
const _ASSERT_TOKEN_ACCOUNT_ALIGN: () = assert!(core::mem::align_of::<TokenAccountState>() == 1);
