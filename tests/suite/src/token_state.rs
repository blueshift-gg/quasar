use {
    quasar_spl::{MintAccountState, TokenAccountState},
    solana_address::Address,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn cast_token(data: &[u8; 165]) -> &TokenAccountState {
    unsafe { &*(data.as_ptr() as *const TokenAccountState) }
}

fn cast_mint(data: &[u8; 82]) -> &MintAccountState {
    unsafe { &*(data.as_ptr() as *const MintAccountState) }
}

// ---------------------------------------------------------------------------
// TokenAccountState tests
// ---------------------------------------------------------------------------

#[test]
fn test_token_state_mint() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let bytes = build_token_account_bytes(&mint, &owner, 0, None, 1, None, 0, None);
    let state = cast_token(&bytes);
    assert_eq!(state.mint(), &mint);
}

#[test]
fn test_token_state_owner() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let bytes = build_token_account_bytes(&mint, &owner, 0, None, 1, None, 0, None);
    let state = cast_token(&bytes);
    assert_eq!(state.owner(), &owner);
}

#[test]
fn test_token_state_amount() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let bytes = build_token_account_bytes(&mint, &owner, 123_456_789, None, 1, None, 0, None);
    let state = cast_token(&bytes);
    assert_eq!(state.amount(), 123_456_789);
}

#[test]
fn test_token_state_amount_max() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let bytes = build_token_account_bytes(&mint, &owner, u64::MAX, None, 1, None, 0, None);
    let state = cast_token(&bytes);
    assert_eq!(state.amount(), u64::MAX);
}

#[test]
fn test_token_state_amount_zero() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let bytes = build_token_account_bytes(&mint, &owner, 0, None, 1, None, 0, None);
    let state = cast_token(&bytes);
    assert_eq!(state.amount(), 0);
}

#[test]
fn test_token_state_delegate_present() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let delegate = Address::new_unique();
    let bytes = build_token_account_bytes(&mint, &owner, 1000, Some(&delegate), 1, None, 500, None);
    let state = cast_token(&bytes);
    assert!(state.has_delegate());
    assert_eq!(state.delegate(), Some(&delegate));
}

#[test]
fn test_token_state_delegate_absent() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let bytes = build_token_account_bytes(&mint, &owner, 1000, None, 1, None, 0, None);
    let state = cast_token(&bytes);
    assert!(!state.has_delegate());
    assert_eq!(state.delegate(), None);
}

#[test]
fn test_token_state_initialized() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let bytes = build_token_account_bytes(&mint, &owner, 0, None, 1, None, 0, None);
    let state = cast_token(&bytes);
    assert!(state.is_initialized());
    assert!(!state.is_frozen());
}

#[test]
fn test_token_state_frozen() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let bytes = build_token_account_bytes(&mint, &owner, 0, None, 2, None, 0, None);
    let state = cast_token(&bytes);
    assert!(state.is_initialized());
    assert!(state.is_frozen());
}

#[test]
fn test_token_state_uninitialized() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let bytes = build_token_account_bytes(&mint, &owner, 0, None, 0, None, 0, None);
    let state = cast_token(&bytes);
    assert!(!state.is_initialized());
    assert!(!state.is_frozen());
}

#[test]
fn test_token_state_native() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let bytes = build_token_account_bytes(&mint, &owner, 500, None, 1, Some(1_000_000), 0, None);
    let state = cast_token(&bytes);
    assert!(state.is_native());
    assert_eq!(state.native_amount(), Some(1_000_000));
}

#[test]
fn test_token_state_not_native() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let bytes = build_token_account_bytes(&mint, &owner, 500, None, 1, None, 0, None);
    let state = cast_token(&bytes);
    assert!(!state.is_native());
    assert_eq!(state.native_amount(), None);
}

#[test]
fn test_token_state_delegated_amount() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let delegate = Address::new_unique();
    let bytes =
        build_token_account_bytes(&mint, &owner, 10_000, Some(&delegate), 1, None, 7_777, None);
    let state = cast_token(&bytes);
    assert_eq!(state.delegated_amount(), 7_777);
}

#[test]
fn test_token_state_close_authority_present() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let close_auth = Address::new_unique();
    let bytes = build_token_account_bytes(&mint, &owner, 0, None, 1, None, 0, Some(&close_auth));
    let state = cast_token(&bytes);
    assert!(state.has_close_authority());
    assert_eq!(state.close_authority(), Some(&close_auth));
}

#[test]
fn test_token_state_close_authority_absent() {
    let mint = Address::new_unique();
    let owner = Address::new_unique();
    let bytes = build_token_account_bytes(&mint, &owner, 0, None, 1, None, 0, None);
    let state = cast_token(&bytes);
    assert!(!state.has_close_authority());
    assert_eq!(state.close_authority(), None);
}

// ---------------------------------------------------------------------------
// MintAccountState tests
// ---------------------------------------------------------------------------

#[test]
fn test_mint_state_has_authority() {
    let authority = Address::new_unique();
    let bytes = build_mint_account_bytes(Some(&authority), 1_000_000, 9, 1, None);
    let state = cast_mint(&bytes);
    assert!(state.has_mint_authority());
    assert_eq!(state.mint_authority(), Some(&authority));
}

#[test]
fn test_mint_state_no_authority() {
    let bytes = build_mint_account_bytes(None, 1_000_000, 9, 1, None);
    let state = cast_mint(&bytes);
    assert!(!state.has_mint_authority());
    assert_eq!(state.mint_authority(), None);
}

#[test]
fn test_mint_state_supply() {
    let authority = Address::new_unique();
    let bytes = build_mint_account_bytes(Some(&authority), 42_000_000_000, 9, 1, None);
    let state = cast_mint(&bytes);
    assert_eq!(state.supply(), 42_000_000_000);
}

#[test]
fn test_mint_state_decimals() {
    let authority = Address::new_unique();
    let bytes = build_mint_account_bytes(Some(&authority), 0, 6, 1, None);
    let state = cast_mint(&bytes);
    assert_eq!(state.decimals(), 6);
}

#[test]
fn test_mint_state_initialized() {
    let authority = Address::new_unique();
    let bytes = build_mint_account_bytes(Some(&authority), 0, 9, 1, None);
    let state = cast_mint(&bytes);
    assert!(state.is_initialized());
}

#[test]
fn test_mint_state_uninitialized() {
    let bytes = build_mint_account_bytes(None, 0, 0, 0, None);
    let state = cast_mint(&bytes);
    assert!(!state.is_initialized());
}

#[test]
fn test_mint_state_freeze_authority_present() {
    let authority = Address::new_unique();
    let freeze = Address::new_unique();
    let bytes = build_mint_account_bytes(Some(&authority), 0, 9, 1, Some(&freeze));
    let state = cast_mint(&bytes);
    assert!(state.has_freeze_authority());
    assert_eq!(state.freeze_authority(), Some(&freeze));
}

#[test]
fn test_mint_state_freeze_authority_absent() {
    let authority = Address::new_unique();
    let bytes = build_mint_account_bytes(Some(&authority), 0, 9, 1, None);
    let state = cast_mint(&bytes);
    assert!(!state.has_freeze_authority());
    assert_eq!(state.freeze_authority(), None);
}

#[test]
fn test_mint_state_zero_supply() {
    let authority = Address::new_unique();
    let bytes = build_mint_account_bytes(Some(&authority), 0, 9, 1, None);
    let state = cast_mint(&bytes);
    assert_eq!(state.supply(), 0);
}

#[test]
fn test_mint_state_max_supply() {
    let authority = Address::new_unique();
    let bytes = build_mint_account_bytes(Some(&authority), u64::MAX, 9, 1, None);
    let state = cast_mint(&bytes);
    assert_eq!(state.supply(), u64::MAX);
}

#[test]
fn test_mint_state_max_decimals() {
    let authority = Address::new_unique();
    let bytes = build_mint_account_bytes(Some(&authority), 0, 255, 1, None);
    let state = cast_mint(&bytes);
    assert_eq!(state.decimals(), 255);
}

// ---------------------------------------------------------------------------
// Layout size assertions
// ---------------------------------------------------------------------------

#[test]
fn test_token_account_state_len() {
    assert_eq!(TokenAccountState::LEN, 165);
}

#[test]
fn test_mint_account_state_len() {
    assert_eq!(MintAccountState::LEN, 82);
}
