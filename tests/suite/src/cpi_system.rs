use {
    crate::helpers::*,
    quasar_svm::{Account, Instruction, ProgramError, Pubkey},
    quasar_test_misc::cpi::*,
};

// create_account.

#[test]
fn create_success() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let new_account = Pubkey::new_unique();
    let owner = Pubkey::new_unique();

    let ix: Instruction = CreateAccountTestInstruction {
        payer,
        new_account,
        system_program: quasar_svm::system_program::ID,
        lamports: 1_000_000,
        space: 100,
        owner,
    }
    .into();

    let result = svm.process_instruction(
        &ix,
        &[rich_signer_account(payer), empty_account(new_account)],
    );
    assert!(result.is_ok(), "create: {:?}", result.raw_result);

    let acc = result.account(&new_account).expect("created account");
    assert_eq!(acc.data.len(), 100, "space");
    assert_eq!(acc.owner, owner, "owner");
    assert!(acc.lamports >= 1_000_000, "lamports");
}

#[test]
fn create_zero_space() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let new_account = Pubkey::new_unique();
    let owner = Pubkey::new_unique();

    let ix: Instruction = CreateAccountTestInstruction {
        payer,
        new_account,
        system_program: quasar_svm::system_program::ID,
        lamports: 890_880, // rent for 0 bytes
        space: 0,
        owner,
    }
    .into();

    let result = svm.process_instruction(
        &ix,
        &[rich_signer_account(payer), empty_account(new_account)],
    );
    assert!(result.is_ok(), "zero space: {:?}", result.raw_result);
}

#[test]
fn create_large_space() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let new_account = Pubkey::new_unique();
    let owner = Pubkey::new_unique();

    let ix: Instruction = CreateAccountTestInstruction {
        payer,
        new_account,
        system_program: quasar_svm::system_program::ID,
        lamports: 100_000_000,
        space: 10_000,
        owner,
    }
    .into();

    let result = svm.process_instruction(
        &ix,
        &[rich_signer_account(payer), empty_account(new_account)],
    );
    assert!(result.is_ok(), "large space: {:?}", result.raw_result);
    let acc = result.account(&new_account).expect("account");
    assert_eq!(acc.data.len(), 10_000);
}

#[test]
fn create_insufficient_funds() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let new_account = Pubkey::new_unique();

    let ix: Instruction = CreateAccountTestInstruction {
        payer,
        new_account,
        system_program: quasar_svm::system_program::ID,
        lamports: 100_000_000,
        space: 100,
        owner: Pubkey::new_unique(),
    }
    .into();

    let result = svm.process_instruction(
        &ix,
        &[
            Account {
                address: payer,
                lamports: 1,
                data: vec![],
                owner: quasar_svm::system_program::ID,
                executable: false,
            },
            empty_account(new_account),
        ],
    );
    // SystemError::ResultWithNegativeLamports = Custom(1)
    result.assert_error(ProgramError::Custom(1));
}

// transfer.

#[test]
fn transfer_success() {
    let mut svm = svm_misc();
    let from = Pubkey::new_unique();
    let to = Pubkey::new_unique();

    let ix: Instruction = TransferTestInstruction {
        from,
        to,
        system_program: quasar_svm::system_program::ID,
        amount: 500_000,
    }
    .into();

    let result = svm.process_instruction(&ix, &[rich_signer_account(from), signer_account(to)]);
    assert!(result.is_ok(), "transfer: {:?}", result.raw_result);
}

#[test]
fn transfer_zero() {
    let mut svm = svm_misc();
    let from = Pubkey::new_unique();
    let to = Pubkey::new_unique();

    let ix: Instruction = TransferTestInstruction {
        from,
        to,
        system_program: quasar_svm::system_program::ID,
        amount: 0,
    }
    .into();

    let result = svm.process_instruction(&ix, &[rich_signer_account(from), signer_account(to)]);
    assert!(result.is_ok(), "transfer zero: {:?}", result.raw_result);
}

#[test]
fn transfer_full_balance() {
    let mut svm = svm_misc();
    let from = Pubkey::new_unique();
    let to = Pubkey::new_unique();
    let balance = 5_000_000u64;

    let ix: Instruction = TransferTestInstruction {
        from,
        to,
        system_program: quasar_svm::system_program::ID,
        amount: balance,
    }
    .into();

    let result = svm.process_instruction(
        &ix,
        &[
            Account {
                address: from,
                lamports: balance,
                data: vec![],
                owner: quasar_svm::system_program::ID,
                executable: false,
            },
            signer_account(to),
        ],
    );
    assert!(result.is_ok(), "full balance: {:?}", result.raw_result);

    let from_acc = result.account(&from).expect("from");
    assert_eq!(from_acc.lamports, 0, "drained");
}

#[test]
fn transfer_to_self_borrow_fail() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();

    let ix: Instruction = TransferTestInstruction {
        from: account,
        to: account,
        system_program: quasar_svm::system_program::ID,
        amount: 100,
    }
    .into();

    // Self-transfer: both `from` and `to` are mutable with the same key.
    // Mutable dups are rejected at parse time to prevent unguarded aliased
    // writes through borrow_unchecked. Use #[account(dup)] to opt in.
    let result = svm.process_instruction(&ix, &[rich_signer_account(account)]);
    assert!(
        result.is_err(),
        "self-transfer should fail (mutable dup rejected)"
    );
}

// assign.

#[test]
fn assign_success() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();
    let new_owner = Pubkey::new_unique();

    let ix: Instruction = AssignTestInstruction {
        account,
        system_program: quasar_svm::system_program::ID,
        owner: new_owner,
    }
    .into();

    let result = svm.process_instruction(&ix, &[signer_account(account)]);
    assert!(result.is_ok(), "assign: {:?}", result.raw_result);

    let acc = result.account(&account).expect("account");
    assert_eq!(acc.owner, new_owner, "new owner");
}

#[test]
fn assign_to_system_program() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();

    let ix: Instruction = AssignTestInstruction {
        account,
        system_program: quasar_svm::system_program::ID,
        owner: quasar_svm::system_program::ID,
    }
    .into();

    let result = svm.process_instruction(&ix, &[signer_account(account)]);
    assert!(result.is_ok(), "assign to system: {:?}", result.raw_result);
}

#[test]
fn transfer_rejects_insufficient_funds() {
    let mut svm = svm_misc();
    let from = Pubkey::new_unique();
    let to = Pubkey::new_unique();

    let ix: Instruction = TransferTestInstruction {
        from,
        to,
        system_program: quasar_svm::system_program::ID,
        amount: 500_000,
    }
    .into();
    let result = svm.process_instruction(
        &ix,
        &[
            Account {
                address: from,
                lamports: 100,
                data: vec![],
                owner: quasar_svm::system_program::ID,
                executable: false,
            },
            signer_account(to),
        ],
    );
    // SystemError::ResultWithNegativeLamports = Custom(1)
    result.assert_error(ProgramError::Custom(1));
}

#[test]
fn transfer_rejects_missing_signer() {
    let mut svm = svm_misc();
    let from = Pubkey::new_unique();
    let to = Pubkey::new_unique();

    let mut ix: Instruction = TransferTestInstruction {
        from,
        to,
        system_program: quasar_svm::system_program::ID,
        amount: 500_000,
    }
    .into();
    // Strip the signer flag from `from`: the framework's header check must
    // reject before the system CPI is reached.
    for meta in &mut ix.accounts {
        if meta.pubkey == from {
            meta.is_signer = false;
        }
    }
    let result = svm.process_instruction(&ix, &[rich_signer_account(from), signer_account(to)]);
    result.assert_error(ProgramError::MissingRequiredSignature);
}
