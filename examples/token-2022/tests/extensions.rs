//! Native SBF coverage for every mint extension and the account-facing port boundaries.
use quasar_test::Test;
use quasar_token_2022::ID;
use solana_address::Address;
use solana_instruction::Instruction;
use spl_token_2022_interface::{
    error::TokenError,
    extension::{self as ext, BaseStateWithExtensions, ExtensionType as E, StateWithExtensions},
    instruction as ix,
    state::{Account, AccountState, Mint},
};

const MINT: Address = Address::new_from_array([20; 32]);
const AUTH: Address = Address::new_from_array([21; 32]);
const ALICE: Address = Address::new_from_array([22; 32]);
const BOB: Address = Address::new_from_array([23; 32]);
const SYSTEM: Address = Address::new_from_array([0; 32]);

fn prepare(extensions: &[E]) -> Test {
    let mut t = Test::new(ID, "quasar_token_2022");
    t.set(AUTH, SYSTEM, 100_000_000_000, vec![]);
    let size = E::try_calculate_account_len::<Mint>(extensions).unwrap();
    t.set(MINT, ID, t.rent(size), vec![0; size]);
    t
}
fn initialize(t: &mut Test) {
    t.run(ix::initialize_mint2(&ID, &MINT, &AUTH, Some(&AUTH), 6).unwrap()).succeeds();
}
fn token(t: &mut Test, key: Address, extensions: &[E]) {
    let len = E::try_calculate_account_len::<Account>(extensions).unwrap();
    t.set(key, ID, t.rent(len), vec![0; len]);
    t.run(ix::initialize_account3(&ID, &key, &MINT, &AUTH).unwrap()).succeeds();
}
fn mint(t: &mut Test, key: Address, amount: u64) {
    t.run(ix::mint_to_checked(&ID, &MINT, &key, &AUTH, &[], amount, 6).unwrap()).succeeds();
}
fn balance(t: &Test, key: Address) -> u64 {
    StateWithExtensions::<Account>::unpack(t.data(&key)).unwrap().base.amount
}

#[test]
fn every_mint_extension_initializes_in_the_native_program() {
    use ext::{
        confidential_mint_burn as cmb, confidential_transfer as ct,
        confidential_transfer_fee as cf, default_account_state as ds, group_member_pointer as gmp,
        group_pointer as gp, interest_bearing_mint as ib, metadata_pointer as mp, pausable as p,
        permissioned_burn as pb, scaled_ui_amount as scaled, transfer_fee as fee,
        transfer_hook as hook,
    };
    let cases: Vec<(Vec<E>, Vec<Instruction>)> = vec![
        (
            vec![E::TransferFeeConfig],
            vec![fee::instruction::initialize_transfer_fee_config(
                &ID,
                &MINT,
                Some(&AUTH),
                Some(&AUTH),
                250,
                100,
            )
            .unwrap()],
        ),
        (
            vec![E::MintCloseAuthority],
            vec![ix::initialize_mint_close_authority(&ID, &MINT, Some(&AUTH)).unwrap()],
        ),
        (
            vec![E::ConfidentialTransferMint],
            vec![ct::instruction::initialize_mint(&ID, &MINT, Some(AUTH), true, None).unwrap()],
        ),
        (
            vec![E::DefaultAccountState],
            vec![ds::instruction::initialize_default_account_state(
                &ID,
                &MINT,
                &AccountState::Frozen,
            )
            .unwrap()],
        ),
        (vec![E::NonTransferable], vec![ix::initialize_non_transferable_mint(&ID, &MINT).unwrap()]),
        (
            vec![E::InterestBearingConfig],
            vec![ib::instruction::initialize(&ID, &MINT, Some(AUTH), 100).unwrap()],
        ),
        (
            vec![E::PermanentDelegate],
            vec![ix::initialize_permanent_delegate(&ID, &MINT, &AUTH).unwrap()],
        ),
        (
            vec![E::TransferHook],
            vec![hook::instruction::initialize(&ID, &MINT, Some(AUTH), Some(BOB)).unwrap()],
        ),
        (
            vec![E::MetadataPointer],
            vec![mp::instruction::initialize(&ID, &MINT, Some(AUTH), Some(MINT)).unwrap()],
        ),
        (
            vec![E::GroupPointer],
            vec![gp::instruction::initialize(&ID, &MINT, Some(AUTH), Some(MINT)).unwrap()],
        ),
        (
            vec![E::GroupMemberPointer],
            vec![gmp::instruction::initialize(&ID, &MINT, Some(AUTH), Some(MINT)).unwrap()],
        ),
        (
            vec![E::ScaledUiAmount],
            vec![scaled::instruction::initialize(&ID, &MINT, Some(AUTH), 2.0).unwrap()],
        ),
        (vec![E::Pausable], vec![p::instruction::initialize(&ID, &MINT, &AUTH).unwrap()]),
        (vec![E::PermissionedBurn], vec![pb::instruction::initialize(&ID, &MINT, &AUTH).unwrap()]),
        (
            vec![E::ConfidentialTransferMint, E::ConfidentialMintBurn],
            vec![
                ct::instruction::initialize_mint(&ID, &MINT, Some(AUTH), true, None).unwrap(),
                cmb::instruction::initialize_mint(
                    &ID,
                    &MINT,
                    &Default::default(),
                    &Default::default(),
                )
                .unwrap(),
            ],
        ),
        (
            vec![
                E::TransferFeeConfig,
                E::ConfidentialTransferMint,
                E::ConfidentialTransferFeeConfig,
            ],
            vec![
                fee::instruction::initialize_transfer_fee_config(
                    &ID,
                    &MINT,
                    Some(&AUTH),
                    Some(&AUTH),
                    250,
                    100,
                )
                .unwrap(),
                ct::instruction::initialize_mint(&ID, &MINT, Some(AUTH), true, None).unwrap(),
                cf::instruction::initialize_confidential_transfer_fee_config(
                    &ID,
                    &MINT,
                    Some(AUTH),
                    &Default::default(),
                )
                .unwrap(),
            ],
        ),
    ];
    for (extensions, instructions) in cases {
        let mut t = prepare(&extensions);
        for instruction in instructions {
            t.run(instruction).succeeds();
        }
        initialize(&mut t);
        let state = StateWithExtensions::<Mint>::unpack(t.data(&MINT)).unwrap();
        assert!(state.base.is_initialized);
        let actual = state.get_extension_types().unwrap();
        for extension in &extensions {
            assert!(actual.contains(extension), "{extension:?}");
        }
    }
}

#[test]
fn transfer_fees_are_withheld_harvested_and_withdrawn() {
    use ext::transfer_fee::{instruction as fee, TransferFeeAmount};
    let mut t = prepare(&[E::TransferFeeConfig]);
    t.run(
        fee::initialize_transfer_fee_config(&ID, &MINT, Some(&AUTH), Some(&AUTH), 250, 100)
            .unwrap(),
    )
    .succeeds();
    initialize(&mut t);
    token(&mut t, ALICE, &[E::TransferFeeAmount]);
    token(&mut t, BOB, &[E::TransferFeeAmount]);
    mint(&mut t, ALICE, 1000);
    t.run(fee::transfer_checked_with_fee(&ID, &ALICE, &MINT, &BOB, &AUTH, &[], 400, 6, 9).unwrap())
        .fails(TokenError::FeeMismatch);
    t.run(
        fee::transfer_checked_with_fee(&ID, &ALICE, &MINT, &BOB, &AUTH, &[], 400, 6, 10).unwrap(),
    )
    .succeeds();
    assert_eq!((balance(&t, ALICE), balance(&t, BOB)), (600, 390));
    let account = StateWithExtensions::<Account>::unpack(t.data(&BOB)).unwrap();
    assert_eq!(
        u64::from(account.get_extension::<TransferFeeAmount>().unwrap().withheld_amount),
        10
    );
    t.run(fee::harvest_withheld_tokens_to_mint(&ID, &MINT, &[&BOB]).unwrap()).succeeds();
    t.run(fee::withdraw_withheld_tokens_from_mint(&ID, &MINT, &ALICE, &AUTH, &[]).unwrap())
        .succeeds();
    assert_eq!(balance(&t, ALICE), 610);
}

#[test]
fn metadata_resizes_a_mint_passed_twice_and_preserves_other_extensions() {
    use spl_token_metadata_interface::{
        instruction as meta,
        state::{Field, TokenMetadata},
    };
    let mut t = prepare(&[E::MetadataPointer]);
    t.run(
        ext::metadata_pointer::instruction::initialize(&ID, &MINT, Some(AUTH), Some(MINT)).unwrap(),
    )
    .succeeds();
    initialize(&mut t);
    // Metadata instructions intentionally require the mint in two account slots.
    t.set(MINT, ID, t.rent(4096), t.data(&MINT).to_vec());
    t.run(meta::initialize(
        &ID,
        &MINT,
        &AUTH,
        &MINT,
        &AUTH,
        "Quasar".into(),
        "QSR".into(),
        "https://example.com".into(),
    ))
    .succeeds();
    let before = t.data(&MINT).len();
    t.run(meta::update_field(&ID, &MINT, &AUTH, Field::Name, "Q".repeat(200))).succeeds();
    assert!(t.data(&MINT).len() > before);
    t.run(meta::update_field(&ID, &MINT, &AUTH, Field::Name, "Q".into())).succeeds();
    assert!(t.data(&MINT).len() < before);
    let state = StateWithExtensions::<Mint>::unpack(t.data(&MINT)).unwrap();
    assert_eq!(state.get_variable_len_extension::<TokenMetadata>().unwrap().name, "Q");
    assert!(state.get_extension::<ext::metadata_pointer::MetadataPointer>().is_ok());
}

#[test]
fn group_and_member_extensions_resize_their_mints() {
    use spl_token_group_interface::{
        instruction as group,
        state::{TokenGroup, TokenGroupMember},
    };
    let mut t = prepare(&[E::GroupPointer]);
    t.run(ext::group_pointer::instruction::initialize(&ID, &MINT, Some(AUTH), Some(MINT)).unwrap())
        .succeeds();
    initialize(&mut t);
    t.set(MINT, ID, t.rent(4096), t.data(&MINT).to_vec());
    t.run(group::initialize_group(&ID, &MINT, &MINT, &AUTH, Some(AUTH), 10)).succeeds();
    let len = E::try_calculate_account_len::<Mint>(&[E::GroupMemberPointer]).unwrap();
    t.set(ALICE, ID, t.rent(4096), vec![0; len]);
    t.run(
        ext::group_member_pointer::instruction::initialize(&ID, &ALICE, Some(AUTH), Some(ALICE))
            .unwrap(),
    )
    .succeeds();
    t.run(ix::initialize_mint2(&ID, &ALICE, &AUTH, None, 0).unwrap()).succeeds();
    t.run(group::initialize_member(&ID, &ALICE, &ALICE, &AUTH, &MINT, &AUTH)).succeeds();
    let group = StateWithExtensions::<Mint>::unpack(t.data(&MINT)).unwrap();
    assert_eq!(u64::from(group.get_extension::<TokenGroup>().unwrap().size), 1);
    let member = StateWithExtensions::<Mint>::unpack(t.data(&ALICE)).unwrap();
    assert_eq!(member.get_extension::<TokenGroupMember>().unwrap().group, MINT);
}

#[test]
fn pause_blocks_transfers_and_resume_restores_them() {
    let mut t = prepare(&[E::Pausable]);
    t.run(ext::pausable::instruction::initialize(&ID, &MINT, &AUTH).unwrap()).succeeds();
    initialize(&mut t);
    token(&mut t, ALICE, &[E::PausableAccount]);
    token(&mut t, BOB, &[E::PausableAccount]);
    mint(&mut t, ALICE, 100);
    t.run(ext::pausable::instruction::pause(&ID, &MINT, &AUTH, &[]).unwrap()).succeeds();
    let transfer = ix::transfer_checked(&ID, &ALICE, &MINT, &BOB, &AUTH, &[], 10, 6).unwrap();
    t.run(transfer.clone()).fails(TokenError::MintPaused);
    t.run(ext::pausable::instruction::resume(&ID, &MINT, &AUTH, &[]).unwrap()).succeeds();
    t.run(transfer).succeeds();
    assert_eq!(balance(&t, BOB), 10);
}

#[test]
fn reallocation_funds_rent_through_native_quasar_cpi() {
    let mut t = prepare(&[]);
    initialize(&mut t);
    token(&mut t, ALICE, &[]);
    let before = t.lamports(&AUTH);
    t.run(ix::reallocate(&ID, &ALICE, &AUTH, &AUTH, &[], &[E::MemoTransfer, E::CpiGuard]).unwrap())
        .succeeds();
    assert!(t.lamports(&AUTH) < before);
    t.run(
        ext::memo_transfer::instruction::enable_required_transfer_memos(&ID, &ALICE, &AUTH, &[])
            .unwrap(),
    )
    .succeeds();
    t.run(ext::cpi_guard::instruction::enable_cpi_guard(&ID, &ALICE, &AUTH, &[]).unwrap())
        .succeeds();
    let state = StateWithExtensions::<Account>::unpack(t.data(&ALICE)).unwrap();
    assert!(bool::from(
        state
            .get_extension::<ext::memo_transfer::MemoTransfer>()
            .unwrap()
            .require_incoming_transfer_memos
    ));
    assert!(bool::from(state.get_extension::<ext::cpi_guard::CpiGuard>().unwrap().lock_cpi));
}

#[test]
fn transfer_hook_resolves_extras_strips_signer_privileges_and_rolls_back_failures() {
    use ext::transfer_hook::{instruction as hook, TransferHookAccount};
    use solana_instruction::AccountMeta;
    use spl_tlv_account_resolution::{account::ExtraAccountMeta, state::ExtraAccountMetaList};
    use spl_transfer_hook_interface::{
        get_extra_account_metas_address, instruction::ExecuteInstruction,
    };
    const HOOK: Address = Address::new_from_array([31; 32]);
    const COUNTER: Address = Address::new_from_array([32; 32]);
    let mut t = prepare(&[E::TransferHook]);
    t.add_program(HOOK, "token_2022_test_hook");
    t.run(hook::initialize(&ID, &MINT, Some(AUTH), Some(HOOK)).unwrap()).succeeds();
    initialize(&mut t);
    token(&mut t, ALICE, &[E::TransferHookAccount]);
    token(&mut t, BOB, &[E::TransferHookAccount]);
    mint(&mut t, ALICE, 100);
    let validation = get_extra_account_metas_address(&MINT, &HOOK);
    let mut data = vec![0; ExtraAccountMetaList::size_of(1).unwrap()];
    ExtraAccountMetaList::init::<ExecuteInstruction>(
        &mut data,
        &[ExtraAccountMeta::new_with_pubkey(&COUNTER, true, true).unwrap()],
    )
    .unwrap();
    t.set(validation, HOOK, t.rent(data.len()), data);
    t.set(COUNTER, HOOK, t.rent(8), vec![0; 8]);
    let transfer = |amount| {
        let mut instruction =
            ix::transfer_checked(&ID, &ALICE, &MINT, &BOB, &AUTH, &[], amount, 6).unwrap();
        instruction.accounts.extend([
            AccountMeta::new_readonly(validation, false),
            AccountMeta::new_readonly(HOOK, false),
            AccountMeta::new(COUNTER, true),
        ]);
        instruction
    };
    t.run(transfer(10)).succeeds();
    assert_eq!(t.data(&COUNTER), &10u64.to_le_bytes());
    for account in [ALICE, BOB] {
        let state = StateWithExtensions::<Account>::unpack(t.data(&account)).unwrap();
        assert!(!bool::from(state.get_extension::<TransferHookAccount>().unwrap().transferring));
    }
    t.run(transfer(13)).fails(solana_program_error::ProgramError::Custom(701));
    assert_eq!((balance(&t, ALICE), balance(&t, BOB)), (90, 10));
    assert_eq!(t.data(&COUNTER), &10u64.to_le_bytes());
}

#[test]
fn confidential_configuration_accepts_verified_contexts_and_rejects_forged_owners() {
    use bytemuck::Zeroable;
    use ext::confidential_transfer::{instruction as ct, ConfidentialTransferAccount};
    use solana_zk_elgamal_proof_interface::{
        proof_data::{ProofType, PubkeyValidityProofContext},
        state::ProofContextState,
    };
    use spl_token_confidential_transfer_proof_extraction::instruction::ProofLocation;
    const PROOF: Address = Address::new_from_array([40; 32]);
    let mut t = prepare(&[E::ConfidentialTransferMint]);
    t.run(ct::initialize_mint(&ID, &MINT, Some(AUTH), true, None).unwrap()).succeeds();
    initialize(&mut t);
    token(&mut t, ALICE, &[E::ConfidentialTransferAccount]);
    let proof = ProofContextState::encode(
        &AUTH,
        ProofType::PubkeyValidity,
        &PubkeyValidityProofContext::zeroed(),
    );
    t.set(PROOF, SYSTEM, t.rent(proof.len()), proof.clone());
    let configure = ct::inner_configure_account(
        &ID,
        &ALICE,
        &MINT,
        &Default::default(),
        100,
        &AUTH,
        &[],
        ProofLocation::ContextStateAccount(&PROOF),
    )
    .unwrap();
    t.run(configure.clone()).fails(solana_program_error::ProgramError::IncorrectProgramId);
    t.set(PROOF, solana_zk_elgamal_proof_interface::ID, t.rent(proof.len()), proof);
    t.run(configure).succeeds();
    let state = StateWithExtensions::<Account>::unpack(t.data(&ALICE)).unwrap();
    assert!(bool::from(state.get_extension::<ConfidentialTransferAccount>().unwrap().approved));
}

#[test]
fn native_mint_creation_uses_quasar_signed_cpi_and_preserves_prefunding() {
    let mut t = prepare(&[]);
    let native = spl_token_2022_interface::native_mint::ID;
    let prefund = t.rent(82) + 100;
    t.set(native, SYSTEM, prefund, vec![]);
    t.run(ix::create_native_mint(&ID, &AUTH).unwrap()).succeeds();
    assert_eq!(t.owner(&native), ID);
    assert_eq!(t.lamports(&native), prefund);
    assert_eq!(StateWithExtensions::<Mint>::unpack(t.data(&native)).unwrap().base.decimals, 9);
}
