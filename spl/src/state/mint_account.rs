use solana_address::Address;

/// Zero-copy layout for SPL Token mint accounts (82 bytes).
///
/// Fields use raw byte arrays for alignment-1 access. The layout is identical
/// for both SPL Token and Token-2022 (base mint data occupies the first 82
/// bytes).
///
/// ```text
/// offset  len  field
/// ──────  ───  ─────
///   0      4   mint_authority_flag   (COption<> tag)
///   4     32   mint_authority
///  36      8   supply               (u64 LE)
///  44      1   decimals
///  45      1   is_initialized       (bool)
///  46      4   freeze_authority_flag (COption<> tag)
///  50     32   freeze_authority
/// ──────  ───
/// total   82
/// ```
#[repr(C)]
pub struct MintAccountState {
    /// `COption` tag: 1 if mint authority is set.
    mint_authority_flag: [u8; 4],

    /// Mint authority (valid only when `mint_authority_flag[0] == 1`).
    mint_authority: Address,

    /// Total supply (little-endian u64).
    supply: [u8; 8],

    /// Number of base-10 digits to the right of the decimal place.
    decimals: u8,

    /// Whether the mint has been initialized (0 or 1).
    is_initialized: u8,

    /// `COption` tag: 1 if freeze authority is set.
    freeze_authority_flag: [u8; 4],

    /// Freeze authority (valid only when `freeze_authority_flag[0] == 1`).
    freeze_authority: Address,
}

impl MintAccountState {
    /// Total byte length of the mint account data.
    pub const LEN: usize = core::mem::size_of::<MintAccountState>();

    /// Whether a mint authority is set.
    #[inline(always)]
    pub fn has_mint_authority(&self) -> bool {
        self.mint_authority_flag[0] == 1
    }

    /// The mint authority, if any.
    #[inline(always)]
    pub fn mint_authority(&self) -> Option<&Address> {
        if self.has_mint_authority() {
            Some(&self.mint_authority)
        } else {
            None
        }
    }

    /// The mint authority address without checking the `COption` tag.
    #[inline(always)]
    pub fn mint_authority_unchecked(&self) -> &Address {
        &self.mint_authority
    }

    /// The total supply of tokens.
    #[inline(always)]
    pub fn supply(&self) -> u64 {
        u64::from_le_bytes(self.supply)
    }

    /// The number of decimals for this mint.
    #[inline(always)]
    pub fn decimals(&self) -> u8 {
        self.decimals
    }

    /// Whether the mint has been initialized.
    #[inline(always)]
    pub fn is_initialized(&self) -> bool {
        self.is_initialized != 0
    }

    /// Whether a freeze authority is set.
    #[inline(always)]
    pub fn has_freeze_authority(&self) -> bool {
        self.freeze_authority_flag[0] == 1
    }

    /// The freeze authority, if any.
    #[inline(always)]
    pub fn freeze_authority(&self) -> Option<&Address> {
        if self.has_freeze_authority() {
            Some(&self.freeze_authority)
        } else {
            None
        }
    }

    /// The freeze authority address without checking the `COption` tag.
    #[inline(always)]
    pub fn freeze_authority_unchecked(&self) -> &Address {
        &self.freeze_authority
    }
}

const _ASSERT_MINT_LEN: () = assert!(MintAccountState::LEN == 82);
const _ASSERT_MINT_ALIGN: () = assert!(core::mem::align_of::<MintAccountState>() == 1);
