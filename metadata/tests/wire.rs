//! Compare every restored instruction with the independently implemented SDK.
mod support;
use mpl_token_metadata::instructions as sdk;
use quasar_lang::wrapper::{Program, UncheckedAccount};
use quasar_metadata::{
    METADATA_PROGRAM_ID, MetadataProgram,
    instructions::{self, Data},
};
use support::{key, with_accounts};

#[test]
fn approve_collection_authority_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..8 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut collection_authority_record = cursor.load::<UncheckedAccount>(true).unwrap();
        let new_collection_authority = cursor.load::<UncheckedAccount>(false).unwrap();
        let mut update_authority = cursor.load::<UncheckedAccount>(true).unwrap();
        let payer = cursor.load::<UncheckedAccount>(true).unwrap();
        let metadata = cursor.load::<UncheckedAccount>(false).unwrap();
        let mint = cursor.load::<UncheckedAccount>(false).unwrap();
        let system_program = cursor.load::<UncheckedAccount>(false).unwrap();
        let rent = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::approve_collection_authority(
            &program,
            &mut collection_authority_record,
            &new_collection_authority,
            &mut update_authority,
            &payer,
            &metadata,
            &mint,
            &system_program,
            &rent,
        )
        .instruction();
        let expected = sdk::ApproveCollectionAuthority {
            collection_authority_record: key(1).to_bytes().into(),
            new_collection_authority: key(2).to_bytes().into(),
            update_authority: key(3).to_bytes().into(),
            payer: key(4).to_bytes().into(),
            metadata: key(5).to_bytes().into(),
            mint: key(6).to_bytes().into(),
            system_program: key(7).to_bytes().into(),
            rent: Some(key(8).to_bytes().into()),
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn burn_nft_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..6 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut owner = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut mint = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut token_account = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut master_edition_account = cursor.load::<UncheckedAccount>(true).unwrap();
        let spl_token_program = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::burn_nft(
            &program,
            &mut metadata,
            &mut owner,
            &mut mint,
            &mut token_account,
            &mut master_edition_account,
            &spl_token_program,
        )
        .instruction();
        let expected = sdk::BurnNft {
            metadata: key(1).to_bytes().into(),
            owner: key(2).to_bytes().into(),
            mint: key(3).to_bytes().into(),
            token_account: key(4).to_bytes().into(),
            master_edition_account: key(5).to_bytes().into(),
            spl_token_program: key(6).to_bytes().into(),
            collection_metadata: None,
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn burn_edition_nft_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..10 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut owner = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut print_edition_mint = cursor.load::<UncheckedAccount>(true).unwrap();
        let master_edition_mint = cursor.load::<UncheckedAccount>(false).unwrap();
        let mut print_edition_token_account = cursor.load::<UncheckedAccount>(true).unwrap();
        let master_edition_token_account = cursor.load::<UncheckedAccount>(false).unwrap();
        let mut master_edition_account = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut print_edition_account = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut edition_marker_account = cursor.load::<UncheckedAccount>(true).unwrap();
        let spl_token_program = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::burn_edition_nft(
            &program,
            &mut metadata,
            &mut owner,
            &mut print_edition_mint,
            &master_edition_mint,
            &mut print_edition_token_account,
            &master_edition_token_account,
            &mut master_edition_account,
            &mut print_edition_account,
            &mut edition_marker_account,
            &spl_token_program,
        )
        .instruction();
        let expected = sdk::BurnEditionNft {
            metadata: key(1).to_bytes().into(),
            owner: key(2).to_bytes().into(),
            print_edition_mint: key(3).to_bytes().into(),
            master_edition_mint: key(4).to_bytes().into(),
            print_edition_token_account: key(5).to_bytes().into(),
            master_edition_token_account: key(6).to_bytes().into(),
            master_edition_account: key(7).to_bytes().into(),
            print_edition_account: key(8).to_bytes().into(),
            edition_marker_account: key(9).to_bytes().into(),
            spl_token_program: key(10).to_bytes().into(),
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn create_master_edition_v3_matches_metaplex() {
    for present in [false, true] {
        let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
        for i in 0..9 {
            specs.push((key(i + 1), key(0), &[]));
        }
        with_accounts(&specs, |mut cursor| {
            let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
            let mut edition = cursor.load::<UncheckedAccount>(true).unwrap();
            let mut mint = cursor.load::<UncheckedAccount>(true).unwrap();
            let update_authority = cursor.load::<UncheckedAccount>(false).unwrap();
            let mint_authority = cursor.load::<UncheckedAccount>(false).unwrap();
            let payer = cursor.load::<UncheckedAccount>(true).unwrap();
            let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
            let token_program = cursor.load::<UncheckedAccount>(false).unwrap();
            let system_program = cursor.load::<UncheckedAccount>(false).unwrap();
            let rent = cursor.load::<UncheckedAccount>(false).unwrap();
            let actual = instructions::create_master_edition_v3(
                &program,
                &mut edition,
                &mut mint,
                &update_authority,
                &mint_authority,
                &payer,
                &mut metadata,
                &token_program,
                &system_program,
                &rent,
                present.then_some(u64::MAX),
            )
            .unwrap()
            .instruction();
            let expected = sdk::CreateMasterEditionV3 {
                edition: key(1).to_bytes().into(),
                mint: key(2).to_bytes().into(),
                update_authority: key(3).to_bytes().into(),
                mint_authority: key(4).to_bytes().into(),
                payer: key(5).to_bytes().into(),
                metadata: key(6).to_bytes().into(),
                token_program: key(7).to_bytes().into(),
                system_program: key(8).to_bytes().into(),
                rent: Some(key(9).to_bytes().into()),
            }
            .instruction(sdk::CreateMasterEditionV3InstructionArgs {
                max_supply: present.then_some(u64::MAX),
            });
            assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
            assert_eq!(actual.data, expected.data);
            assert_eq!(actual.accounts.len(), expected.accounts.len());
            for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
                assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
                assert_eq!(actual.is_signer, expected.is_signer);
                assert_eq!(actual.is_writable, expected.is_writable);
            }
        });
    }
}

#[test]
fn create_metadata_accounts_v3_matches_metaplex() {
    for present in [false, true] {
        let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
        for i in 0..7 {
            specs.push((key(i + 1), key(0), &[]));
        }
        with_accounts(&specs, |mut cursor| {
            let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
            let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
            let mint = cursor.load::<UncheckedAccount>(false).unwrap();
            let mint_authority = cursor.load::<UncheckedAccount>(false).unwrap();
            let payer = cursor.load::<UncheckedAccount>(true).unwrap();
            let update_authority = cursor.load::<UncheckedAccount>(false).unwrap();
            let system_program = cursor.load::<UncheckedAccount>(false).unwrap();
            let rent = cursor.load::<UncheckedAccount>(false).unwrap();
            let actual = instructions::create_metadata_accounts_v3(
                &program,
                &mut metadata,
                &mint,
                &mint_authority,
                &payer,
                &update_authority,
                &system_program,
                &rent,
                Data {
                    name: "é",
                    symbol: "NFT",
                    uri: "https://example.com/metadata.json",
                    seller_fee_basis_points: 500,
                },
                present,
                present,
            )
            .unwrap()
            .instruction();
            let expected = sdk::CreateMetadataAccountV3 {
                metadata: key(1).to_bytes().into(),
                mint: key(2).to_bytes().into(),
                mint_authority: key(3).to_bytes().into(),
                payer: key(4).to_bytes().into(),
                update_authority: (key(5).to_bytes().into(), present),
                system_program: key(6).to_bytes().into(),
                rent: Some(key(7).to_bytes().into()),
            }
            .instruction(sdk::CreateMetadataAccountV3InstructionArgs {
                data: data(),
                is_mutable: present,
                collection_details: None,
            });
            assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
            assert_eq!(actual.data, expected.data);
            assert_eq!(actual.accounts.len(), expected.accounts.len());
            for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
                assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
                assert_eq!(actual.is_signer, expected.is_signer);
                assert_eq!(actual.is_writable, expected.is_writable);
            }
        });
    }
}

#[test]
fn freeze_delegated_account_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..5 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut delegate = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut token_account = cursor.load::<UncheckedAccount>(true).unwrap();
        let edition = cursor.load::<UncheckedAccount>(false).unwrap();
        let mint = cursor.load::<UncheckedAccount>(false).unwrap();
        let token_program = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::freeze_delegated_account(
            &program,
            &mut delegate,
            &mut token_account,
            &edition,
            &mint,
            &token_program,
        )
        .instruction();
        let expected = sdk::FreezeDelegatedAccount {
            delegate: key(1).to_bytes().into(),
            token_account: key(2).to_bytes().into(),
            edition: key(3).to_bytes().into(),
            mint: key(4).to_bytes().into(),
            token_program: key(5).to_bytes().into(),
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn thaw_delegated_account_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..5 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut delegate = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut token_account = cursor.load::<UncheckedAccount>(true).unwrap();
        let edition = cursor.load::<UncheckedAccount>(false).unwrap();
        let mint = cursor.load::<UncheckedAccount>(false).unwrap();
        let token_program = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::thaw_delegated_account(
            &program,
            &mut delegate,
            &mut token_account,
            &edition,
            &mint,
            &token_program,
        )
        .instruction();
        let expected = sdk::ThawDelegatedAccount {
            delegate: key(1).to_bytes().into(),
            token_account: key(2).to_bytes().into(),
            edition: key(3).to_bytes().into(),
            mint: key(4).to_bytes().into(),
            token_program: key(5).to_bytes().into(),
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn mint_new_edition_from_master_edition_via_token_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..14 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut new_metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut new_edition = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut master_edition = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut new_mint = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut edition_mark_pda = cursor.load::<UncheckedAccount>(true).unwrap();
        let new_mint_authority = cursor.load::<UncheckedAccount>(false).unwrap();
        let payer = cursor.load::<UncheckedAccount>(true).unwrap();
        let token_account_owner = cursor.load::<UncheckedAccount>(false).unwrap();
        let token_account = cursor.load::<UncheckedAccount>(false).unwrap();
        let new_metadata_update_authority = cursor.load::<UncheckedAccount>(false).unwrap();
        let metadata = cursor.load::<UncheckedAccount>(false).unwrap();
        let token_program = cursor.load::<UncheckedAccount>(false).unwrap();
        let system_program = cursor.load::<UncheckedAccount>(false).unwrap();
        let rent = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::mint_new_edition_from_master_edition_via_token(
            &program,
            &mut new_metadata,
            &mut new_edition,
            &mut master_edition,
            &mut new_mint,
            &mut edition_mark_pda,
            &new_mint_authority,
            &payer,
            &token_account_owner,
            &token_account,
            &new_metadata_update_authority,
            &metadata,
            &token_program,
            &system_program,
            &rent,
            u64::MAX,
        )
        .instruction();
        let expected = sdk::MintNewEditionFromMasterEditionViaToken {
            new_metadata: key(1).to_bytes().into(),
            new_edition: key(2).to_bytes().into(),
            master_edition: key(3).to_bytes().into(),
            new_mint: key(4).to_bytes().into(),
            edition_mark_pda: key(5).to_bytes().into(),
            new_mint_authority: key(6).to_bytes().into(),
            payer: key(7).to_bytes().into(),
            token_account_owner: key(8).to_bytes().into(),
            token_account: key(9).to_bytes().into(),
            new_metadata_update_authority: key(10).to_bytes().into(),
            metadata: key(11).to_bytes().into(),
            token_program: key(12).to_bytes().into(),
            system_program: key(13).to_bytes().into(),
            rent: Some(key(14).to_bytes().into()),
        }
        .instruction(sdk::MintNewEditionFromMasterEditionViaTokenInstructionArgs {
            mint_new_edition_from_master_edition_via_token_args:
                mpl_token_metadata::types::MintNewEditionFromMasterEditionViaTokenArgs {
                    edition: u64::MAX,
                },
        });
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn remove_creator_verification_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..2 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let creator = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::remove_creator_verification(&program, &mut metadata, &creator)
            .instruction();
        let expected = sdk::RemoveCreatorVerification {
            metadata: key(1).to_bytes().into(),
            creator: key(2).to_bytes().into(),
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn revoke_collection_authority_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..5 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut collection_authority_record = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut delegate_authority = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut revoke_authority = cursor.load::<UncheckedAccount>(true).unwrap();
        let metadata = cursor.load::<UncheckedAccount>(false).unwrap();
        let mint = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::revoke_collection_authority(
            &program,
            &mut collection_authority_record,
            &mut delegate_authority,
            &mut revoke_authority,
            &metadata,
            &mint,
        )
        .instruction();
        let expected = sdk::RevokeCollectionAuthority {
            collection_authority_record: key(1).to_bytes().into(),
            delegate_authority: key(2).to_bytes().into(),
            revoke_authority: key(3).to_bytes().into(),
            metadata: key(4).to_bytes().into(),
            mint: key(5).to_bytes().into(),
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn set_and_verify_collection_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..7 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut collection_authority = cursor.load::<UncheckedAccount>(true).unwrap();
        let payer = cursor.load::<UncheckedAccount>(true).unwrap();
        let update_authority = cursor.load::<UncheckedAccount>(false).unwrap();
        let collection_mint = cursor.load::<UncheckedAccount>(false).unwrap();
        let collection = cursor.load::<UncheckedAccount>(false).unwrap();
        let collection_master_edition_account = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::set_and_verify_collection(
            &program,
            &mut metadata,
            &mut collection_authority,
            &payer,
            &update_authority,
            &collection_mint,
            &collection,
            &collection_master_edition_account,
        )
        .instruction();
        let expected = sdk::SetAndVerifyCollection {
            metadata: key(1).to_bytes().into(),
            collection_authority: key(2).to_bytes().into(),
            payer: key(3).to_bytes().into(),
            update_authority: key(4).to_bytes().into(),
            collection_mint: key(5).to_bytes().into(),
            collection: key(6).to_bytes().into(),
            collection_master_edition_account: key(7).to_bytes().into(),
            collection_authority_record: None,
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn set_and_verify_sized_collection_item_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..7 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let collection_authority = cursor.load::<UncheckedAccount>(false).unwrap();
        let payer = cursor.load::<UncheckedAccount>(true).unwrap();
        let update_authority = cursor.load::<UncheckedAccount>(false).unwrap();
        let collection_mint = cursor.load::<UncheckedAccount>(false).unwrap();
        let mut collection = cursor.load::<UncheckedAccount>(true).unwrap();
        let collection_master_edition_account = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::set_and_verify_sized_collection_item(
            &program,
            &mut metadata,
            &collection_authority,
            &payer,
            &update_authority,
            &collection_mint,
            &mut collection,
            &collection_master_edition_account,
        )
        .instruction();
        let expected = sdk::SetAndVerifySizedCollectionItem {
            metadata: key(1).to_bytes().into(),
            collection_authority: key(2).to_bytes().into(),
            payer: key(3).to_bytes().into(),
            update_authority: key(4).to_bytes().into(),
            collection_mint: key(5).to_bytes().into(),
            collection: key(6).to_bytes().into(),
            collection_master_edition_account: key(7).to_bytes().into(),
            collection_authority_record: None,
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn set_collection_size_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..3 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut collection_metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut collection_authority = cursor.load::<UncheckedAccount>(true).unwrap();
        let collection_mint = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::set_collection_size(
            &program,
            &mut collection_metadata,
            &mut collection_authority,
            &collection_mint,
            u64::MAX,
        )
        .instruction();
        let expected = sdk::SetCollectionSize {
            collection_metadata: key(1).to_bytes().into(),
            collection_authority: key(2).to_bytes().into(),
            collection_mint: key(3).to_bytes().into(),
            collection_authority_record: None,
        }
        .instruction(sdk::SetCollectionSizeInstructionArgs {
            set_collection_size_args: mpl_token_metadata::types::SetCollectionSizeArgs {
                size: u64::MAX,
            },
        });
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn bubblegum_set_collection_size_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..4 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut collection_metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let collection_authority = cursor.load::<UncheckedAccount>(false).unwrap();
        let collection_mint = cursor.load::<UncheckedAccount>(false).unwrap();
        let bubblegum_signer = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::bubblegum_set_collection_size(
            &program,
            &mut collection_metadata,
            &collection_authority,
            &collection_mint,
            &bubblegum_signer,
            u64::MAX,
        )
        .instruction();
        let expected = sdk::BubblegumSetCollectionSize {
            collection_metadata: key(1).to_bytes().into(),
            collection_authority: key(2).to_bytes().into(),
            collection_mint: key(3).to_bytes().into(),
            bubblegum_signer: key(4).to_bytes().into(),
            collection_authority_record: None,
        }
        .instruction(sdk::BubblegumSetCollectionSizeInstructionArgs {
            set_collection_size_args: mpl_token_metadata::types::SetCollectionSizeArgs {
                size: u64::MAX,
            },
        });
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn set_token_standard_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..3 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let update_authority = cursor.load::<UncheckedAccount>(false).unwrap();
        let mint = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual =
            instructions::set_token_standard(&program, &mut metadata, &update_authority, &mint)
                .instruction();
        let expected = sdk::SetTokenStandard {
            metadata: key(1).to_bytes().into(),
            update_authority: key(2).to_bytes().into(),
            mint: key(3).to_bytes().into(),
            edition: None,
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn sign_metadata_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..2 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let creator = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::sign_metadata(&program, &mut metadata, &creator).instruction();
        let expected = sdk::SignMetadata {
            metadata: key(1).to_bytes().into(),
            creator: key(2).to_bytes().into(),
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn unverify_collection_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..5 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut collection_authority = cursor.load::<UncheckedAccount>(true).unwrap();
        let collection_mint = cursor.load::<UncheckedAccount>(false).unwrap();
        let collection = cursor.load::<UncheckedAccount>(false).unwrap();
        let collection_master_edition_account = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::unverify_collection(
            &program,
            &mut metadata,
            &mut collection_authority,
            &collection_mint,
            &collection,
            &collection_master_edition_account,
        )
        .instruction();
        let expected = sdk::UnverifyCollection {
            metadata: key(1).to_bytes().into(),
            collection_authority: key(2).to_bytes().into(),
            collection_mint: key(3).to_bytes().into(),
            collection: key(4).to_bytes().into(),
            collection_master_edition_account: key(5).to_bytes().into(),
            collection_authority_record: None,
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn unverify_sized_collection_item_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..6 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let collection_authority = cursor.load::<UncheckedAccount>(false).unwrap();
        let payer = cursor.load::<UncheckedAccount>(true).unwrap();
        let collection_mint = cursor.load::<UncheckedAccount>(false).unwrap();
        let mut collection = cursor.load::<UncheckedAccount>(true).unwrap();
        let collection_master_edition_account = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::unverify_sized_collection_item(
            &program,
            &mut metadata,
            &collection_authority,
            &payer,
            &collection_mint,
            &mut collection,
            &collection_master_edition_account,
        )
        .instruction();
        let expected = sdk::UnverifySizedCollectionItem {
            metadata: key(1).to_bytes().into(),
            collection_authority: key(2).to_bytes().into(),
            payer: key(3).to_bytes().into(),
            collection_mint: key(4).to_bytes().into(),
            collection: key(5).to_bytes().into(),
            collection_master_edition_account: key(6).to_bytes().into(),
            collection_authority_record: None,
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn update_metadata_accounts_v2_matches_metaplex() {
    for present in [false, true] {
        let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
        for i in 0..2 {
            specs.push((key(i + 1), key(0), &[]));
        }
        with_accounts(&specs, |mut cursor| {
            let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
            let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
            let update_authority = cursor.load::<UncheckedAccount>(false).unwrap();
            let actual = instructions::update_metadata_accounts_v2(
                &program,
                &mut metadata,
                &update_authority,
                present.then_some(Data {
                    name: "é",
                    symbol: "NFT",
                    uri: "https://example.com/metadata.json",
                    seller_fee_basis_points: 500,
                }),
                present.then_some(&key(50)),
                Some(present),
                present.then_some(false),
            )
            .unwrap()
            .instruction();
            let expected = sdk::UpdateMetadataAccountV2 {
                metadata: key(1).to_bytes().into(),
                update_authority: key(2).to_bytes().into(),
            }
            .instruction(sdk::UpdateMetadataAccountV2InstructionArgs {
                data: present.then(data),
                new_update_authority: present.then_some(key(50).to_bytes().into()),
                primary_sale_happened: Some(present),
                is_mutable: present.then_some(false),
            });
            assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
            assert_eq!(actual.data, expected.data);
            assert_eq!(actual.accounts.len(), expected.accounts.len());
            for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
                assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
                assert_eq!(actual.is_signer, expected.is_signer);
                assert_eq!(actual.is_writable, expected.is_writable);
            }
        });
    }
}

#[test]
fn update_primary_sale_happened_via_token_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..3 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let owner = cursor.load::<UncheckedAccount>(false).unwrap();
        let token = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::update_primary_sale_happened_via_token(
            &program,
            &mut metadata,
            &owner,
            &token,
        )
        .instruction();
        let expected = sdk::UpdatePrimarySaleHappenedViaToken {
            metadata: key(1).to_bytes().into(),
            owner: key(2).to_bytes().into(),
            token: key(3).to_bytes().into(),
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn utilize_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..9 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut token_account = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut mint = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut use_authority = cursor.load::<UncheckedAccount>(true).unwrap();
        let owner = cursor.load::<UncheckedAccount>(false).unwrap();
        let token_program = cursor.load::<UncheckedAccount>(false).unwrap();
        let ata_program = cursor.load::<UncheckedAccount>(false).unwrap();
        let system_program = cursor.load::<UncheckedAccount>(false).unwrap();
        let rent = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::utilize(
            &program,
            &mut metadata,
            &mut token_account,
            &mut mint,
            &mut use_authority,
            &owner,
            &token_program,
            &ata_program,
            &system_program,
            &rent,
            u64::MAX,
        )
        .instruction();
        let expected = sdk::Utilize {
            metadata: key(1).to_bytes().into(),
            token_account: key(2).to_bytes().into(),
            mint: key(3).to_bytes().into(),
            use_authority: key(4).to_bytes().into(),
            owner: key(5).to_bytes().into(),
            token_program: key(6).to_bytes().into(),
            ata_program: key(7).to_bytes().into(),
            system_program: key(8).to_bytes().into(),
            rent: key(9).to_bytes().into(),
            use_authority_record: None,
            burner: None,
        }
        .instruction(sdk::UtilizeInstructionArgs { number_of_uses: u64::MAX });
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn verify_collection_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..6 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let mut collection_authority = cursor.load::<UncheckedAccount>(true).unwrap();
        let payer = cursor.load::<UncheckedAccount>(true).unwrap();
        let collection_mint = cursor.load::<UncheckedAccount>(false).unwrap();
        let collection = cursor.load::<UncheckedAccount>(false).unwrap();
        let collection_master_edition_account = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::verify_collection(
            &program,
            &mut metadata,
            &mut collection_authority,
            &payer,
            &collection_mint,
            &collection,
            &collection_master_edition_account,
        )
        .instruction();
        let expected = sdk::VerifyCollection {
            metadata: key(1).to_bytes().into(),
            collection_authority: key(2).to_bytes().into(),
            payer: key(3).to_bytes().into(),
            collection_mint: key(4).to_bytes().into(),
            collection: key(5).to_bytes().into(),
            collection_master_edition_account: key(6).to_bytes().into(),
            collection_authority_record: None,
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

#[test]
fn verify_sized_collection_item_matches_metaplex() {
    let mut specs = vec![(METADATA_PROGRAM_ID, key(0), &[][..])];
    for i in 0..6 {
        specs.push((key(i + 1), key(0), &[]));
    }
    with_accounts(&specs, |mut cursor| {
        let program = cursor.load::<Program<MetadataProgram>>(false).unwrap();
        let mut metadata = cursor.load::<UncheckedAccount>(true).unwrap();
        let collection_authority = cursor.load::<UncheckedAccount>(false).unwrap();
        let payer = cursor.load::<UncheckedAccount>(true).unwrap();
        let collection_mint = cursor.load::<UncheckedAccount>(false).unwrap();
        let mut collection = cursor.load::<UncheckedAccount>(true).unwrap();
        let collection_master_edition_account = cursor.load::<UncheckedAccount>(false).unwrap();
        let actual = instructions::verify_sized_collection_item(
            &program,
            &mut metadata,
            &collection_authority,
            &payer,
            &collection_mint,
            &mut collection,
            &collection_master_edition_account,
        )
        .instruction();
        let expected = sdk::VerifySizedCollectionItem {
            metadata: key(1).to_bytes().into(),
            collection_authority: key(2).to_bytes().into(),
            payer: key(3).to_bytes().into(),
            collection_mint: key(4).to_bytes().into(),
            collection: key(5).to_bytes().into(),
            collection_master_edition_account: key(6).to_bytes().into(),
            collection_authority_record: None,
        }
        .instruction();
        assert_eq!(actual.program_id.to_bytes(), expected.program_id.to_bytes());
        assert_eq!(actual.data, expected.data);
        assert_eq!(actual.accounts.len(), expected.accounts.len());
        for (actual, expected) in actual.accounts.iter().zip(expected.accounts) {
            assert_eq!(actual.pubkey.to_bytes(), expected.pubkey.to_bytes());
            assert_eq!(actual.is_signer, expected.is_signer);
            assert_eq!(actual.is_writable, expected.is_writable);
        }
    });
}

fn data() -> mpl_token_metadata::types::DataV2 {
    mpl_token_metadata::types::DataV2 {
        name: "é".into(),
        symbol: "NFT".into(),
        uri: "https://example.com/metadata.json".into(),
        seller_fee_basis_points: 500,
        creators: None,
        collection: None,
        uses: None,
    }
}
