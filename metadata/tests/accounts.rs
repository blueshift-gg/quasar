mod support;

use quasar_lang::{
    ProgramError,
    account::InterfaceKind,
    wrapper::{UncheckedAccount, Wrapper},
    zeropod::Layout,
};
use quasar_metadata::{
    METADATA_PROGRAM_ID, MasterEdition, MasterEditionAccount, Metadata, MetadataAccount, pda,
    validate,
};
use support::{key, with_accounts};

#[test]
fn metadata_prefix_allows_trailing_fields_but_checks_every_boundary() {
    for offset in 0..8 {
        let mut allocation = vec![0; offset + 65];
        let data = &mut allocation[offset..];
        data[0] = 4;
        data[1..33].fill(1);
        data[33..65].fill(2);
        assert!(Metadata::validate(data));
        let view = Metadata::view(data).unwrap();
        assert_eq!(view.update_authority(), &key(1));
        assert_eq!(view.mint(), &key(2));
        for len in 0..65 {
            assert!(!Metadata::validate(&data[..len]));
        }
        data[0] = 6;
        assert!(!Metadata::validate(data));
    }
    let mut data = vec![0; 679];
    data[0] = 4;
    with_accounts(&[(key(1), METADATA_PROGRAM_ID, &data)], |mut cursor| {
        let account = cursor.load::<MetadataAccount>(false).unwrap();
        assert_eq!(account.key(), 4);
        assert_eq!(account.data().len(), 679);
    });
}

#[test]
fn edition_prefix_accepts_borsh_options_and_rejects_short_or_invalid_tags() {
    for offset in 0..8 {
        for some in [false, true] {
            let len = if some { 18 } else { 10 };
            let mut allocation = vec![0; offset + len];
            let data = &mut allocation[offset..];
            data[0] = 6;
            data[1..9].copy_from_slice(&u64::MAX.to_le_bytes());
            data[9] = u8::from(some);
            if some {
                data[10..].copy_from_slice(&42u64.to_le_bytes());
            }
            assert!(MasterEdition::validate(data));
            let view = MasterEdition::view(data).unwrap();
            assert_eq!(view.supply(), u64::MAX);
            assert_eq!(view.max_supply(), some.then_some(42));
            for short in 0..len {
                assert!(!MasterEdition::validate(&data[..short]));
            }
            with_accounts(&[(key(1), METADATA_PROGRAM_ID, data)], |mut cursor| {
                let account = cursor.load::<MasterEditionAccount>(false).unwrap();
                assert_eq!(account.max_supply(), some.then_some(42));
            });
            data[9] = 2;
            assert!(!MasterEdition::validate(data));
        }
    }
}

#[test]
fn account_loads_and_validation_check_owner_mint_and_authority() {
    let mut data = [0; 65];
    data[0] = 4;
    data[1..33].fill(1);
    data[33..65].fill(2);
    for owner in [key(0), METADATA_PROGRAM_ID] {
        with_accounts(&[(key(3), owner, &data)], |mut cursor| {
            assert_eq!(cursor.load::<MetadataAccount>(false).is_ok(), owner == METADATA_PROGRAM_ID);
        });
        with_accounts(&[(key(3), owner, &data)], |mut cursor| {
            let account = cursor.load::<UncheckedAccount>(false).unwrap();
            let entry = account.entry();
            let expected =
                if owner == METADATA_PROGRAM_ID { Ok(()) } else { Err(ProgramError::IllegalOwner) };
            assert_eq!(
                validate::validate_metadata_account(entry, &key(2), Some(&key(1))),
                expected
            );
            assert!(validate::validate_metadata_account(entry, &key(4), None).is_err());
            assert!(validate::validate_metadata_account(entry, &key(2), Some(&key(4))).is_err());
        });
    }
}

#[test]
fn canonical_pdas_match_metaplex_and_reject_other_mints() {
    for mint in [key(1), key(2)] {
        let (metadata, bump) = pda::metadata_address(&mint).unwrap();
        let expected = mpl_token_metadata::accounts::Metadata::find_pda(&mint.to_bytes().into());
        assert_eq!((metadata.to_bytes(), bump), (expected.0.to_bytes(), expected.1));
        assert!(pda::verify_metadata_address(&metadata, &mint).is_ok());
        assert!(pda::verify_metadata_address(&metadata, &key(3)).is_err());
        let (edition, bump) = pda::master_edition_address(&mint).unwrap();
        let expected =
            mpl_token_metadata::accounts::MasterEdition::find_pda(&mint.to_bytes().into());
        assert_eq!((edition.to_bytes(), bump), (expected.0.to_bytes(), expected.1));
        assert!(pda::verify_master_edition_address(&edition, &mint).is_ok());
        assert!(pda::verify_master_edition_address(&edition, &key(3)).is_err());
    }
}
