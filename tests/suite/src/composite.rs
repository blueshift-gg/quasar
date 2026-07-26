//! Composite (`#[account(group)]`) accounts end to end.
//!
//! A composite field contributes `Inner::COUNT` accounts to the flattened
//! list, not one. These tests pin that the generated client agrees with the
//! on-chain parser about that count and its ordering.

use {
    crate::compat::{AccountMeta, Instruction, Pubkey},
    crate::helpers::*,
    quasar_lang::traits::AccountCount,
    quasar_test_misc::{cpi::*, instructions::CompositeGroup},
};

fn composite_ix(payer: Pubkey, first: Pubkey, second: Pubkey) -> Instruction {
    CompositeGroupInstruction {
        payer,
        pair: vec![
            AccountMeta::new_readonly(first, true),
            AccountMeta::new_readonly(second, true),
        ],
    }
    .into()
}

#[test]
fn composite_flattens_to_inner_count() {
    assert_eq!(
        <CompositeGroup as AccountCount>::COUNT,
        3,
        "payer + SignerPair::COUNT"
    );
}

#[test]
fn composite_client_builds_every_account() {
    let (payer, first, second) = (
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    );
    let ix = composite_ix(payer, first, second);

    assert_eq!(
        ix.accounts.len(),
        <CompositeGroup as AccountCount>::COUNT,
        "generated client must emit one meta per flattened account"
    );
    assert_eq!(
        ix.accounts.iter().map(|m| m.pubkey).collect::<Vec<_>>(),
        vec![payer, first, second]
    );
}

#[test]
fn composite_succeeds() {
    let mut svm = svm_misc();
    let (payer, first, second) = (
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    );

    let result = svm.process_instruction(
        &composite_ix(payer, first, second),
        &[
            signer_account(payer),
            signer_account(first),
            signer_account(second),
        ],
    );
    assert!(result.is_ok(), "composite: {:?}", result.raw_result);
}
