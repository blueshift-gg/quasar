use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_raw::cpi::*,
};

/// Normal instruction via the typed client — proves the framework pipeline
/// is unaffected by the presence of raw instructions in the same program.
#[test]
fn normal_instruction_works_alongside_raw() {
    let mut svm = svm_raw();
    let signer = Pubkey::new_unique();
    let ix: Instruction = NormalInstruction { signer }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(
        result.is_ok(),
        "normal instruction should succeed: {:?}",
        result.raw_result
    );
}

/// Raw instruction — manually construct the instruction data (discriminator +
/// payload). Passes two accounts: a writable data account and a signer.
/// The raw handler writes a u64 from instruction data into account[0] at offset
/// 8.
#[test]
fn raw_write_succeeds() {
    let mut svm = svm_raw();
    let signer = Pubkey::new_unique();
    let target = Pubkey::new_unique();

    // Create a data account owned by the program with 32 bytes of data.
    let target_account = quasar_svm::Account {
        address: target,
        lamports: 1_000_000,
        data: vec![0u8; 32],
        owner: quasar_test_raw::ID,
        executable: false,
    };

    // Discriminator [1] + u64 payload (42_u64 little-endian).
    let value: u64 = 42;
    let mut data = vec![1u8]; // discriminator
    data.extend_from_slice(&value.to_le_bytes());

    let ix = Instruction {
        program_id: quasar_test_raw::ID,
        accounts: vec![
            quasar_svm::AccountMeta {
                pubkey: target,
                is_signer: false,
                is_writable: true,
            },
            quasar_svm::AccountMeta {
                pubkey: signer,
                is_signer: true,
                is_writable: false,
            },
        ],
        data,
    };

    let result = svm.process_instruction(&ix, &[target_account, signer_account(signer)]);
    assert!(
        result.is_ok(),
        "raw_write should succeed: {:?}",
        result.raw_result
    );

    // Verify the value was written to account data at offset 8.
    let account_after = svm.get_account(&target).expect("account should exist");
    let written = u64::from_le_bytes(account_after.data[8..16].try_into().unwrap());
    assert_eq!(written, 42, "raw handler should write 42 at offset 8");
}

/// Raw instruction fails when signer check fails — account[1] is not a signer.
#[test]
fn raw_write_fails_without_signer() {
    let mut svm = svm_raw();
    let not_signer = Pubkey::new_unique();
    let target = Pubkey::new_unique();

    let target_account = quasar_svm::Account {
        address: target,
        lamports: 1_000_000,
        data: vec![0u8; 32],
        owner: quasar_test_raw::ID,
        executable: false,
    };

    let mut data = vec![1u8];
    data.extend_from_slice(&0u64.to_le_bytes());

    let ix = Instruction {
        program_id: quasar_test_raw::ID,
        accounts: vec![
            quasar_svm::AccountMeta {
                pubkey: target,
                is_signer: false,
                is_writable: true,
            },
            quasar_svm::AccountMeta {
                pubkey: not_signer,
                is_signer: false, // NOT a signer
                is_writable: false,
            },
        ],
        data,
    };

    let non_signer_account = quasar_svm::Account {
        address: not_signer,
        lamports: 1_000_000,
        data: vec![],
        owner: quasar_svm::system_program::ID,
        executable: false,
    };

    let result = svm.process_instruction(&ix, &[target_account, non_signer_account]);
    assert!(
        result.raw_result.is_err(),
        "raw_write should fail without signer"
    );
}

/// Raw + inline asm — the handler uses sBPF ldxdw/stxdw to copy a u64
/// from instruction data into account data at offset 8.
#[test]
fn raw_asm_write_succeeds() {
    let mut svm = svm_raw();
    let signer = Pubkey::new_unique();
    let target = Pubkey::new_unique();

    let target_account = quasar_svm::Account {
        address: target,
        lamports: 1_000_000,
        data: vec![0u8; 32],
        owner: quasar_test_raw::ID,
        executable: false,
    };

    // Discriminator [2] + u64 payload (0xDEADBEEF_CAFEBABE).
    let value: u64 = 0xDEAD_BEEF_CAFE_BABE;
    let mut data = vec![2u8]; // discriminator for raw_asm_write
    data.extend_from_slice(&value.to_le_bytes());

    let ix = Instruction {
        program_id: quasar_test_raw::ID,
        accounts: vec![
            quasar_svm::AccountMeta {
                pubkey: target,
                is_signer: false,
                is_writable: true,
            },
            quasar_svm::AccountMeta {
                pubkey: signer,
                is_signer: true,
                is_writable: false,
            },
        ],
        data,
    };

    let result = svm.process_instruction(&ix, &[target_account, signer_account(signer)]);
    assert!(
        result.is_ok(),
        "raw_asm_write should succeed: {:?}",
        result.raw_result
    );

    // Verify the asm wrote the value at offset 8.
    let account_after = svm.get_account(&target).expect("account should exist");
    let written = u64::from_le_bytes(account_after.data[8..16].try_into().unwrap());
    assert_eq!(
        written, 0xDEAD_BEEF_CAFE_BABE,
        "inline asm should write 0xDEADBEEFCAFEBABE at offset 8"
    );
}

/// Raw instruction fails with too-short instruction data.
#[test]
fn raw_write_fails_with_short_data() {
    let mut svm = svm_raw();
    let signer = Pubkey::new_unique();
    let target = Pubkey::new_unique();

    let target_account = quasar_svm::Account {
        address: target,
        lamports: 1_000_000,
        data: vec![0u8; 32],
        owner: quasar_test_raw::ID,
        executable: false,
    };

    // Discriminator [1] + only 4 bytes (needs 8).
    let data = vec![1u8, 0, 0, 0, 0];

    let ix = Instruction {
        program_id: quasar_test_raw::ID,
        accounts: vec![
            quasar_svm::AccountMeta {
                pubkey: target,
                is_signer: false,
                is_writable: true,
            },
            quasar_svm::AccountMeta {
                pubkey: signer,
                is_signer: true,
                is_writable: false,
            },
        ],
        data,
    };

    let result = svm.process_instruction(&ix, &[target_account, signer_account(signer)]);
    assert!(
        result.raw_result.is_err(),
        "raw_write should fail with short data"
    );
}

// ===========================================================================
// callx dispatch — proves the SVM accepts indirect function calls via
// function pointer tables. Foundation for O(1) raw instruction dispatch.
// ===========================================================================

#[test]
fn callx_dispatch_selector_0_writes_aa() {
    let mut svm = svm_raw();
    let target = Pubkey::new_unique();

    let target_account = quasar_svm::Account {
        address: target,
        lamports: 1_000_000,
        data: vec![0u8; 32],
        owner: quasar_test_raw::ID,
        executable: false,
    };

    // Discriminator [5] + selector byte 0 → write_aa → 0xAA at offset 8
    let data = vec![5u8, 0];

    let ix = Instruction {
        program_id: quasar_test_raw::ID,
        accounts: vec![quasar_svm::AccountMeta {
            pubkey: target,
            is_signer: false,
            is_writable: true,
        }],
        data,
    };

    let result = svm.process_instruction(&ix, &[target_account]);
    assert!(
        result.is_ok(),
        "callx dispatch selector 0 should succeed: {:?}",
        result.raw_result
    );

    let account_after = svm.get_account(&target).expect("account should exist");
    assert_eq!(
        account_after.data[8], 0xAA,
        "selector 0 should write 0xAA at offset 8"
    );
}

#[test]
fn callx_dispatch_selector_1_writes_bb() {
    let mut svm = svm_raw();
    let target = Pubkey::new_unique();

    let target_account = quasar_svm::Account {
        address: target,
        lamports: 1_000_000,
        data: vec![0u8; 32],
        owner: quasar_test_raw::ID,
        executable: false,
    };

    // Discriminator [5] + selector byte 1 → write_bb → 0xBB at offset 8
    let data = vec![5u8, 1];

    let ix = Instruction {
        program_id: quasar_test_raw::ID,
        accounts: vec![quasar_svm::AccountMeta {
            pubkey: target,
            is_signer: false,
            is_writable: true,
        }],
        data,
    };

    let result = svm.process_instruction(&ix, &[target_account]);
    assert!(
        result.is_ok(),
        "callx dispatch selector 1 should succeed: {:?}",
        result.raw_result
    );

    let account_after = svm.get_account(&target).expect("account should exist");
    assert_eq!(
        account_after.data[8], 0xBB,
        "selector 1 should write 0xBB at offset 8"
    );
}
