//! Exact-size and unaligned SPL layouts, exercised by both Miri modes.

use quasar_lang::{
    Address,
    account::{InterfaceKind, Kind},
    zeropod::{self, Layout},
};
use quasar_spl::token::{COption, Mint, TokenAccount};

#[test]
fn token_views_roundtrip_optional_fields_at_every_alignment() {
    for offset in 0..8 {
        // No spare bytes after the encoding: an overread reaches the allocation end.
        let mut allocation = vec![0; offset + 165];
        let bytes = &mut allocation[offset..];
        for present in [true, false, true] {
            let address = Address::new_from_array([0xa5; 32]);
            let token = TokenAccount {
                mint: address,
                owner: address,
                amount: u64::MAX,
                delegate: COption(present.then_some(address)),
                state: 1,
                is_native: COption(present.then_some(u64::MAX)),
                delegated_amount: 42,
                close_authority: COption(present.then_some(address)),
            };
            assert_eq!(zeropod::write(&token, bytes).unwrap(), 165);
            let view = TokenAccount::view(bytes).unwrap();
            assert_eq!(view.amount(), u64::MAX);
            assert_eq!(view.delegate().copied(), token.delegate.0);
            assert_eq!(view.is_native(), token.is_native.0);
            assert_eq!(view.close_authority().copied(), token.close_authority.0);
            assert!(<TokenAccount as Kind>::validate(view));
            assert!(<TokenAccount as InterfaceKind>::validate(bytes));
        }
        for length in 0..165 {
            assert!(TokenAccount::view(&bytes[..length]).is_err());
            assert!(!<TokenAccount as InterfaceKind>::validate(&bytes[..length]));
        }
        bytes[72..76].copy_from_slice(&2u32.to_le_bytes());
        assert!(TokenAccount::view(bytes).is_err());
    }
}

#[test]
fn mint_views_reject_invalid_tags_and_flags_at_every_alignment() {
    for offset in 0..8 {
        let mut allocation = vec![0; offset + 82];
        let bytes = &mut allocation[offset..];
        for present in [true, false] {
            let mint = Mint {
                mint_authority: COption(present.then_some(Address::new_from_array([7; 32]))),
                supply: u64::MAX,
                decimals: 9,
                is_initialized: true,
                freeze_authority: COption(None),
            };
            assert_eq!(zeropod::write(&mint, bytes).unwrap(), 82);
            let view = Mint::view(bytes).unwrap();
            assert_eq!(view.supply(), u64::MAX);
            assert_eq!(view.mint_authority().copied(), mint.mint_authority.0);
            assert!(<Mint as InterfaceKind>::validate(bytes));
        }
        for length in 0..82 {
            assert!(Mint::view(&bytes[..length]).is_err());
            assert!(!<Mint as InterfaceKind>::validate(&bytes[..length]));
        }
        bytes[45] = 2;
        assert!(Mint::view(bytes).is_err());
        bytes[45] = 1;
        bytes[46..50].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Mint::view(bytes).is_err());
    }
}
