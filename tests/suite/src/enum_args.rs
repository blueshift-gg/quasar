//! Integration coverage for `#[derive(QuasarSerialize)]` on
//! `#[repr(u8)]` unit enums. The `enum_arg_check` instruction
//! (discriminator 58) takes two enum arguments and requires them to be
//! exactly `Side::Ask` and `Priority::High`.
//!
//! Tests are split into two categories:
//! * Happy-path round-trips through the generated CPI struct, covering both
//!   explicit (`Side`) and implicit (`Priority`) discriminants.
//! * Adversarial raw-byte instructions that bypass the CPI struct, to prove
//!   `validate_zc` rejects bytes outside the declared tag set and truncated
//!   payloads.

use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_misc::{
        cpi::*,
        state::{Priority, Side},
    },
};

const ENUM_CHECK_DISC: u8 = 58;

// ---------------------------------------------------------------------------
// Happy-path via generated CPI structs
// ---------------------------------------------------------------------------

#[test]
fn enum_arg_accepts_expected_variants() {
    let mut svm = svm_misc();
    let signer = Pubkey::new_unique();
    let ix: Instruction = EnumArgCheckInstruction {
        signer,
        side: Side::Ask,
        priority: Priority::High,
    }
    .into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(
        result.is_ok(),
        "Ask + High should satisfy require!: {:?}",
        result.raw_result
    );
}

#[test]
fn enum_arg_rejects_wrong_side() {
    // Bid is a declared variant (tag = 7) so `validate_zc` passes, but
    // the handler's `require!` still rejects it — this proves that a
    // valid tag byte does not imply a valid business-logic argument.
    let mut svm = svm_misc();
    let signer = Pubkey::new_unique();
    let ix: Instruction = EnumArgCheckInstruction {
        signer,
        side: Side::Bid,
        priority: Priority::High,
    }
    .into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(result.is_err(), "Bid should be rejected by require!");
}

#[test]
fn enum_arg_rejects_wrong_priority() {
    let mut svm = svm_misc();
    let signer = Pubkey::new_unique();
    let ix: Instruction = EnumArgCheckInstruction {
        signer,
        side: Side::Ask,
        priority: Priority::Normal,
    }
    .into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(result.is_err(), "Normal priority should be rejected");
}

// ---------------------------------------------------------------------------
// Adversarial raw-byte instructions
//
// Wire format: [disc(58), side_tag: u8, priority_tag: u8]. Side accepts
// only {7, 42}; Priority accepts only {0, 1, 2}. Any other combination
// must fail before the handler runs (rejected by `validate_zc`).
// ---------------------------------------------------------------------------

fn raw_enum_ix(side_tag: u8, priority_tag: u8) -> solana_instruction::Instruction {
    solana_instruction::Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(
            Pubkey::new_unique(),
            true,
        )],
        data: vec![ENUM_CHECK_DISC, side_tag, priority_tag],
    }
}

#[test]
fn enum_arg_rejects_undeclared_side_tag() {
    // Tag 0 is not a declared Side variant — validate_zc must reject.
    let mut svm = svm_misc();
    let ix = raw_enum_ix(0, 2);
    let signer = ix.accounts[0].pubkey;
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(
        result.is_err(),
        "side_tag=0 is not a declared variant (only 7 and 42 are)"
    );
}

#[test]
fn enum_arg_rejects_undeclared_side_tag_boundary() {
    // 6 and 8 flank the `Bid = 7` discriminant; 41 and 43 flank
    // `Ask = 42`. All four must be rejected, proving `validate_zc` is
    // checking equality rather than a range.
    let mut svm = svm_misc();
    for side_tag in [6u8, 8, 41, 43, 0xFF] {
        let ix = raw_enum_ix(side_tag, 2);
        let signer = ix.accounts[0].pubkey;
        let result = svm.process_instruction(&ix, &[signer_account(signer)]);
        assert!(
            result.is_err(),
            "side_tag={side_tag} must be rejected by validate_zc"
        );
    }
}

#[test]
fn enum_arg_rejects_undeclared_priority_tag() {
    // Priority is {Low=0, Normal=1, High=2}; any higher byte must fail.
    let mut svm = svm_misc();
    for priority_tag in [3u8, 4, 0x7F, 0xFF] {
        let ix = raw_enum_ix(42, priority_tag);
        let signer = ix.accounts[0].pubkey;
        let result = svm.process_instruction(&ix, &[signer_account(signer)]);
        assert!(
            result.is_err(),
            "priority_tag={priority_tag} must be rejected by validate_zc"
        );
    }
}

#[test]
fn enum_arg_rejects_truncated_data() {
    // Only the discriminator + side tag — priority byte missing.
    let mut svm = svm_misc();
    let signer = Pubkey::new_unique();
    let ix = solana_instruction::Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(signer, true)],
        data: vec![ENUM_CHECK_DISC, 42u8], // Ask, then truncated
    };
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(
        result.is_err(),
        "truncated instruction data (missing priority tag) must fail"
    );
}
