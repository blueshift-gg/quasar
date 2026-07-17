use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_token_cpi::cpi::*,
    solana_program_pack::Pack,
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

    let source = result.account(&source_key).expect("source result");
    assert_eq!(
        quasar_svm::token::TokenAccount::unpack(&source.data)
            .expect("decode source")
            .amount,
        0,
        "source drained"
    );
    let receiver = result.account(&receiver_key).expect("receiver result");
    assert_eq!(
        quasar_svm::token::TokenAccount::unpack(&receiver.data)
            .expect("decode receiver")
            .amount,
        500,
        "receiver credited"
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

    let receiver = result.account(&receiver_key).expect("receiver result");
    assert_eq!(
        quasar_svm::token::TokenAccount::unpack(&receiver.data)
            .expect("decode receiver")
            .amount,
        0,
        "zero-balance sweep must be a no-op"
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
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
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

    let source = result.account(&source_key).expect("source result");
    assert_eq!(
        quasar_svm::token::TokenAccount::unpack(&source.data)
            .expect("decode source")
            .amount,
        0,
        "source drained"
    );
    let receiver = result.account(&receiver_key).expect("receiver result");
    assert_eq!(
        quasar_svm::token::TokenAccount::unpack(&receiver.data)
            .expect("decode receiver")
            .amount,
        500,
        "receiver credited"
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

    let source = result.account(&source_key).expect("source result");
    assert_eq!(
        quasar_svm::token::TokenAccount::unpack(&source.data)
            .expect("decode source")
            .amount,
        0,
        "source drained"
    );
    let receiver = result.account(&receiver_key).expect("receiver result");
    assert_eq!(
        quasar_svm::token::TokenAccount::unpack(&receiver.data)
            .expect("decode receiver")
            .amount,
        500,
        "receiver credited"
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
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
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

    let receiver = result.account(&receiver_key).expect("receiver result");
    assert_eq!(
        quasar_svm::token::TokenAccount::unpack(&receiver.data)
            .expect("decode receiver")
            .amount,
        1000,
        "receiver credited"
    );
    assert_eq!(
        result.account(&source_key).expect("closed source").lamports,
        0,
        "source closed"
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

    let receiver = result.account(&receiver_key).expect("receiver result");
    assert_eq!(
        quasar_svm::token::TokenAccount::unpack(&receiver.data)
            .expect("decode receiver")
            .amount,
        0,
        "zero-balance sweep must be a no-op"
    );
    assert_eq!(
        result.account(&source_key).expect("closed source").lamports,
        0,
        "source still closed"
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
    // spl_token::TokenError::MintMismatch
    result.assert_error(quasar_svm::ProgramError::Custom(3));
}

// sweep + close with Token-2022.

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

    let receiver = result.account(&receiver_key).expect("receiver result");
    assert_eq!(
        quasar_svm::token::TokenAccount::unpack(&receiver.data)
            .expect("decode receiver")
            .amount,
        1000,
        "receiver credited"
    );
    assert_eq!(
        result.account(&source_key).expect("closed source").lamports,
        0,
        "source closed"
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
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
}

#[test]
fn pda_sweep_and_close_runs_token_exits_before_authority_close() {
    let mut svm = svm_cpi();
    let owner = Pubkey::new_unique();
    let (authority, bump) =
        Pubkey::find_program_address(&[b"lifecycle", owner.as_ref()], &quasar_test_token_cpi::ID);
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let receiver_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = PdaSweepAndCloseInstruction {
        owner,
        source: source_key,
        receiver: receiver_key,
        mint: mint_key,
    }
    .into();

    let mut authority_data = vec![0; 34];
    authority_data[0] = 2;
    authority_data[1..33].copy_from_slice(owner.as_ref());
    authority_data[33] = bump;
    let authority_lamports = 1_000_000;
    let source = token_account(source_key, mint_key, authority, 500, token_program);
    let source_lamports = source.lamports;
    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(owner),
            raw_account(
                authority,
                authority_lamports,
                authority_data,
                quasar_test_token_cpi::ID,
            ),
            source,
            token_account(receiver_key, mint_key, owner, 0, token_program),
            mint_account(mint_key, owner, 6, token_program),
        ],
    );
    result.assert_success();

    let receiver = result.account(&receiver_key).expect("receiver result");
    assert_eq!(
        quasar_svm::token::TokenAccount::unpack(&receiver.data)
            .expect("decode receiver")
            .amount,
        500
    );
    let owner_result = result.account(&owner).expect("owner result");
    assert_eq!(
        owner_result.lamports,
        1_000_000 + authority_lamports + source_lamports
    );
    assert_eq!(
        result
            .account(&authority)
            .expect("closed authority")
            .lamports,
        0
    );
    assert_eq!(
        result.account(&source_key).expect("closed source").lamports,
        0
    );
}
