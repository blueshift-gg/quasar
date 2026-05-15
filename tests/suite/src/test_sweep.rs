use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_token_cpi::cpi::*,
};

// sweep only with SPL Token.

#[test]
fn sweep_spl_happy() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = SweepTokenInstruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 500, token_program),
            token_account(receiver_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "sweep SPL should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn sweep_spl_zero_balance() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = SweepTokenInstruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 0, token_program),
            token_account(receiver_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "sweep SPL zero balance should be no-op: {:?}",
        result.raw_result
    );
}

#[test]
fn sweep_spl_wrong_authority() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = SweepTokenInstruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, wrong_authority, 500, token_program),
            token_account(receiver_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
        ],
    );
    assert!(result.is_err(), "sweep SPL wrong authority should fail");
}

// sweep only with Token-2022.

#[test]
fn sweep_t22_happy() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = SweepTokenT22Instruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 500, token_program),
            token_account(receiver_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "sweep T22 should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn sweep_t22_zero_balance() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = SweepTokenT22Instruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 0, token_program),
            token_account(receiver_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "sweep T22 zero balance should be no-op: {:?}",
        result.raw_result
    );
}

// sweep only with InterfaceAccount, SPL and Token-2022.

#[test]
fn sweep_interface_spl_happy() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = SweepTokenInterfaceInstruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 500, token_program),
            token_account(receiver_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "sweep Interface SPL should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn sweep_interface_t22_happy() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = SweepTokenInterfaceInstruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 500, token_program),
            token_account(receiver_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "sweep Interface T22 should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn sweep_interface_wrong_authority() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = SweepTokenInterfaceInstruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, wrong_authority, 500, token_program),
            token_account(receiver_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
        ],
    );
    assert!(
        result.is_err(),
        "sweep Interface wrong authority should fail"
    );
}

// sweep + close with SPL Token.

#[test]
fn sweep_and_close_spl_happy() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = SweepAndCloseInstruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 1000, token_program),
            token_account(receiver_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_ok(),
        "sweep + close SPL should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn sweep_and_close_spl_zero_balance() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = SweepAndCloseInstruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 0, token_program),
            token_account(receiver_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_ok(),
        "sweep + close SPL zero balance should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn sweep_and_close_spl_wrong_mint_receiver() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = SweepAndCloseInstruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 500, token_program),
            token_account(receiver_key, wrong_mint, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_err(),
        "sweep + close SPL wrong mint receiver should fail"
    );
}

// sweep + close with Token-2022.

#[test]
fn sweep_and_close_t22_happy() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = SweepAndCloseT22Instruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 1000, token_program),
            token_account(receiver_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_ok(),
        "sweep + close T22 should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn sweep_and_close_t22_zero_balance() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = SweepAndCloseT22Instruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 0, token_program),
            token_account(receiver_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_ok(),
        "sweep + close T22 zero balance should succeed: {:?}",
        result.raw_result
    );
}

// sweep + close with InterfaceAccount, SPL and Token-2022.

#[test]
fn sweep_and_close_interface_spl_happy() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = SweepAndCloseInterfaceInstruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 1000, token_program),
            token_account(receiver_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_ok(),
        "sweep + close Interface SPL should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn sweep_and_close_interface_t22_happy() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = SweepAndCloseInterfaceInstruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 1000, token_program),
            token_account(receiver_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_ok(),
        "sweep + close Interface T22 should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn sweep_and_close_interface_wrong_authority() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = SweepAndCloseInterfaceInstruction {
        authority,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, wrong_authority, 500, token_program),
            token_account(receiver_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_err(),
        "sweep + close Interface wrong authority should fail"
    );
}
