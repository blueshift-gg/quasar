//! Zero-copy layouts for SPL Token account state.
//!
//! Provides [`COption<T>`], [`TokenAccountState`], and [`MintAccountState`] —
//! alignment-1 structs that can be cast directly from on-chain account data
//! without copying. The layouts are identical for both SPL Token and
//! Token-2022.

use solana_address::Address;

mod sealed {
    pub trait Sealed {}

    impl Sealed for solana_address::Address {}
    impl Sealed for [u8; 8] {}
}

/// Payload types that are valid for [`COption`].
///
/// `COption` is read directly from zero-copy account bytes, so its payload type
/// must be valid for every possible bit pattern that may appear in the value
/// slot even when the tag is `None`.
pub trait COptionValue: sealed::Sealed {}

impl COptionValue for Address {}
impl COptionValue for [u8; 8] {}

// ---------------------------------------------------------------------------
// COption<T>
// ---------------------------------------------------------------------------

/// SPL Token `COption` — a C-compatible optional value.
///
/// Mirrors the `COption` layout used by the SPL Token program:
/// a 4-byte little-endian tag (0 = None, 1 = Some) followed by
/// the value. The value bytes are always present regardless of the tag.
#[repr(C)]
pub struct COption<T: COptionValue> {
    tag: [u8; 4],
    value: T,
}

impl<T: COptionValue> COption<T> {
    /// Whether the option contains a value.
    ///
    /// Checks only `tag[0] == 1` (the low byte of the LE u32 tag).
    /// This matches the SPL Token program's COption encoding where
    /// `0u32 = None` and `1u32 = Some`.
    #[inline(always)]
    pub fn is_some(&self) -> bool {
        self.tag[0] == 1
    }

    /// Whether the option is empty (`tag != 1`).
    #[inline(always)]
    pub fn is_none(&self) -> bool {
        self.tag[0] != 1
    }

    /// Returns a reference to the value if present.
    #[inline(always)]
    pub fn get(&self) -> Option<&T> {
        if self.is_some() {
            Some(&self.value)
        } else {
            None
        }
    }

    /// Returns a reference to the value without checking the tag.
    ///
    /// The returned reference is always valid (the bytes exist
    /// regardless of the tag), but the value may be uninitialized
    /// or stale when `is_some()` is false.
    #[inline(always)]
    pub fn get_unchecked(&self) -> &T {
        &self.value
    }
}

// ---------------------------------------------------------------------------
// TokenAccountState
// ---------------------------------------------------------------------------

/// Zero-copy layout for SPL Token accounts (165 bytes).
///
/// Fields use raw byte arrays for alignment-1 access. The layout is identical
/// for both SPL Token and Token-2022 (base token data occupies the first 165
/// bytes).
#[repr(C)]
pub struct TokenAccountState {
    /// Mint associated with this token account.
    mint: Address,

    /// Owner of this token account.
    owner: Address,

    /// Token balance (little-endian u64).
    amount: [u8; 8],

    /// Approved delegate, wrapped in a `COption`.
    delegate: COption<Address>,

    /// Account state: 0 = uninitialized, 1 = initialized, 2 = frozen.
    state: u8,

    /// Native SOL amount, wrapped in a `COption` over a raw LE u64.
    native: COption<[u8; 8]>,

    /// Amount currently delegated (little-endian u64).
    delegated_amount: [u8; 8],

    /// Close authority, wrapped in a `COption`.
    close_authority: COption<Address>,
}

impl TokenAccountState {
    /// Total byte length of the token account data (165).
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
        self.delegate.is_some()
    }

    /// The approved delegate, if any.
    #[inline(always)]
    pub fn delegate(&self) -> Option<&Address> {
        self.delegate.get()
    }

    /// The delegate address without checking the `COption` tag.
    #[inline(always)]
    pub fn delegate_unchecked(&self) -> &Address {
        self.delegate.get_unchecked()
    }

    /// Whether this is a native SOL token account.
    #[inline(always)]
    pub fn is_native(&self) -> bool {
        self.native.is_some()
    }

    /// The native SOL amount, if this is a native token account.
    #[inline(always)]
    pub fn native_amount(&self) -> Option<u64> {
        if self.native.is_some() {
            Some(u64::from_le_bytes(*self.native.get_unchecked()))
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
        self.close_authority.is_some()
    }

    /// The close authority, if any.
    #[inline(always)]
    pub fn close_authority(&self) -> Option<&Address> {
        self.close_authority.get()
    }

    /// The close authority address without checking the `COption` tag.
    #[inline(always)]
    pub fn close_authority_unchecked(&self) -> &Address {
        self.close_authority.get_unchecked()
    }

    /// Whether the account has been initialized (state != 0).
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

const _: () = assert!(TokenAccountState::LEN == 165);
const _: () = assert!(core::mem::align_of::<TokenAccountState>() == 1);

// ---------------------------------------------------------------------------
// MintAccountState
// ---------------------------------------------------------------------------

/// Zero-copy layout for SPL Token mint accounts (82 bytes).
///
/// Fields use raw byte arrays for alignment-1 access. The layout is identical
/// for both SPL Token and Token-2022 (base mint data occupies the first 82
/// bytes).
#[repr(C)]
pub struct MintAccountState {
    /// Mint authority, wrapped in a `COption`.
    mint_authority: COption<Address>,

    /// Total supply (little-endian u64).
    supply: [u8; 8],

    /// Number of base-10 digits to the right of the decimal place.
    decimals: u8,

    /// Whether the mint has been initialized (0 or 1).
    is_initialized: u8,

    /// Freeze authority, wrapped in a `COption`.
    freeze_authority: COption<Address>,
}

impl MintAccountState {
    /// Total byte length of the mint account data (82).
    pub const LEN: usize = core::mem::size_of::<MintAccountState>();

    /// Whether a mint authority is set.
    #[inline(always)]
    pub fn has_mint_authority(&self) -> bool {
        self.mint_authority.is_some()
    }

    /// The mint authority, if any.
    #[inline(always)]
    pub fn mint_authority(&self) -> Option<&Address> {
        self.mint_authority.get()
    }

    /// The mint authority address without checking the `COption` tag.
    #[inline(always)]
    pub fn mint_authority_unchecked(&self) -> &Address {
        self.mint_authority.get_unchecked()
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
        self.freeze_authority.is_some()
    }

    /// The freeze authority, if any.
    #[inline(always)]
    pub fn freeze_authority(&self) -> Option<&Address> {
        self.freeze_authority.get()
    }

    /// The freeze authority address without checking the `COption` tag.
    #[inline(always)]
    pub fn freeze_authority_unchecked(&self) -> &Address {
        self.freeze_authority.get_unchecked()
    }
}

const _: () = assert!(MintAccountState::LEN == 82);
const _: () = assert!(core::mem::align_of::<MintAccountState>() == 1);

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Prove TokenAccountState has alignment 1 (safe for pointer cast from
    /// arbitrary account data).
    #[kani::proof]
    fn token_account_state_align_one() {
        assert!(core::mem::align_of::<TokenAccountState>() == 1);
    }

    /// Prove TokenAccountState is exactly 165 bytes (SPL Token spec).
    #[kani::proof]
    fn token_account_state_size_165() {
        assert!(core::mem::size_of::<TokenAccountState>() == 165);
    }

    /// Prove TokenAccountState field offsets match the SPL Token layout.
    /// This ensures the pointer cast from raw account data produces correct
    /// field references.
    #[kani::proof]
    fn token_account_state_field_offsets() {
        assert!(core::mem::offset_of!(TokenAccountState, mint) == 0);
        assert!(core::mem::offset_of!(TokenAccountState, owner) == 32);
        assert!(core::mem::offset_of!(TokenAccountState, amount) == 64);
        assert!(core::mem::offset_of!(TokenAccountState, delegate) == 72);
        assert!(core::mem::offset_of!(TokenAccountState, state) == 108);
        assert!(core::mem::offset_of!(TokenAccountState, native) == 109);
        assert!(core::mem::offset_of!(TokenAccountState, delegated_amount) == 121);
        assert!(core::mem::offset_of!(TokenAccountState, close_authority) == 129);
    }

    /// Prove MintAccountState has alignment 1 (safe for pointer cast).
    #[kani::proof]
    fn mint_account_state_align_one() {
        assert!(core::mem::align_of::<MintAccountState>() == 1);
    }

    /// Prove MintAccountState is exactly 82 bytes (SPL Token spec).
    #[kani::proof]
    fn mint_account_state_size_82() {
        assert!(core::mem::size_of::<MintAccountState>() == 82);
    }

    /// Prove MintAccountState field offsets match the SPL Token layout.
    #[kani::proof]
    fn mint_account_state_field_offsets() {
        assert!(core::mem::offset_of!(MintAccountState, mint_authority) == 0);
        assert!(core::mem::offset_of!(MintAccountState, supply) == 36);
        assert!(core::mem::offset_of!(MintAccountState, decimals) == 44);
        assert!(core::mem::offset_of!(MintAccountState, is_initialized) == 45);
        assert!(core::mem::offset_of!(MintAccountState, freeze_authority) == 46);
    }

    /// Prove COption<Address> is exactly 36 bytes (4-byte tag + 32-byte value).
    #[kani::proof]
    fn coption_address_size() {
        assert!(core::mem::size_of::<COption<Address>>() == 36);
        assert!(core::mem::align_of::<COption<Address>>() == 1);
    }

    /// Prove COption tag interpretation: tag[0]==1 is Some, anything else is
    /// None.
    #[kani::proof]
    fn coption_tag_semantics() {
        let tag_byte: u8 = kani::any();
        let tag = [tag_byte, 0, 0, 0];
        let value = Address::new_from_array([0u8; 32]);
        // SAFETY: COption is repr(C) with tag + value, alignment 1.
        // Construct via raw bytes to test tag interpretation.
        let opt = COption { tag, value };
        assert!(opt.is_some() == (tag_byte == 1));
        assert!(opt.is_none() == (tag_byte != 1));
    }
}
