//! Zero-copy layout decode tests for the SPL token/mint state readers.
//!
//! One test per population state of each layout (fully populated, absent
//! options, state-byte matrix) asserting every accessor together, plus the
//! size pins. Adversarial/malformed-account rejection for these same types
//! lives in test_validate_token.rs / test_validate_mint.rs.

use {
    quasar_spl::{MintDataZc, TokenDataZc},
    solana_address::Address,
};

#[allow(clippy::too_many_arguments)]
fn build_token_account_bytes(
    mint: &Address,
    owner: &Address,
    amount: u64,
    delegate: Option<&Address>,
    state: u8,
    is_native: Option<u64>,
    delegated_amount: u64,
    close_authority: Option<&Address>,
) -> [u8; 165] {
    let mut data = [0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    if let Some(d) = delegate {
        data[72..76].copy_from_slice(&1u32.to_le_bytes());
        data[76..108].copy_from_slice(d.as_ref());
    }
    data[108] = state;
    if let Some(native_amount) = is_native {
        data[109..113].copy_from_slice(&1u32.to_le_bytes());
        data[113..121].copy_from_slice(&native_amount.to_le_bytes());
    }
    data[121..129].copy_from_slice(&delegated_amount.to_le_bytes());
    if let Some(ca) = close_authority {
        data[129..133].copy_from_slice(&1u32.to_le_bytes());
        data[133..165].copy_from_slice(ca.as_ref());
    }
    data
}

fn build_mint_account_bytes(
    mint_authority: Option<&Address>,
    supply: u64,
    decimals: u8,
    is_initialized: u8,
    freeze_authority: Option<&Address>,
) -> [u8; 82] {
    let mut data = [0u8; 82];
    if let Some(auth) = mint_authority {
        data[0..4].copy_from_slice(&1u32.to_le_bytes());
        data[4..36].copy_from_slice(auth.as_ref());
    }
    data[36..44].copy_from_slice(&supply.to_le_bytes());
    data[44] = decimals;
    data[45] = is_initialized;
    if let Some(freeze) = freeze_authority {
        data[46..50].copy_from_slice(&1u32.to_le_bytes());
        data[50..82].copy_from_slice(freeze.as_ref());
    }
    data
}

fn cast_token(data: &[u8; 165]) -> &TokenDataZc {
    // SAFETY: TokenDataZc is the zero-copy view of the exact 165-byte SPL token
    // layout and has byte alignment.
    unsafe { &*(data.as_ptr() as *const TokenDataZc) }
}

fn cast_mint(data: &[u8; 82]) -> &MintDataZc {
    // SAFETY: MintDataZc is the zero-copy view of the exact 82-byte SPL mint
    // layout and has byte alignment.
    unsafe { &*(data.as_ptr() as *const MintDataZc) }
}

#[test]
fn token_layout_decodes_every_field_when_fully_populated() {
    // Boundary values in every slot with all COptions Some: each accessor
    // must read exactly its own offsets even with maximal adjacent data.
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let delegate = Address::new_unique();
    let close_auth = Address::new_unique();
    let bytes = build_token_account_bytes(
        &mint,
        &owner,
        u64::MAX,
        Some(&delegate),
        2, // frozen
        Some(1_000_000),
        7_777,
        Some(&close_auth),
    );
    let state = cast_token(&bytes);

    assert_eq!(state.mint(), &mint);
    assert_eq!(state.owner(), &owner);
    assert_eq!(state.amount(), u64::MAX);
    assert_eq!(state.delegate(), Some(&delegate));
    assert!(state.is_initialized());
    assert!(state.is_frozen());
    assert!(state.native().is_some());
    assert_eq!(state.native_amount(), Some(1_000_000));
    assert_eq!(state.delegated_amount(), 7_777);
    assert_eq!(state.close_authority(), Some(&close_auth));
}

#[test]
fn token_layout_decodes_absent_options() {
    // All COptions None with zero amounts: every optional accessor must
    // report absence, not read stale bytes.
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let bytes = build_token_account_bytes(&mint, &owner, 0, None, 1, None, 0, None);
    let state = cast_token(&bytes);

    assert_eq!(state.mint(), &mint);
    assert_eq!(state.owner(), &owner);
    assert_eq!(state.amount(), 0);
    assert_eq!(state.delegate(), None);
    assert!(state.is_initialized());
    assert!(!state.is_frozen());
    assert!(state.native().is_none());
    assert_eq!(state.native_amount(), None);
    assert_eq!(state.delegated_amount(), 0);
    assert_eq!(state.close_authority(), None);
}

#[test]
fn token_layout_distinguishes_account_states() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    for (state_byte, initialized, frozen) in [(0, false, false), (1, true, false), (2, true, true)]
    {
        let bytes = build_token_account_bytes(&mint, &owner, 0, None, state_byte, None, 0, None);
        let state = cast_token(&bytes);
        assert_eq!(state.is_initialized(), initialized, "state {state_byte}");
        assert_eq!(state.is_frozen(), frozen, "state {state_byte}");
    }
}

#[test]
fn mint_layout_decodes_every_field_when_fully_populated() {
    let authority = Address::new_unique();
    let freeze = Address::new_unique();
    let bytes = build_mint_account_bytes(Some(&authority), u64::MAX, 255, 1, Some(&freeze));
    let state = cast_mint(&bytes);

    assert_eq!(state.mint_authority(), Some(&authority));
    assert_eq!(state.supply(), u64::MAX);
    assert_eq!(state.decimals(), 255);
    assert!(state.is_initialized());
    assert_eq!(state.freeze_authority(), Some(&freeze));
}

#[test]
fn mint_layout_decodes_absent_options() {
    let bytes = build_mint_account_bytes(None, 0, 0, 0, None);
    let state = cast_mint(&bytes);

    assert_eq!(state.mint_authority(), None);
    assert_eq!(state.supply(), 0);
    assert_eq!(state.decimals(), 0);
    assert!(!state.is_initialized());
    assert_eq!(state.freeze_authority(), None);
}

#[test]
fn test_token_account_state_len() {
    assert_eq!(core::mem::size_of::<TokenDataZc>(), 165);
}

#[test]
fn test_mint_account_state_len() {
    assert_eq!(core::mem::size_of::<MintDataZc>(), 82);
}
