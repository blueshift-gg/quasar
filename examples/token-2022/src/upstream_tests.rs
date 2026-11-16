//! Core processor regression tests from the pinned upstream revision.

use {
    super::*,
    mollusk_svm::result::Check,
    solana_account::{Account as SolanaAccount, ReadableAccount},
    solana_account_info::AccountInfo,
    solana_account_info::IntoAccountInfo,
    solana_instruction::{AccountMeta, Instruction},
    solana_program_option::COption,
    solana_rent_legacy::Rent,
    solana_sdk_ids::system_program,
    solana_sdk_ids::sysvar::{self, rent},
    spl_token_2022_interface::{
        extension::{
            permissioned_burn, transfer_fee::instruction::initialize_transfer_fee_config,
            ExtensionType,
        },
        instruction::*,
        state::{Account, Multisig},
    },
    std::collections::HashMap,
};

fn do_process_instruction(
    instruction: Instruction,
    mut accounts: Vec<&mut SolanaAccount>,
    checks: &[Check],
) {
    // Prepare accounts for mollusk.
    let instruction_accounts: Vec<(Address, SolanaAccount)> = instruction
        .accounts
        .iter()
        .zip(&accounts)
        .map(|(account_meta, account)| (account_meta.pubkey, (*account).clone()))
        .collect();

    let mut mollusk = crate::runtime::testing::mollusk();
    mollusk.sysvars.rent = test_rent();
    // Always panic on failed checks in tests.
    mollusk.config.panic = true;
    // Process the instruction and validate the result. It panics if the result does not
    // match the expected checks.
    let result =
        mollusk.process_and_validate_instruction(&instruction, &instruction_accounts, checks);

    // Update accounts after the instruction is processed.
    for (original, (_, updated)) in accounts.iter_mut().zip(result.resulting_accounts) {
        original.data = updated.data().to_vec();
        original.lamports = updated.lamports();
        original.owner = *updated.owner();
    }
}

fn do_process_instruction_dups(
    instruction: Instruction,
    account_infos: Vec<AccountInfo>,
    checks: &[Check],
) {
    let mut cached_accounts = HashMap::new();
    let mut dedup_accounts = Vec::new();

    // Dedup accounts for mollusk.
    account_infos.iter().for_each(|account_info| {
        if !cached_accounts.contains_key(account_info.key) {
            let account = SolanaAccount {
                lamports: account_info.lamports(),
                data: account_info.try_borrow_data().unwrap().to_vec(),
                owner: *account_info.owner,
                executable: account_info.executable,
                rent_epoch: u64::MAX,
            };
            dedup_accounts.push((*account_info.key, account));
            cached_accounts.insert(account_info.key, account_info);
        }
    });

    let mut mollusk = crate::runtime::testing::mollusk();
    mollusk.sysvars.rent = test_rent();
    // Always panic on failed checks in tests.
    mollusk.config.panic = true;
    // Process the instruction and validate the result. It panics if the result does not
    // match the expected checks.
    let result = mollusk.process_and_validate_instruction(&instruction, &dedup_accounts, checks);

    // Update accounts after the instruction is processed.
    result.resulting_accounts.into_iter().for_each(|(pubkey, account)| {
        let account_info = cached_accounts.get_mut(&pubkey).unwrap();
        if account.data().is_empty() {
            // Set an empty data slice for accounts that have been resized
            // to zero length.
            *account_info.try_borrow_mut_data().unwrap() = &mut [];
        } else {
            account_info.try_borrow_mut_data().unwrap().copy_from_slice(account.data());
        }
        **account_info.try_borrow_mut_lamports().unwrap() = account.lamports();
        account_info.assign(account.owner());
    });
}

fn return_token_error_as_program_error() -> ProgramError {
    TokenError::MintMismatch.into()
}

fn test_rent() -> solana_rent_legacy::Rent {
    solana_rent_legacy::Rent::default()
}

fn rent_sysvar() -> SolanaAccount {
    SolanaAccount::new_data_with_space(1, &test_rent(), 17, &sysvar::id()).unwrap()
}

fn mint_minimum_balance() -> u64 {
    solana_rent_legacy::Rent::default().minimum_balance(Mint::get_packed_len())
}

fn account_minimum_balance() -> u64 {
    solana_rent_legacy::Rent::default().minimum_balance(Account::get_packed_len())
}

fn multisig_minimum_balance() -> u64 {
    solana_rent_legacy::Rent::default().minimum_balance(Multisig::get_packed_len())
}

fn batch_instruction(instructions: &[&Instruction]) -> Result<Instruction, ProgramError> {
    // Create a `Vec` of ordered `AccountMeta`s
    let mut accounts: Vec<AccountMeta> = vec![];
    // Start with the batch discriminator
    let mut data: Vec<u8> = vec![0xff];

    for instruction in instructions {
        // Error out on non-token IX.
        if instruction.program_id.ne(&crate::ID) {
            return Err(ProgramError::IncorrectProgramId);
        }

        data.push(instruction.accounts.len() as u8);
        data.push(instruction.data.len() as u8);

        data.extend_from_slice(&instruction.data);
        accounts.extend_from_slice(&instruction.accounts);
    }

    Ok(Instruction { program_id: crate::ID, data, accounts })
}

fn native_mint() -> SolanaAccount {
    let mut rent_sysvar = rent_sysvar();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &crate::id());
    do_process_instruction(
        initialize_mint(
            &crate::id(),
            &crate::native_mint::id(),
            &Address::default(),
            None,
            crate::native_mint::DECIMALS,
        )
        .unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );
    mint_account
}

fn native_mint_to(account: &AccountInfo, amount: u64) -> ProgramResult {
    let mut buffer = account.try_borrow_mut_data()?;
    let mut account_account = Account::unpack(&buffer)?;

    if !account_account.is_native() {
        return Err(TokenError::NonNativeNotSupported.into());
    }

    let mut lamports = account.try_borrow_mut_lamports()?;
    **lamports += amount;

    account_account.amount += amount;

    Account::pack(account_account, &mut buffer)?;

    Ok(())
}

#[test]
fn test_error_as_custom() {
    assert_eq!(return_token_error_as_program_error(), ProgramError::Custom(3));
}

#[test]
fn test_unique_account_sizes() {
    assert_ne!(Mint::get_packed_len(), 0);
    assert_ne!(Mint::get_packed_len(), Account::get_packed_len());
    assert_ne!(Mint::get_packed_len(), Multisig::get_packed_len());
    assert_ne!(Account::get_packed_len(), 0);
    assert_ne!(Account::get_packed_len(), Multisig::get_packed_len());
    assert_ne!(Multisig::get_packed_len(), 0);
}

#[test]
fn test_initialize_mint() {
    let program_id = crate::id();
    let owner_key = Address::new_unique();
    let mint_key = Address::new_unique();
    let mut mint_account = SolanaAccount::new(42, Mint::get_packed_len(), &program_id);
    let mint2_key = Address::new_unique();
    let mut mint2_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // mint is not rent exempt
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::err(TokenError::NotRentExempt.into())],
    );

    mint_account.lamports = mint_minimum_balance();

    // create new mint
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create twice
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::err(TokenError::AlreadyInUse.into())],
    );

    // create another mint that can freeze
    do_process_instruction(
        initialize_mint(&program_id, &mint2_key, &owner_key, Some(&owner_key), 2).unwrap(),
        vec![&mut mint2_account, &mut rent_sysvar],
        &[Check::success()],
    );
    let mint = Mint::unpack_unchecked(&mint2_account.data).unwrap();
    assert_eq!(mint.freeze_authority, COption::Some(owner_key));
}

#[test]
fn test_initialize_mint2() {
    let program_id = crate::id();
    let owner_key = Address::new_unique();
    let mint_key = Address::new_unique();
    let mut mint_account = SolanaAccount::new(42, Mint::get_packed_len(), &program_id);
    let mint2_key = Address::new_unique();
    let mut mint2_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);

    // mint is not rent exempt
    do_process_instruction(
        initialize_mint2(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account],
        &[Check::err(TokenError::NotRentExempt.into())],
    );

    mint_account.lamports = mint_minimum_balance();

    // create new mint
    do_process_instruction(
        initialize_mint2(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account],
        &[Check::success()],
    );

    // create twice
    do_process_instruction(
        initialize_mint2(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account],
        &[Check::err(TokenError::AlreadyInUse.into())],
    );

    // create another mint that can freeze
    do_process_instruction(
        initialize_mint2(&program_id, &mint2_key, &owner_key, Some(&owner_key), 2).unwrap(),
        vec![&mut mint2_account],
        &[Check::success()],
    );
    let mint = Mint::unpack_unchecked(&mint2_account.data).unwrap();
    assert_eq!(mint.freeze_authority, COption::Some(owner_key));
}

#[test]
fn test_initialize_mint_account() {
    let program_id = crate::id();
    let account_key = Address::new_unique();
    let mut account_account = SolanaAccount::new(42, Account::get_packed_len(), &program_id);
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // account is not rent exempt
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::err(TokenError::NotRentExempt.into())],
    );

    account_account.lamports = account_minimum_balance();

    // mint is not valid (not initialized)
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::err(TokenError::InvalidMint.into())],
    );

    // create mint
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // mint not owned by program
    let not_program_id = Address::new_unique();
    mint_account.owner = not_program_id;
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::err(ProgramError::IncorrectProgramId)],
    );
    mint_account.owner = program_id;

    // create account
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create twice
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::err(TokenError::AlreadyInUse.into())],
    );
}

#[test]
fn test_transfer_dups() {
    let program_id = crate::id();
    let account1_key = Address::new_unique();
    let mut account1_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let mut account1_info: AccountInfo = (&account1_key, true, &mut account1_account).into();
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let mut account2_info: AccountInfo = (&account2_key, false, &mut account2_account).into();
    let account3_key = Address::new_unique();
    let mut account3_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account3_info: AccountInfo = (&account3_key, false, &mut account3_account).into();
    let account4_key = Address::new_unique();
    let mut account4_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account4_info: AccountInfo = (&account4_key, true, &mut account4_account).into();
    let multisig_key = Address::new_unique();
    let mut multisig_account =
        SolanaAccount::new(multisig_minimum_balance(), Multisig::get_packed_len(), &program_id);
    let multisig_info: AccountInfo = (&multisig_key, true, &mut multisig_account).into();
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner_info: AccountInfo = (&owner_key, true, &mut owner_account).into();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint_info: AccountInfo = (&mint_key, false, &mut mint_account).into();
    let rent_key = rent::id();
    let mut rent_sysvar = rent_sysvar();
    let rent_info: AccountInfo = (&rent_key, false, &mut rent_sysvar).into();

    // create mint
    do_process_instruction_dups(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![mint_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // create account
    do_process_instruction_dups(
        initialize_account(&program_id, &account1_key, &mint_key, &account1_key).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), account1_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // create another account
    do_process_instruction_dups(
        initialize_account(&program_id, &account2_key, &mint_key, &owner_key).unwrap(),
        vec![account2_info.clone(), mint_info.clone(), owner_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // mint to account
    do_process_instruction_dups(
        mint_to(&program_id, &mint_key, &account1_key, &owner_key, &[], 1000).unwrap(),
        vec![mint_info.clone(), account1_info.clone(), owner_info.clone()],
        &[Check::success()],
    );

    // source-owner transfer
    do_process_instruction_dups(
        #[allow(deprecated)]
        transfer(&program_id, &account1_key, &account2_key, &account1_key, &[], 500).unwrap(),
        vec![account1_info.clone(), account2_info.clone(), account1_info.clone()],
        &[Check::success()],
    );

    // source-owner TransferChecked
    do_process_instruction_dups(
        transfer_checked(
            &program_id,
            &account1_key,
            &mint_key,
            &account2_key,
            &account1_key,
            &[],
            500,
            2,
        )
        .unwrap(),
        vec![
            account1_info.clone(),
            mint_info.clone(),
            account2_info.clone(),
            account1_info.clone(),
        ],
        &[Check::success()],
    );

    // source-delegate transfer
    let mut account = Account::unpack_unchecked(&account1_info.data.borrow()).unwrap();
    account.amount = 1000;
    account.delegated_amount = 1000;
    account.delegate = COption::Some(account1_key);
    account.owner = owner_key;
    Account::pack(account, &mut account1_info.data.borrow_mut()).unwrap();

    do_process_instruction_dups(
        #[allow(deprecated)]
        transfer(&program_id, &account1_key, &account2_key, &account1_key, &[], 500).unwrap(),
        vec![account1_info.clone(), account2_info.clone(), account1_info.clone()],
        &[Check::success()],
    );

    // source-delegate TransferChecked
    do_process_instruction_dups(
        transfer_checked(
            &program_id,
            &account1_key,
            &mint_key,
            &account2_key,
            &account1_key,
            &[],
            500,
            2,
        )
        .unwrap(),
        vec![
            account1_info.clone(),
            mint_info.clone(),
            account2_info.clone(),
            account1_info.clone(),
        ],
        &[Check::success()],
    );

    // test destination-owner transfer
    do_process_instruction_dups(
        initialize_account(&program_id, &account3_key, &mint_key, &account2_key).unwrap(),
        vec![account3_info.clone(), mint_info.clone(), account2_info.clone(), rent_info.clone()],
        &[Check::success()],
    );
    do_process_instruction_dups(
        mint_to(&program_id, &mint_key, &account3_key, &owner_key, &[], 1000).unwrap(),
        vec![mint_info.clone(), account3_info.clone(), owner_info.clone()],
        &[Check::success()],
    );

    account1_info.is_signer = false;
    account2_info.is_signer = true;
    do_process_instruction_dups(
        #[allow(deprecated)]
        transfer(&program_id, &account3_key, &account2_key, &account2_key, &[], 500).unwrap(),
        vec![account3_info.clone(), account2_info.clone(), account2_info.clone()],
        &[Check::success()],
    );

    // destination-owner TransferChecked
    do_process_instruction_dups(
        transfer_checked(
            &program_id,
            &account3_key,
            &mint_key,
            &account2_key,
            &account2_key,
            &[],
            500,
            2,
        )
        .unwrap(),
        vec![
            account3_info.clone(),
            mint_info.clone(),
            account2_info.clone(),
            account2_info.clone(),
        ],
        &[Check::success()],
    );

    // test source-multisig signer
    do_process_instruction_dups(
        initialize_multisig(&program_id, &multisig_key, &[&account4_key], 1).unwrap(),
        vec![multisig_info.clone(), rent_info.clone(), account4_info.clone()],
        &[Check::success()],
    );

    do_process_instruction_dups(
        initialize_account(&program_id, &account4_key, &mint_key, &multisig_key).unwrap(),
        vec![account4_info.clone(), mint_info.clone(), multisig_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    do_process_instruction_dups(
        mint_to(&program_id, &mint_key, &account4_key, &owner_key, &[], 1000).unwrap(),
        vec![mint_info.clone(), account4_info.clone(), owner_info.clone()],
        &[Check::success()],
    );

    // source-multisig-signer transfer
    do_process_instruction_dups(
        #[allow(deprecated)]
        transfer(&program_id, &account4_key, &account2_key, &multisig_key, &[&account4_key], 500)
            .unwrap(),
        vec![
            account4_info.clone(),
            account2_info.clone(),
            multisig_info.clone(),
            account4_info.clone(),
        ],
        &[Check::success()],
    );

    // source-multisig-signer TransferChecked
    do_process_instruction_dups(
        transfer_checked(
            &program_id,
            &account4_key,
            &mint_key,
            &account2_key,
            &multisig_key,
            &[&account4_key],
            500,
            2,
        )
        .unwrap(),
        vec![
            account4_info.clone(),
            mint_info.clone(),
            account2_info.clone(),
            multisig_info.clone(),
            account4_info.clone(),
        ],
        &[Check::success()],
    );
}

#[test]
fn test_transfer() {
    let program_id = crate::id();
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account3_key = Address::new_unique();
    let mut account3_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let delegate_key = Address::new_unique();
    let mut delegate_account = SolanaAccount::default();
    let mismatch_key = Address::new_unique();
    let mut mismatch_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner2_key = Address::new_unique();
    let mut owner2_account = SolanaAccount::default();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint2_key = Address::new_unique();
    let mut rent_sysvar = rent_sysvar();

    // create mint
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create account
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create another account
    do_process_instruction(
        initialize_account(&program_id, &account2_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account2_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create another account
    do_process_instruction(
        initialize_account(&program_id, &account3_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account3_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create mismatch account
    do_process_instruction(
        initialize_account(&program_id, &mismatch_key, &mint_key, &owner_key).unwrap(),
        vec![&mut mismatch_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );
    let mut account = Account::unpack_unchecked(&mismatch_account.data).unwrap();
    account.mint = mint2_key;
    Account::pack(account, &mut mismatch_account.data).unwrap();

    // mint to account
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &owner_key, &[], 1000).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account],
        &[Check::success()],
    );

    // missing signer
    #[allow(deprecated)]
    let mut instruction =
        transfer(&program_id, &account_key, &account2_key, &owner_key, &[], 1000).unwrap();
    instruction.accounts[2].is_signer = false;
    do_process_instruction(
        instruction,
        vec![&mut account_account, &mut account2_account, &mut owner_account],
        &[Check::err(ProgramError::MissingRequiredSignature)],
    );

    // mismatch mint
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &mismatch_key, &owner_key, &[], 1000).unwrap(),
        vec![&mut account_account, &mut mismatch_account, &mut owner_account],
        &[Check::err(TokenError::MintMismatch.into())],
    );

    // missing owner
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &account2_key, &owner2_key, &[], 1000).unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner2_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // account not owned by program
    let not_program_id = Address::new_unique();
    account_account.owner = not_program_id;
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &account2_key, &owner_key, &[], 0).unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner2_account],
        &[Check::err(ProgramError::IncorrectProgramId)],
    );
    account_account.owner = program_id;

    // account 2 not owned by program
    let not_program_id = Address::new_unique();
    account2_account.owner = not_program_id;
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &account2_key, &owner_key, &[], 0).unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner2_account],
        &[Check::err(ProgramError::IncorrectProgramId)],
    );
    account2_account.owner = program_id;

    // transfer
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &account2_key, &owner_key, &[], 1000).unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner_account],
        &[Check::success()],
    );

    // insufficient funds
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &account2_key, &owner_key, &[], 1).unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner_account],
        &[Check::err(TokenError::InsufficientFunds.into())],
    );

    // transfer half back
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account2_key, &account_key, &owner_key, &[], 500).unwrap(),
        vec![&mut account2_account, &mut account_account, &mut owner_account],
        &[Check::success()],
    );

    // incorrect decimals
    do_process_instruction(
        transfer_checked(
            &program_id,
            &account2_key,
            &mint_key,
            &account_key,
            &owner_key,
            &[],
            1,
            10, // <-- incorrect decimals
        )
        .unwrap(),
        vec![&mut account2_account, &mut mint_account, &mut account_account, &mut owner_account],
        &[Check::err(TokenError::MintDecimalsMismatch.into())],
    );

    // incorrect mint
    do_process_instruction(
        transfer_checked(
            &program_id,
            &account2_key,
            &account3_key, // <-- incorrect mint
            &account_key,
            &owner_key,
            &[],
            1,
            2,
        )
        .unwrap(),
        vec![
            &mut account2_account,
            &mut account3_account, // <-- incorrect mint
            &mut account_account,
            &mut owner_account,
        ],
        &[Check::err(TokenError::MintMismatch.into())],
    );
    // transfer rest with explicit decimals
    do_process_instruction(
        transfer_checked(
            &program_id,
            &account2_key,
            &mint_key,
            &account_key,
            &owner_key,
            &[],
            500,
            2,
        )
        .unwrap(),
        vec![&mut account2_account, &mut mint_account, &mut account_account, &mut owner_account],
        &[Check::success()],
    );

    // insufficient funds
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account2_key, &account_key, &owner_key, &[], 1).unwrap(),
        vec![&mut account2_account, &mut account_account, &mut owner_account],
        &[Check::err(TokenError::InsufficientFunds.into())],
    );

    // approve delegate
    do_process_instruction(
        approve(&program_id, &account_key, &delegate_key, &owner_key, &[], 100).unwrap(),
        vec![&mut account_account, &mut delegate_account, &mut owner_account],
        &[Check::success()],
    );

    // not a delegate of source account
    do_process_instruction(
        #[allow(deprecated)]
        transfer(
            &program_id,
            &account_key,
            &account2_key,
            &owner2_key, // <-- incorrect owner or delegate
            &[],
            1,
        )
        .unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner2_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // insufficient funds approved via delegate
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &account2_key, &delegate_key, &[], 101).unwrap(),
        vec![&mut account_account, &mut account2_account, &mut delegate_account],
        &[Check::err(TokenError::InsufficientFunds.into())],
    );

    // transfer via delegate
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &account2_key, &delegate_key, &[], 100).unwrap(),
        vec![&mut account_account, &mut account2_account, &mut delegate_account],
        &[Check::success()],
    );

    // insufficient funds approved via delegate
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &account2_key, &delegate_key, &[], 1).unwrap(),
        vec![&mut account_account, &mut account2_account, &mut delegate_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // transfer rest
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &account2_key, &owner_key, &[], 900).unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner_account],
        &[Check::success()],
    );

    // approve delegate
    do_process_instruction(
        approve(&program_id, &account_key, &delegate_key, &owner_key, &[], 100).unwrap(),
        vec![&mut account_account, &mut delegate_account, &mut owner_account],
        &[Check::success()],
    );

    // insufficient funds in source account via delegate
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &account2_key, &delegate_key, &[], 100).unwrap(),
        vec![&mut account_account, &mut account2_account, &mut delegate_account],
        &[Check::err(TokenError::InsufficientFunds.into())],
    );
}

#[test]
fn test_self_transfer() {
    let program_id = crate::id();
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account3_key = Address::new_unique();
    let mut account3_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let delegate_key = Address::new_unique();
    let mut delegate_account = SolanaAccount::default();
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner2_key = Address::new_unique();
    let mut owner2_account = SolanaAccount::default();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // create mint
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create account
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create another account
    do_process_instruction(
        initialize_account(&program_id, &account2_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account2_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create another account
    do_process_instruction(
        initialize_account(&program_id, &account3_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account3_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // mint to account
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &owner_key, &[], 1000).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account],
        &[Check::success()],
    );

    let account_info = (&account_key, false, &mut account_account).into_account_info();
    let account3_info = (&account3_key, false, &mut account3_account).into_account_info();
    let delegate_info = (&delegate_key, true, &mut delegate_account).into_account_info();
    let owner_info = (&owner_key, true, &mut owner_account).into_account_info();
    let owner2_info = (&owner2_key, true, &mut owner2_account).into_account_info();
    let mint_info = (&mint_key, false, &mut mint_account).into_account_info();

    // transfer
    #[allow(deprecated)]
    let instruction =
        transfer(&program_id, account_info.key, account_info.key, owner_info.key, &[], 1000)
            .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), account_info.clone(), owner_info.clone()],
        &[Check::success()],
    );
    // no balance change...
    let account = Account::unpack_unchecked(&account_info.try_borrow_data().unwrap()).unwrap();
    assert_eq!(account.amount, 1000);

    // transfer checked
    let instruction = transfer_checked(
        &program_id,
        account_info.key,
        mint_info.key,
        account_info.key,
        owner_info.key,
        &[],
        1000,
        2,
    )
    .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), mint_info.clone(), account_info.clone(), owner_info.clone()],
        &[Check::success()],
    );
    // no balance change...
    let account = Account::unpack_unchecked(&account_info.try_borrow_data().unwrap()).unwrap();
    assert_eq!(account.amount, 1000);

    // missing signer
    let mut owner_no_sign_info = owner_info.clone();
    #[allow(deprecated)]
    let mut instruction = transfer(
        &program_id,
        account_info.key,
        account_info.key,
        owner_no_sign_info.key,
        &[],
        1000,
    )
    .unwrap();
    instruction.accounts[2].is_signer = false;
    owner_no_sign_info.is_signer = false;
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), account_info.clone(), owner_no_sign_info.clone()],
        &[Check::err(ProgramError::MissingRequiredSignature)],
    );

    // missing signer checked
    let mut instruction = transfer_checked(
        &program_id,
        account_info.key,
        mint_info.key,
        account_info.key,
        owner_no_sign_info.key,
        &[],
        1000,
        2,
    )
    .unwrap();
    instruction.accounts[3].is_signer = false;
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), mint_info.clone(), account_info.clone(), owner_no_sign_info],
        &[Check::err(ProgramError::MissingRequiredSignature)],
    );

    // missing owner
    #[allow(deprecated)]
    let instruction =
        transfer(&program_id, account_info.key, account_info.key, owner2_info.key, &[], 1000)
            .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), account_info.clone(), owner2_info.clone()],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // missing owner checked
    let instruction = transfer_checked(
        &program_id,
        account_info.key,
        mint_info.key,
        account_info.key,
        owner2_info.key,
        &[],
        1000,
        2,
    )
    .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), mint_info.clone(), account_info.clone(), owner2_info.clone()],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // insufficient funds
    #[allow(deprecated)]
    let instruction =
        transfer(&program_id, account_info.key, account_info.key, owner_info.key, &[], 1001)
            .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), account_info.clone(), owner_info.clone()],
        &[Check::err(TokenError::InsufficientFunds.into())],
    );

    // insufficient funds checked
    let instruction = transfer_checked(
        &program_id,
        account_info.key,
        mint_info.key,
        account_info.key,
        owner_info.key,
        &[],
        1001,
        2,
    )
    .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), mint_info.clone(), account_info.clone(), owner_info.clone()],
        &[Check::err(TokenError::InsufficientFunds.into())],
    );

    // incorrect decimals
    let instruction = transfer_checked(
        &program_id,
        account_info.key,
        mint_info.key,
        account_info.key,
        owner_info.key,
        &[],
        1,
        10, // <-- incorrect decimals
    )
    .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), mint_info.clone(), account_info.clone(), owner_info.clone()],
        &[Check::err(TokenError::MintDecimalsMismatch.into())],
    );

    // incorrect mint
    let instruction = transfer_checked(
        &program_id,
        account_info.key,
        account3_info.key, // <-- incorrect mint
        account_info.key,
        owner_info.key,
        &[],
        1,
        2,
    )
    .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![
            account_info.clone(),
            account3_info.clone(), // <-- incorrect mint
            account_info.clone(),
            owner_info.clone(),
        ],
        &[Check::err(TokenError::MintMismatch.into())],
    );

    // approve delegate
    let instruction =
        approve(&program_id, account_info.key, delegate_info.key, owner_info.key, &[], 100)
            .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), delegate_info.clone(), owner_info.clone()],
        &[Check::success()],
    );

    // delegate transfer
    #[allow(deprecated)]
    let instruction =
        transfer(&program_id, account_info.key, account_info.key, delegate_info.key, &[], 100)
            .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), account_info.clone(), delegate_info.clone()],
        &[Check::success()],
    );
    // no balance change...
    let account = Account::unpack_unchecked(&account_info.try_borrow_data().unwrap()).unwrap();
    assert_eq!(account.amount, 1000);
    assert_eq!(account.delegated_amount, 100);

    // delegate transfer checked
    let instruction = transfer_checked(
        &program_id,
        account_info.key,
        mint_info.key,
        account_info.key,
        delegate_info.key,
        &[],
        100,
        2,
    )
    .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), mint_info.clone(), account_info.clone(), delegate_info.clone()],
        &[Check::success()],
    );
    // no balance change...
    let account = Account::unpack_unchecked(&account_info.try_borrow_data().unwrap()).unwrap();
    assert_eq!(account.amount, 1000);
    assert_eq!(account.delegated_amount, 100);

    // delegate insufficient funds
    #[allow(deprecated)]
    let instruction =
        transfer(&program_id, account_info.key, account_info.key, delegate_info.key, &[], 101)
            .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), account_info.clone(), delegate_info.clone()],
        &[Check::err(TokenError::InsufficientFunds.into())],
    );

    // delegate insufficient funds checked
    let instruction = transfer_checked(
        &program_id,
        account_info.key,
        mint_info.key,
        account_info.key,
        delegate_info.key,
        &[],
        101,
        2,
    )
    .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), mint_info.clone(), account_info.clone(), delegate_info.clone()],
        &[Check::err(TokenError::InsufficientFunds.into())],
    );

    // owner transfer with delegate assigned
    #[allow(deprecated)]
    let instruction =
        transfer(&program_id, account_info.key, account_info.key, owner_info.key, &[], 1000)
            .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), account_info.clone(), owner_info.clone()],
        &[Check::success()],
    );
    // no balance change...
    let account = Account::unpack_unchecked(&account_info.try_borrow_data().unwrap()).unwrap();
    assert_eq!(account.amount, 1000);

    // owner transfer with delegate assigned checked
    let instruction = transfer_checked(
        &program_id,
        account_info.key,
        mint_info.key,
        account_info.key,
        owner_info.key,
        &[],
        1000,
        2,
    )
    .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), mint_info.clone(), account_info.clone(), owner_info.clone()],
        &[Check::success()],
    );
    // no balance change...
    let account = Account::unpack_unchecked(&account_info.try_borrow_data().unwrap()).unwrap();
    assert_eq!(account.amount, 1000);
}

#[test]
fn test_unwrap_lamports_dups() {
    let program_id = crate::id();
    let account1_key = Address::new_unique();
    let mut account1_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let mut account1_info: AccountInfo = (&account1_key, true, &mut account1_account).into();
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let mut account2_info: AccountInfo = (&account2_key, false, &mut account2_account).into();
    let account3_key = Address::new_unique();
    let mut account3_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account3_info: AccountInfo = (&account3_key, false, &mut account3_account).into();
    let account4_key = Address::new_unique();
    let mut account4_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account4_info: AccountInfo = (&account4_key, true, &mut account4_account).into();
    let multisig_key = Address::new_unique();
    let mut multisig_account =
        SolanaAccount::new(multisig_minimum_balance(), Multisig::get_packed_len(), &program_id);
    let multisig_info: AccountInfo = (&multisig_key, true, &mut multisig_account).into();
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner_info: AccountInfo = (&owner_key, true, &mut owner_account).into();
    let mint_key = native_mint::id();
    let mut mint_account = native_mint();
    let mint_info: AccountInfo = (&mint_key, false, &mut mint_account).into();
    let rent_key = rent::id();
    let mut rent_sysvar = rent_sysvar();
    let rent_info: AccountInfo = (&rent_key, false, &mut rent_sysvar).into();

    // create account
    do_process_instruction_dups(
        initialize_account(&program_id, &account1_key, &mint_key, &account1_key).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), account1_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // create another account
    do_process_instruction_dups(
        initialize_account(&program_id, &account2_key, &mint_key, &owner_key).unwrap(),
        vec![account2_info.clone(), mint_info.clone(), owner_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // mint to account
    native_mint_to(&account1_info, 1000).unwrap();

    // source-owner unwrap lamports
    do_process_instruction_dups(
        unwrap_lamports(&program_id, &account1_key, &account2_key, &account1_key, &[], Some(500))
            .unwrap(),
        vec![account1_info.clone(), account2_info.clone(), account1_info.clone()],
        &[Check::success()],
    );

    // test destination-owner unwrap lamports
    do_process_instruction_dups(
        initialize_account(&program_id, &account3_key, &mint_key, &account2_key).unwrap(),
        vec![account3_info.clone(), mint_info.clone(), account2_info.clone(), rent_info.clone()],
        &[Check::success()],
    );
    native_mint_to(&account3_info, 1000).unwrap();

    account1_info.is_signer = false;
    account2_info.is_signer = true;
    do_process_instruction_dups(
        unwrap_lamports(&program_id, &account3_key, &account2_key, &account2_key, &[], Some(500))
            .unwrap(),
        vec![account3_info.clone(), account2_info.clone(), account2_info.clone()],
        &[Check::success()],
    );

    // test source-multisig signer
    do_process_instruction_dups(
        initialize_multisig(&program_id, &multisig_key, &[&account4_key], 1).unwrap(),
        vec![multisig_info.clone(), rent_info.clone(), account4_info.clone()],
        &[Check::success()],
    );

    do_process_instruction_dups(
        initialize_account(&program_id, &account4_key, &mint_key, &multisig_key).unwrap(),
        vec![account4_info.clone(), mint_info.clone(), multisig_info.clone(), rent_info.clone()],
        &[Check::success()],
    );
    native_mint_to(&account4_info, 1000).unwrap();

    // source-multisig-signer unwrap lamports
    do_process_instruction_dups(
        unwrap_lamports(
            &program_id,
            &account4_key,
            &account2_key,
            &multisig_key,
            &[&account4_key],
            Some(500),
        )
        .unwrap(),
        vec![
            account4_info.clone(),
            account2_info.clone(),
            multisig_info.clone(),
            account4_info.clone(),
        ],
        &[Check::success()],
    );
}

#[test]
fn test_unwrap_lamports() {
    let program_id = crate::id();
    let zero_space_rent_exempt_balance = Rent::default().minimum_balance(0);
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(zero_space_rent_exempt_balance, Account::get_packed_len(), &program_id);
    let mismatch_account_key = Address::new_unique();
    let mut mismatch_account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner2_key = Address::new_unique();
    let mut owner2_account = SolanaAccount::default();
    let mint_key = native_mint::id();
    let mut mint_account = native_mint();
    let mismatch_mint_key = Address::new_unique();
    let mut mismatch_mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // create mismatch mint
    do_process_instruction(
        initialize_mint(&program_id, &mismatch_mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mismatch_mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create account
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create mismatch account
    do_process_instruction(
        initialize_account(&program_id, &mismatch_account_key, &mismatch_mint_key, &owner_key)
            .unwrap(),
        vec![
            &mut mismatch_account_account,
            &mut mismatch_mint_account,
            &mut owner_account,
            &mut rent_sysvar,
        ],
        &[Check::success()],
    );

    // mint to account
    let account_info = (&account_key, false, &mut account_account).into();
    native_mint_to(&account_info, 1000).unwrap();

    // missing signer
    let mut instruction =
        unwrap_lamports(&program_id, &account_key, &account2_key, &owner_key, &[], Some(1000))
            .unwrap();
    instruction.accounts[2].is_signer = false;
    do_process_instruction(
        instruction,
        vec![&mut account_account, &mut account2_account, &mut owner_account],
        &[Check::err(ProgramError::MissingRequiredSignature)],
    );

    // non native mint
    do_process_instruction(
        unwrap_lamports(&program_id, &mismatch_account_key, &account_key, &owner_key, &[], None)
            .unwrap(),
        vec![&mut mismatch_account_account, &mut account_account, &mut owner_account],
        &[Check::err(TokenError::NonNativeNotSupported.into())],
    );

    // missing owner
    do_process_instruction(
        unwrap_lamports(&program_id, &account_key, &account2_key, &owner2_key, &[], Some(1000))
            .unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner2_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // account not owned by program
    let not_program_id = Address::new_unique();
    account_account.owner = not_program_id;
    do_process_instruction(
        unwrap_lamports(&program_id, &account_key, &account2_key, &owner_key, &[], Some(0))
            .unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner2_account],
        &[Check::err(ProgramError::IncorrectProgramId)],
    );
    account_account.owner = program_id;

    // unwrap Some(500) lamports
    do_process_instruction(
        unwrap_lamports(&program_id, &account_key, &account2_key, &owner_key, &[], Some(500))
            .unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner_account],
        &[Check::success()],
    );
    // 500 amount balance change...
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.amount, 500);
    // 500 lamports balance change...
    assert_eq!(account_account.lamports(), 500 + account_minimum_balance());
    assert_eq!(account2_account.lamports(), zero_space_rent_exempt_balance + 500);

    // insufficient funds
    do_process_instruction(
        unwrap_lamports(&program_id, &account_key, &account2_key, &owner_key, &[], Some(501))
            .unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner_account],
        &[Check::err(TokenError::InsufficientFunds.into())],
    );

    // unwrap None lamports
    do_process_instruction(
        unwrap_lamports(&program_id, &account_key, &account2_key, &owner_key, &[], None).unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner_account],
        &[Check::success()],
    );
    // 500 amount balance change...
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.amount, 0);
    // 500 lamports balance change...
    assert_eq!(account_account.lamports(), account_minimum_balance());
    assert_eq!(account2_account.lamports(), zero_space_rent_exempt_balance + 500 + 500);
}

#[test]
fn test_self_unwrap_lamports() {
    let program_id = crate::id();
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let mismatch_account_key = Address::new_unique();
    let mut mismatch_account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner2_key = Address::new_unique();
    let mut owner2_account = SolanaAccount::default();
    let mint_key = native_mint::id();
    let mut mint_account = native_mint();
    let mismatch_mint_key = Address::new_unique();
    let mut mismatch_mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // create mismatch mint
    do_process_instruction(
        initialize_mint(&program_id, &mismatch_mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mismatch_mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create account
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create mismatch account
    do_process_instruction(
        initialize_account(&program_id, &mismatch_account_key, &mismatch_mint_key, &owner_key)
            .unwrap(),
        vec![
            &mut mismatch_account_account,
            &mut mismatch_mint_account,
            &mut owner_account,
            &mut rent_sysvar,
        ],
        &[Check::success()],
    );

    let account_info = (&account_key, false, &mut account_account).into_account_info();
    let mismatch_account_info =
        (&mismatch_account_key, false, &mut mismatch_account_account).into_account_info();
    let owner_info = (&owner_key, true, &mut owner_account).into_account_info();
    let owner2_info = (&owner2_key, true, &mut owner2_account).into_account_info();

    // mint to account
    native_mint_to(&account_info, 1000).unwrap();

    // unwrap Some(500) lamports
    let instruction = unwrap_lamports(
        &program_id,
        account_info.key,
        account_info.key,
        owner_info.key,
        &[],
        Some(500),
    )
    .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), account_info.clone(), owner_info.clone()],
        &[Check::success()],
    );
    // 500 amount balance change...
    let account = Account::unpack_unchecked(&account_info.try_borrow_data().unwrap()).unwrap();
    assert_eq!(account.amount, 500);
    // no lamport balance change...
    assert_eq!(account_info.lamports(), 1000 + account_minimum_balance());

    // insufficient funds
    let instruction = unwrap_lamports(
        &program_id,
        account_info.key,
        account_info.key,
        owner_info.key,
        &[],
        Some(501),
    )
    .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), account_info.clone(), owner_info.clone()],
        &[Check::err(TokenError::InsufficientFunds.into())],
    );

    // missing signer
    let mut owner_no_sign_info = owner_info.clone();
    let mut instruction = unwrap_lamports(
        &program_id,
        account_info.key,
        account_info.key,
        owner_no_sign_info.key,
        &[],
        Some(500),
    )
    .unwrap();
    instruction.accounts[2].is_signer = false;
    owner_no_sign_info.is_signer = false;
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), account_info.clone(), owner_no_sign_info.clone()],
        &[Check::err(ProgramError::MissingRequiredSignature)],
    );

    // non native mint
    let instruction = unwrap_lamports(
        &program_id,
        mismatch_account_info.key,
        mismatch_account_info.key,
        owner_info.key,
        &[],
        None,
    )
    .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![mismatch_account_info.clone(), mismatch_account_info.clone(), owner_info.clone()],
        &[Check::err(TokenError::NonNativeNotSupported.into())],
    );

    // missing owner
    let instruction = unwrap_lamports(
        &program_id,
        account_info.key,
        account_info.key,
        owner2_info.key,
        &[],
        Some(500),
    )
    .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), account_info.clone(), owner2_info.clone()],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // unwrap None lamports
    let instruction =
        unwrap_lamports(&program_id, account_info.key, account_info.key, owner_info.key, &[], None)
            .unwrap();
    do_process_instruction_dups(
        instruction,
        vec![account_info.clone(), account_info.clone(), owner_info.clone()],
        &[Check::success()],
    );
    // 500 amount balance change...
    let account = Account::unpack_unchecked(&account_info.try_borrow_data().unwrap()).unwrap();
    assert_eq!(account.amount, 0);
    // no lamport balance change...
    assert_eq!(account_info.lamports(), 1000 + account_minimum_balance());
}

#[test]
fn test_mintable_token_with_zero_supply() {
    let program_id = crate::id();
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // create mint-able token with zero supply
    let decimals = 2;
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, decimals).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );
    let mint = Mint::unpack_unchecked(&mint_account.data).unwrap();
    assert_eq!(
        mint,
        Mint {
            mint_authority: COption::Some(owner_key),
            supply: 0,
            decimals,
            is_initialized: true,
            freeze_authority: COption::None,
        }
    );

    // create account
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // mint to
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &owner_key, &[], 42).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account],
        &[Check::success()],
    );
    let _ = Mint::unpack(&mint_account.data).unwrap();
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.amount, 42);

    // mint to 2, with incorrect decimals
    do_process_instruction(
        mint_to_checked(&program_id, &mint_key, &account_key, &owner_key, &[], 42, decimals + 1)
            .unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account],
        &[Check::err(TokenError::MintDecimalsMismatch.into())],
    );

    let _ = Mint::unpack(&mint_account.data).unwrap();
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.amount, 42);

    // mint to 2
    do_process_instruction(
        mint_to_checked(&program_id, &mint_key, &account_key, &owner_key, &[], 42, decimals)
            .unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account],
        &[Check::success()],
    );
    let _ = Mint::unpack(&mint_account.data).unwrap();
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.amount, 84);
}

#[test]
fn test_approve_dups() {
    let program_id = crate::id();
    let account1_key = Address::new_unique();
    let mut account1_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account1_info: AccountInfo = (&account1_key, true, &mut account1_account).into();
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account2_info: AccountInfo = (&account2_key, false, &mut account2_account).into();
    let account3_key = Address::new_unique();
    let mut account3_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account3_info: AccountInfo = (&account3_key, true, &mut account3_account).into();
    let multisig_key = Address::new_unique();
    let mut multisig_account =
        SolanaAccount::new(multisig_minimum_balance(), Multisig::get_packed_len(), &program_id);
    let multisig_info: AccountInfo = (&multisig_key, true, &mut multisig_account).into();
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner_info: AccountInfo = (&owner_key, true, &mut owner_account).into();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint_info: AccountInfo = (&mint_key, false, &mut mint_account).into();
    let rent_key = rent::id();
    let mut rent_sysvar = rent_sysvar();
    let rent_info: AccountInfo = (&rent_key, false, &mut rent_sysvar).into();

    // create mint
    do_process_instruction_dups(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![mint_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // create account
    do_process_instruction_dups(
        initialize_account(&program_id, &account1_key, &mint_key, &account1_key).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), account1_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // create another account
    do_process_instruction_dups(
        initialize_account(&program_id, &account2_key, &mint_key, &owner_key).unwrap(),
        vec![account2_info.clone(), mint_info.clone(), owner_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // mint to account
    do_process_instruction_dups(
        mint_to(&program_id, &mint_key, &account1_key, &owner_key, &[], 1000).unwrap(),
        vec![mint_info.clone(), account1_info.clone(), owner_info.clone()],
        &[Check::success()],
    );

    // source-owner approve
    do_process_instruction_dups(
        approve(&program_id, &account1_key, &account2_key, &account1_key, &[], 500).unwrap(),
        vec![account1_info.clone(), account2_info.clone(), account1_info.clone()],
        &[Check::success()],
    );

    // source-owner approve_checked
    do_process_instruction_dups(
        approve_checked(
            &program_id,
            &account1_key,
            &mint_key,
            &account2_key,
            &account1_key,
            &[],
            500,
            2,
        )
        .unwrap(),
        vec![
            account1_info.clone(),
            mint_info.clone(),
            account2_info.clone(),
            account1_info.clone(),
        ],
        &[Check::success()],
    );

    // source-owner revoke
    do_process_instruction_dups(
        revoke(&program_id, &account1_key, &account1_key, &[]).unwrap(),
        vec![account1_info.clone(), account1_info.clone()],
        &[Check::success()],
    );

    // test source-multisig signer
    do_process_instruction_dups(
        initialize_multisig(&program_id, &multisig_key, &[&account3_key], 1).unwrap(),
        vec![multisig_info.clone(), rent_info.clone(), account3_info.clone()],
        &[Check::success()],
    );

    do_process_instruction_dups(
        initialize_account(&program_id, &account3_key, &mint_key, &multisig_key).unwrap(),
        vec![account3_info.clone(), mint_info.clone(), multisig_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    do_process_instruction_dups(
        mint_to(&program_id, &mint_key, &account3_key, &owner_key, &[], 1000).unwrap(),
        vec![mint_info.clone(), account3_info.clone(), owner_info.clone()],
        &[Check::success()],
    );

    // source-multisig-signer approve
    do_process_instruction_dups(
        approve(&program_id, &account3_key, &account2_key, &multisig_key, &[&account3_key], 500)
            .unwrap(),
        vec![
            account3_info.clone(),
            account2_info.clone(),
            multisig_info.clone(),
            account3_info.clone(),
        ],
        &[Check::success()],
    );

    // source-multisig-signer approve_checked
    do_process_instruction_dups(
        approve_checked(
            &program_id,
            &account3_key,
            &mint_key,
            &account2_key,
            &multisig_key,
            &[&account3_key],
            500,
            2,
        )
        .unwrap(),
        vec![
            account3_info.clone(),
            mint_info.clone(),
            account2_info.clone(),
            multisig_info.clone(),
            account3_info.clone(),
        ],
        &[Check::success()],
    );

    // source-owner multisig-signer
    do_process_instruction_dups(
        revoke(&program_id, &account3_key, &multisig_key, &[&account3_key]).unwrap(),
        vec![account3_info.clone(), multisig_info.clone(), account3_info.clone()],
        &[Check::success()],
    );

    // approve to source
    do_process_instruction_dups(
        approve_checked(
            &program_id,
            &account2_key,
            &mint_key,
            &account2_key,
            &owner_key,
            &[],
            500,
            2,
        )
        .unwrap(),
        vec![account2_info.clone(), mint_info.clone(), account2_info.clone(), owner_info.clone()],
        &[Check::success()],
    );

    // source-delegate revoke, force account2 to be a signer
    let account2_info: AccountInfo = (&account2_key, true, &mut account2_account).into();
    do_process_instruction_dups(
        revoke(&program_id, &account2_key, &account2_key, &[]).unwrap(),
        vec![account2_info.clone(), account2_info.clone()],
        &[Check::success()],
    );
}

#[test]
fn test_approve() {
    let program_id = crate::id();
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let delegate_key = Address::new_unique();
    let mut delegate_account = SolanaAccount::default();
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner2_key = Address::new_unique();
    let mut owner2_account = SolanaAccount::default();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // create mint
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create account
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create another account
    do_process_instruction(
        initialize_account(&program_id, &account2_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account2_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // mint to account
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &owner_key, &[], 1000).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account],
        &[Check::success()],
    );

    // missing signer
    let mut instruction =
        approve(&program_id, &account_key, &delegate_key, &owner_key, &[], 100).unwrap();
    instruction.accounts[2].is_signer = false;
    do_process_instruction(
        instruction,
        vec![&mut account_account, &mut delegate_account, &mut owner_account],
        &[Check::err(ProgramError::MissingRequiredSignature)],
    );

    // no owner
    do_process_instruction(
        approve(&program_id, &account_key, &delegate_key, &owner2_key, &[], 100).unwrap(),
        vec![&mut account_account, &mut delegate_account, &mut owner2_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // approve delegate
    do_process_instruction(
        approve(&program_id, &account_key, &delegate_key, &owner_key, &[], 100).unwrap(),
        vec![&mut account_account, &mut delegate_account, &mut owner_account],
        &[Check::success()],
    );

    // approve delegate 2, with incorrect decimals
    do_process_instruction(
        approve_checked(
            &program_id,
            &account_key,
            &mint_key,
            &delegate_key,
            &owner_key,
            &[],
            100,
            0, // <-- incorrect decimals
        )
        .unwrap(),
        vec![&mut account_account, &mut mint_account, &mut delegate_account, &mut owner_account],
        &[Check::err(TokenError::MintDecimalsMismatch.into())],
    );

    // approve delegate 2, with incorrect mint
    do_process_instruction(
        approve_checked(
            &program_id,
            &account_key,
            &account2_key, // <-- bad mint
            &delegate_key,
            &owner_key,
            &[],
            100,
            0,
        )
        .unwrap(),
        vec![
            &mut account_account,
            &mut account2_account, // <-- bad mint
            &mut delegate_account,
            &mut owner_account,
        ],
        &[Check::err(TokenError::MintMismatch.into())],
    );

    // approve delegate 2
    do_process_instruction(
        approve_checked(
            &program_id,
            &account_key,
            &mint_key,
            &delegate_key,
            &owner_key,
            &[],
            100,
            2,
        )
        .unwrap(),
        vec![&mut account_account, &mut mint_account, &mut delegate_account, &mut owner_account],
        &[Check::success()],
    );

    // revoke delegate
    do_process_instruction(
        revoke(&program_id, &account_key, &owner_key, &[]).unwrap(),
        vec![&mut account_account, &mut owner_account],
        &[Check::success()],
    );

    // approve delegate 3
    do_process_instruction(
        approve_checked(
            &program_id,
            &account_key,
            &mint_key,
            &delegate_key,
            &owner_key,
            &[],
            100,
            2,
        )
        .unwrap(),
        vec![&mut account_account, &mut mint_account, &mut delegate_account, &mut owner_account],
        &[Check::success()],
    );

    // revoke by delegate
    do_process_instruction(
        revoke(&program_id, &account_key, &delegate_key, &[]).unwrap(),
        vec![&mut account_account, &mut delegate_account],
        &[Check::success()],
    );

    // fails the second time
    do_process_instruction(
        revoke(&program_id, &account_key, &delegate_key, &[]).unwrap(),
        vec![&mut account_account, &mut delegate_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );
}

#[test]
fn test_set_authority_dups() {
    let program_id = crate::id();
    let account1_key = Address::new_unique();
    let mut account1_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account1_info: AccountInfo = (&account1_key, true, &mut account1_account).into();
    let owner_key = Address::new_unique();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint_info: AccountInfo = (&mint_key, true, &mut mint_account).into();
    let rent_key = rent::id();
    let mut rent_sysvar = rent_sysvar();
    let rent_info: AccountInfo = (&rent_key, false, &mut rent_sysvar).into();

    // create mint
    do_process_instruction_dups(
        initialize_mint(&program_id, &mint_key, &mint_key, Some(&mint_key), 2).unwrap(),
        vec![mint_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // create account
    do_process_instruction_dups(
        initialize_account(&program_id, &account1_key, &mint_key, &account1_key).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), account1_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // set mint_authority when currently self
    do_process_instruction_dups(
        set_authority(
            &program_id,
            &mint_key,
            Some(&owner_key),
            AuthorityType::MintTokens,
            &mint_key,
            &[],
        )
        .unwrap(),
        vec![mint_info.clone(), mint_info.clone()],
        &[Check::success()],
    );

    // set freeze_authority when currently self
    do_process_instruction_dups(
        set_authority(
            &program_id,
            &mint_key,
            Some(&owner_key),
            AuthorityType::FreezeAccount,
            &mint_key,
            &[],
        )
        .unwrap(),
        vec![mint_info.clone(), mint_info.clone()],
        &[Check::success()],
    );

    // set account owner when currently self
    do_process_instruction_dups(
        set_authority(
            &program_id,
            &account1_key,
            Some(&owner_key),
            AuthorityType::AccountOwner,
            &account1_key,
            &[],
        )
        .unwrap(),
        vec![account1_info.clone(), account1_info.clone()],
        &[Check::success()],
    );

    // set close_authority when currently self
    let mut account = Account::unpack_unchecked(&account1_info.data.borrow()).unwrap();
    account.close_authority = COption::Some(account1_key);
    Account::pack(account, &mut account1_info.data.borrow_mut()).unwrap();

    do_process_instruction_dups(
        set_authority(
            &program_id,
            &account1_key,
            Some(&owner_key),
            AuthorityType::CloseAccount,
            &account1_key,
            &[],
        )
        .unwrap(),
        vec![account1_info.clone(), account1_info.clone()],
        &[Check::success()],
    );
}

#[test]
fn test_set_authority() {
    let program_id = crate::id();
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner2_key = Address::new_unique();
    let mut owner2_account = SolanaAccount::default();
    let owner3_key = Address::new_unique();
    let mut owner3_account = SolanaAccount::default();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint2_key = Address::new_unique();
    let mut mint2_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // create new mint with owner
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create mint with owner and freeze_authority
    do_process_instruction(
        initialize_mint(&program_id, &mint2_key, &owner_key, Some(&owner_key), 2).unwrap(),
        vec![&mut mint2_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // invalid account
    do_process_instruction(
        set_authority(
            &program_id,
            &account_key,
            Some(&owner2_key),
            AuthorityType::AccountOwner,
            &owner_key,
            &[],
        )
        .unwrap(),
        vec![&mut account_account, &mut owner_account],
        &[Check::err(ProgramError::InvalidAccountData)],
    );

    // create account
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create another account
    do_process_instruction(
        initialize_account(&program_id, &account2_key, &mint2_key, &owner_key).unwrap(),
        vec![&mut account2_account, &mut mint2_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // missing owner
    do_process_instruction(
        set_authority(
            &program_id,
            &account_key,
            Some(&owner_key),
            AuthorityType::AccountOwner,
            &owner2_key,
            &[],
        )
        .unwrap(),
        vec![&mut account_account, &mut owner2_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // owner did not sign
    let mut instruction = set_authority(
        &program_id,
        &account_key,
        Some(&owner2_key),
        AuthorityType::AccountOwner,
        &owner_key,
        &[],
    )
    .unwrap();
    instruction.accounts[1].is_signer = false;
    do_process_instruction(
        instruction,
        vec![&mut account_account, &mut owner_account],
        &[Check::err(ProgramError::MissingRequiredSignature)],
    );

    // wrong authority type
    do_process_instruction(
        set_authority(
            &program_id,
            &account_key,
            Some(&owner2_key),
            AuthorityType::FreezeAccount,
            &owner_key,
            &[],
        )
        .unwrap(),
        vec![&mut account_account, &mut owner_account],
        &[Check::err(TokenError::AuthorityTypeNotSupported.into())],
    );

    // account owner may not be set to None
    do_process_instruction(
        set_authority(
            &program_id,
            &account_key,
            None,
            AuthorityType::AccountOwner,
            &owner_key,
            &[],
        )
        .unwrap(),
        vec![&mut account_account, &mut owner_account],
        &[Check::err(TokenError::InvalidInstruction.into())],
    );

    // set delegate
    do_process_instruction(
        approve(&program_id, &account_key, &owner2_key, &owner_key, &[], u64::MAX).unwrap(),
        vec![&mut account_account, &mut owner2_account, &mut owner_account],
        &[Check::success()],
    );
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.delegate, COption::Some(owner2_key));
    assert_eq!(account.delegated_amount, u64::MAX);

    // set owner
    do_process_instruction(
        set_authority(
            &program_id,
            &account_key,
            Some(&owner3_key),
            AuthorityType::AccountOwner,
            &owner_key,
            &[],
        )
        .unwrap(),
        vec![&mut account_account, &mut owner_account],
        &[Check::success()],
    );

    // check delegate cleared
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.delegate, COption::None);
    assert_eq!(account.delegated_amount, 0);

    // set owner without existing delegate
    do_process_instruction(
        set_authority(
            &program_id,
            &account_key,
            Some(&owner2_key),
            AuthorityType::AccountOwner,
            &owner3_key,
            &[],
        )
        .unwrap(),
        vec![&mut account_account, &mut owner3_account],
        &[Check::success()],
    );

    // set close_authority
    do_process_instruction(
        set_authority(
            &program_id,
            &account_key,
            Some(&owner2_key),
            AuthorityType::CloseAccount,
            &owner2_key,
            &[],
        )
        .unwrap(),
        vec![&mut account_account, &mut owner2_account],
        &[Check::success()],
    );

    // close_authority may be set to None
    do_process_instruction(
        set_authority(
            &program_id,
            &account_key,
            None,
            AuthorityType::CloseAccount,
            &owner2_key,
            &[],
        )
        .unwrap(),
        vec![&mut account_account, &mut owner2_account],
        &[Check::success()],
    );

    // wrong owner
    do_process_instruction(
        set_authority(
            &program_id,
            &mint_key,
            Some(&owner3_key),
            AuthorityType::MintTokens,
            &owner2_key,
            &[],
        )
        .unwrap(),
        vec![&mut mint_account, &mut owner2_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // owner did not sign
    let mut instruction = set_authority(
        &program_id,
        &mint_key,
        Some(&owner2_key),
        AuthorityType::MintTokens,
        &owner_key,
        &[],
    )
    .unwrap();
    instruction.accounts[1].is_signer = false;
    do_process_instruction(
        instruction,
        vec![&mut mint_account, &mut owner_account],
        &[Check::err(ProgramError::MissingRequiredSignature)],
    );

    // cannot freeze
    do_process_instruction(
        set_authority(
            &program_id,
            &mint_key,
            Some(&owner2_key),
            AuthorityType::FreezeAccount,
            &owner_key,
            &[],
        )
        .unwrap(),
        vec![&mut mint_account, &mut owner_account],
        &[Check::err(TokenError::MintCannotFreeze.into())],
    );

    // set owner
    do_process_instruction(
        set_authority(
            &program_id,
            &mint_key,
            Some(&owner2_key),
            AuthorityType::MintTokens,
            &owner_key,
            &[],
        )
        .unwrap(),
        vec![&mut mint_account, &mut owner_account],
        &[Check::success()],
    );

    // set owner to None
    do_process_instruction(
        set_authority(&program_id, &mint_key, None, AuthorityType::MintTokens, &owner2_key, &[])
            .unwrap(),
        vec![&mut mint_account, &mut owner2_account],
        &[Check::success()],
    );

    // test unsetting mint_authority is one-way operation
    do_process_instruction(
        set_authority(
            &program_id,
            &mint2_key,
            Some(&owner2_key),
            AuthorityType::MintTokens,
            &owner_key,
            &[],
        )
        .unwrap(),
        vec![&mut mint_account, &mut owner_account],
        &[Check::err(TokenError::FixedSupply.into())],
    );

    // set freeze_authority
    do_process_instruction(
        set_authority(
            &program_id,
            &mint2_key,
            Some(&owner2_key),
            AuthorityType::FreezeAccount,
            &owner_key,
            &[],
        )
        .unwrap(),
        vec![&mut mint2_account, &mut owner_account],
        &[Check::success()],
    );

    // test unsetting freeze_authority is one-way operation
    do_process_instruction(
        set_authority(
            &program_id,
            &mint2_key,
            None,
            AuthorityType::FreezeAccount,
            &owner2_key,
            &[],
        )
        .unwrap(),
        vec![&mut mint2_account, &mut owner2_account],
        &[Check::success()],
    );

    do_process_instruction(
        set_authority(
            &program_id,
            &mint2_key,
            Some(&owner2_key),
            AuthorityType::FreezeAccount,
            &owner_key,
            &[],
        )
        .unwrap(),
        vec![&mut mint2_account, &mut owner2_account],
        &[Check::err(TokenError::MintCannotFreeze.into())],
    );
}

#[test]
fn test_set_authority_with_immutable_owner_extension() {
    let program_id = crate::id();
    let account_key = Address::new_unique();

    let account_len =
        ExtensionType::try_calculate_account_len::<Account>(&[ExtensionType::ImmutableOwner])
            .unwrap();
    let mut account_account =
        SolanaAccount::new(Rent::default().minimum_balance(account_len), account_len, &program_id);
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner2_key = Address::new_unique();

    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // create mint
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create account
    do_process_instruction(
        initialize_immutable_owner(&program_id, &account_key).unwrap(),
        vec![&mut account_account],
        &[Check::success()],
    );
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // Immutable Owner extension blocks account owner authority changes
    do_process_instruction(
        set_authority(
            &program_id,
            &account_key,
            Some(&owner2_key),
            AuthorityType::AccountOwner,
            &owner_key,
            &[],
        )
        .unwrap(),
        vec![&mut account_account, &mut owner_account],
        &[Check::err(TokenError::ImmutableOwner.into())],
    );
}

#[test]
fn test_mint_to_dups() {
    let program_id = crate::id();
    let account1_key = Address::new_unique();
    let mut account1_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account1_info: AccountInfo = (&account1_key, true, &mut account1_account).into();
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner_info: AccountInfo = (&owner_key, true, &mut owner_account).into();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint_info: AccountInfo = (&mint_key, true, &mut mint_account).into();
    let rent_key = rent::id();
    let mut rent_sysvar = rent_sysvar();
    let rent_info: AccountInfo = (&rent_key, false, &mut rent_sysvar).into();

    // create mint
    do_process_instruction_dups(
        initialize_mint(&program_id, &mint_key, &mint_key, None, 2).unwrap(),
        vec![mint_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // create account
    do_process_instruction_dups(
        initialize_account(&program_id, &account1_key, &mint_key, &owner_key).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), owner_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // mint_to when mint_authority is self
    do_process_instruction_dups(
        mint_to(&program_id, &mint_key, &account1_key, &mint_key, &[], 42).unwrap(),
        vec![mint_info.clone(), account1_info.clone(), mint_info.clone()],
        &[Check::success()],
    );

    // mint_to_checked when mint_authority is self
    do_process_instruction_dups(
        mint_to_checked(&program_id, &mint_key, &account1_key, &mint_key, &[], 42, 2).unwrap(),
        vec![mint_info.clone(), account1_info.clone(), mint_info.clone()],
        &[Check::success()],
    );

    // mint_to when mint_authority is account owner
    let mut mint = Mint::unpack_unchecked(&mint_info.data.borrow()).unwrap();
    mint.mint_authority = COption::Some(account1_key);
    Mint::pack(mint, &mut mint_info.data.borrow_mut()).unwrap();
    do_process_instruction_dups(
        mint_to(&program_id, &mint_key, &account1_key, &account1_key, &[], 42).unwrap(),
        vec![mint_info.clone(), account1_info.clone(), account1_info.clone()],
        &[Check::success()],
    );

    // mint_to_checked when mint_authority is account owner
    do_process_instruction_dups(
        mint_to(&program_id, &mint_key, &account1_key, &account1_key, &[], 42).unwrap(),
        vec![mint_info.clone(), account1_info.clone(), account1_info.clone()],
        &[Check::success()],
    );
}

#[test]
fn test_mint_to() {
    let program_id = crate::id();
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account3_key = Address::new_unique();
    let mut account3_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let mismatch_key = Address::new_unique();
    let mut mismatch_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner2_key = Address::new_unique();
    let mut owner2_account = SolanaAccount::default();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint2_key = Address::new_unique();
    let uninitialized_key = Address::new_unique();
    let mut uninitialized_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // create new mint with owner
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create account
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create another account
    do_process_instruction(
        initialize_account(&program_id, &account2_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account2_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create another account
    do_process_instruction(
        initialize_account(&program_id, &account3_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account3_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create mismatch account
    do_process_instruction(
        initialize_account(&program_id, &mismatch_key, &mint_key, &owner_key).unwrap(),
        vec![&mut mismatch_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );
    let mut account = Account::unpack_unchecked(&mismatch_account.data).unwrap();
    account.mint = mint2_key;
    Account::pack(account, &mut mismatch_account.data).unwrap();

    // mint to
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &owner_key, &[], 42).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account],
        &[Check::success()],
    );

    let mint = Mint::unpack_unchecked(&mint_account.data).unwrap();
    assert_eq!(mint.supply, 42);
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.amount, 42);

    // mint to another account to test supply accumulation
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account2_key, &owner_key, &[], 42).unwrap(),
        vec![&mut mint_account, &mut account2_account, &mut owner_account],
        &[Check::success()],
    );

    let mint = Mint::unpack_unchecked(&mint_account.data).unwrap();
    assert_eq!(mint.supply, 84);
    let account = Account::unpack_unchecked(&account2_account.data).unwrap();
    assert_eq!(account.amount, 42);

    // missing signer
    let mut instruction =
        mint_to(&program_id, &mint_key, &account2_key, &owner_key, &[], 42).unwrap();
    instruction.accounts[2].is_signer = false;
    do_process_instruction(
        instruction,
        vec![&mut mint_account, &mut account2_account, &mut owner_account],
        &[Check::err(ProgramError::MissingRequiredSignature)],
    );

    // mismatch account
    do_process_instruction(
        mint_to(&program_id, &mint_key, &mismatch_key, &owner_key, &[], 42).unwrap(),
        vec![&mut mint_account, &mut mismatch_account, &mut owner_account],
        &[Check::err(TokenError::MintMismatch.into())],
    );

    // missing owner
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account2_key, &owner2_key, &[], 42).unwrap(),
        vec![&mut mint_account, &mut account2_account, &mut owner2_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // mint not owned by program
    let not_program_id = Address::new_unique();
    mint_account.owner = not_program_id;
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &owner_key, &[], 0).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account],
        &[Check::err(ProgramError::IncorrectProgramId)],
    );
    mint_account.owner = program_id;

    // account not owned by program
    let not_program_id = Address::new_unique();
    account_account.owner = not_program_id;
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &owner_key, &[], 0).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account],
        &[Check::err(ProgramError::IncorrectProgramId)],
    );
    account_account.owner = program_id;

    // uninitialized destination account
    do_process_instruction(
        mint_to(&program_id, &mint_key, &uninitialized_key, &owner_key, &[], 42).unwrap(),
        vec![&mut mint_account, &mut uninitialized_account, &mut owner_account],
        &[Check::err(ProgramError::UninitializedAccount)],
    );

    // unset mint_authority and test minting fails
    do_process_instruction(
        set_authority(&program_id, &mint_key, None, AuthorityType::MintTokens, &owner_key, &[])
            .unwrap(),
        vec![&mut mint_account, &mut owner_account],
        &[Check::success()],
    );
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account2_key, &owner_key, &[], 42).unwrap(),
        vec![&mut mint_account, &mut account2_account, &mut owner_account],
        &[Check::err(TokenError::FixedSupply.into())],
    );
}

#[test]
fn test_batch_with_invalid_count() {
    let program_id = crate::id();
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let mint_account_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint_owner = Address::new_unique();
    let account_owner = Address::new_unique();
    let mut account_owner_account = SolanaAccount::default();
    let rent_key = rent::id();
    let mut rent_sysvar = rent_sysvar();
    let account_info: AccountInfo = (&account_key, true, &mut account_account).into();
    let mint_account_info: AccountInfo = (&mint_account_key, true, &mut mint_account).into();
    let account_owner_info: AccountInfo = (&account_owner, true, &mut account_owner_account).into();
    let rent_info: AccountInfo = (&rent_key, false, &mut rent_sysvar).into();

    let initialize_mint_instruction =
        initialize_mint(&program_id, &mint_account_key, &mint_owner, None, 9).unwrap();

    let initialize_account_instruction =
        initialize_account3(&program_id, &account_key, &mint_account_key, &account_owner).unwrap();

    let close_account_instruction =
        close_account(&program_id, &account_key, &account_owner, &account_owner, &[]).unwrap();

    let mut batch_instr = batch_instruction(&[
        &initialize_mint_instruction,
        &initialize_account_instruction,
        &close_account_instruction,
    ])
    .unwrap();

    // clear the close discriminator
    batch_instr.data.pop();

    let temp_mint_data = mint_account_info.data.borrow().to_vec();
    let temp_account_data = account_info.data.borrow().to_vec();

    do_process_instruction_dups(
        batch_instr,
        vec![
            mint_account_info.clone(),
            rent_info.clone(),
            account_info.clone(),
            mint_account_info.clone(),
            account_info.clone(),
            account_owner_info.clone(),
            account_owner_info.clone(),
        ],
        &[Check::err(TokenError::InvalidInstruction.into())],
    );

    // reset the accounts
    mint_account_info.data.borrow_mut().copy_from_slice(&temp_mint_data);
    account_info.data.borrow_mut().copy_from_slice(&temp_account_data);

    let mut batch_instr = batch_instruction(&[
        &initialize_mint_instruction,
        &initialize_account_instruction,
        &close_account_instruction,
    ])
    .unwrap();

    // Omit account metas as well so the program sees the truncated account list.
    batch_instr.accounts.truncate(3);

    do_process_instruction_dups(
        batch_instr,
        vec![mint_account_info.clone(), rent_info.clone(), account_info.clone()], // leave out the `initialize_account3` accounts and `close_account` accounts
        &[Check::err(ProgramError::NotEnoughAccountKeys)],
    );

    // reset the accounts
    mint_account_info.data.borrow_mut().copy_from_slice(&temp_mint_data);
    account_info.data.borrow_mut().copy_from_slice(&temp_account_data);

    let batch_instruction = batch_instruction(&[
        &initialize_mint_instruction,
        &initialize_account_instruction,
        &close_account_instruction,
    ])
    .unwrap();

    do_process_instruction_dups(
        batch_instruction,
        vec![
            mint_account_info.clone(),
            rent_info.clone(),
            account_info.clone(),
            mint_account_info.clone(),
            account_info.clone(),
            account_owner_info.clone(),
            account_owner_info.clone(),
        ],
        &[Check::success()],
    );
}

#[test]
fn test_batch_in_batch() {
    let program_id = crate::id();
    let mint_account_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint_owner = Address::new_unique();
    let mut mint_owner_account = SolanaAccount::default();
    let rent_key = rent::id();
    let mut rent_sysvar = rent_sysvar();

    let mint_account_info: AccountInfo = (&mint_account_key, true, &mut mint_account).into();
    let mint_owner_info: AccountInfo = (&mint_owner, true, &mut mint_owner_account).into();
    let rent_info: AccountInfo = (&rent_key, false, &mut rent_sysvar).into();

    let initialize_mint_instruction =
        initialize_mint(&program_id, &mint_account_key, &mint_owner, Some(&mint_owner), 9).unwrap();

    let set_authority_instruction = set_authority(
        &program_id,
        &mint_account_key,
        None,
        AuthorityType::FreezeAccount,
        &mint_owner,
        &[],
    )
    .unwrap();

    let batch_instr = batch_instruction(&[&set_authority_instruction]).unwrap();

    let batch_in_batch_instruction =
        batch_instruction(&[&batch_instr, &initialize_mint_instruction]).unwrap();

    let temp_mint_account_data = mint_account_info.data.borrow().to_vec();

    do_process_instruction_dups(
        batch_in_batch_instruction,
        vec![
            mint_account_info.clone(),
            rent_info.clone(),
            mint_account_info.clone(),
            mint_owner_info.clone(),
        ],
        &[Check::err(TokenError::InvalidInstruction.into())],
    );

    // reset account
    mint_account_info.data.borrow_mut().copy_from_slice(&temp_mint_account_data);

    do_process_instruction_dups(
        initialize_mint_instruction,
        vec![mint_account_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    do_process_instruction_dups(
        batch_instr,
        vec![mint_account_info.clone(), mint_owner_info.clone()],
        &[Check::success()],
    );
}

#[test]
#[allow(deprecated)]
fn test_batch_initialize_mint_swap() {
    let program_id = crate::id();
    let account1_key = Address::new_unique();
    let mut account1_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account3_key = Address::new_unique();
    let mut account3_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account4_key = Address::new_unique();
    let mut account4_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let mint_account1_key = Address::new_unique();
    let mut mint1_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint_account2_key = Address::new_unique();
    let mut mint2_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &Address::new_unique());
    let mint_owner = Address::new_unique();
    let mut mint_owner_account = SolanaAccount::default();
    let account_owner = Address::new_unique();
    let mut account_owner_account = SolanaAccount::default();
    let rent_key = rent::id();
    let mut rent_sysvar = rent_sysvar();

    let account1_info: AccountInfo = (&account1_key, true, &mut account1_account).into();
    let account2_info: AccountInfo = (&account2_key, true, &mut account2_account).into();
    let account3_info: AccountInfo = (&account3_key, true, &mut account3_account).into();
    let account4_info: AccountInfo = (&account4_key, true, &mut account4_account).into();
    let mint_account1_info: AccountInfo = (&mint_account1_key, true, &mut mint1_account).into();
    let mint_account2_info: AccountInfo = (&mint_account2_key, true, &mut mint2_account).into();
    let mint_owner_info: AccountInfo = (&mint_owner, true, &mut mint_owner_account).into();
    let account_owner_info: AccountInfo = (&account_owner, true, &mut account_owner_account).into();
    let rent_info: AccountInfo = (&rent_key, false, &mut rent_sysvar).into();

    let initialize_mint_1_instruction =
        initialize_mint(&program_id, &mint_account1_key, &mint_owner, None, 9).unwrap();
    let initialize_mint_2_instruction =
        initialize_mint(&program_id, &mint_account2_key, &mint_owner, None, 9).unwrap();

    let initialize_account_1_instruction =
        initialize_account3(&program_id, &account1_key, &mint_account1_key, &account_owner)
            .unwrap();
    let initialize_account_2_instruction =
        initialize_account3(&program_id, &account2_key, &mint_account2_key, &account_owner)
            .unwrap();

    let initialize_account_3_instruction =
        initialize_account3(&program_id, &account3_key, &mint_account1_key, &account_owner)
            .unwrap();
    let initialize_account_4_instruction =
        initialize_account3(&program_id, &account4_key, &mint_account2_key, &account_owner)
            .unwrap();

    let mint_to_1_instruction =
        mint_to(&program_id, &mint_account1_key, &account1_key, &mint_owner, &[], 1000).unwrap();
    let mint_to_2_instruction =
        mint_to_checked(&program_id, &mint_account2_key, &account2_key, &mint_owner, &[], 1000, 9)
            .unwrap();
    let mint_to_2_with_invalid_decimals =
        mint_to_checked(&program_id, &mint_account2_key, &account2_key, &mint_owner, &[], 1000, 8)
            .unwrap();

    let transfer_to_3_instruction =
        transfer(&program_id, &account1_key, &account3_key, &account_owner, &[], 500).unwrap();
    let transfer_to_3_with_invalid_amount =
        transfer(&program_id, &account1_key, &account3_key, &account_owner, &[], 501).unwrap();
    let transfer_4_instruction = transfer_checked(
        &program_id,
        &account2_key,
        &mint_account2_key,
        &account4_key,
        &account_owner,
        &[],
        500,
        9,
    )
    .unwrap();

    let batch_1_instruction = batch_instruction(&[&initialize_mint_1_instruction]).unwrap();

    do_process_instruction_dups(
        batch_1_instruction,
        vec![mint_account1_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    let temp_mint1_data = mint_account1_info.data.borrow().to_vec();
    let temp_account1_data = account1_info.data.borrow().to_vec();
    let temp_account2_data = account2_info.data.borrow().to_vec();

    // fails with incorrect program id(caused by `initialize_account_2_instr`)
    let batch_2_fail_instruction = batch_instruction(&[
        &initialize_mint_2_instruction,
        &initialize_account_1_instruction,
        &initialize_account_2_instruction,
        &initialize_account_3_instruction,
        &initialize_account_4_instruction,
    ])
    .unwrap();

    do_process_instruction_dups(
        batch_2_fail_instruction,
        vec![
            mint_account2_info.clone(),
            rent_info.clone(),
            account1_info.clone(),
            mint_account1_info.clone(),
            account2_info.clone(),
            mint_account2_info.clone(),
            account3_info.clone(),
            mint_account1_info.clone(),
            account4_info.clone(),
            mint_account2_info.clone(),
        ],
        &[Check::err(ProgramError::IncorrectProgramId)],
    );

    // reset accounts
    mint_account1_info.data.borrow_mut().copy_from_slice(&temp_mint1_data);
    account1_info.data.borrow_mut().copy_from_slice(&temp_account1_data);
    account2_info.data.borrow_mut().copy_from_slice(&temp_account2_data);

    // correct the owner
    let mut mint2_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint_account2_info: AccountInfo = (&mint_account2_key, true, &mut mint2_account).into();

    // with correct owner
    let batch_2_success_instruction = batch_instruction(&[
        &initialize_mint_2_instruction,
        &initialize_account_1_instruction,
        &initialize_account_2_instruction,
        &initialize_account_3_instruction,
        &initialize_account_4_instruction,
    ])
    .unwrap();

    do_process_instruction_dups(
        batch_2_success_instruction,
        vec![
            mint_account2_info.clone(),
            rent_info.clone(),
            account1_info.clone(),
            mint_account1_info.clone(),
            account2_info.clone(),
            mint_account2_info.clone(),
            account3_info.clone(),
            mint_account1_info.clone(),
            account4_info.clone(),
            mint_account2_info.clone(),
        ],
        &[Check::success()],
    );

    let temp_mint1_data = mint_account1_info.data.borrow().to_vec();
    let temp_account1_data = account1_info.data.borrow().to_vec();
    let temp_mint2_data = mint_account2_info.data.borrow().to_vec();
    let temp_account2_data = account2_info.data.borrow().to_vec();

    // with decimals mismatch
    let batch_3_fail_instruction = batch_instruction(&[
        &mint_to_1_instruction,
        &mint_to_2_instruction,
        &mint_to_2_with_invalid_decimals,
    ])
    .unwrap();

    do_process_instruction_dups(
        batch_3_fail_instruction,
        vec![
            mint_account1_info.clone(),
            account1_info.clone(),
            mint_owner_info.clone(),
            mint_account2_info.clone(),
            account2_info.clone(),
            mint_owner_info.clone(),
            mint_account2_info.clone(),
            account2_info.clone(),
            mint_owner_info.clone(),
        ],
        &[Check::err(TokenError::MintDecimalsMismatch.into())],
    );

    // reset accounts
    mint_account1_info.data.borrow_mut().copy_from_slice(&temp_mint1_data);
    account1_info.data.borrow_mut().copy_from_slice(&temp_account1_data);
    mint_account2_info.data.borrow_mut().copy_from_slice(&temp_mint2_data);
    account2_info.data.borrow_mut().copy_from_slice(&temp_account2_data);

    // without the invalid decimals instruction
    let batch_3_success_instruction =
        batch_instruction(&[&mint_to_1_instruction, &mint_to_2_instruction]).unwrap();

    do_process_instruction_dups(
        batch_3_success_instruction,
        vec![
            mint_account1_info.clone(),
            account1_info.clone(),
            mint_owner_info.clone(),
            mint_account2_info.clone(),
            account2_info.clone(),
            mint_owner_info.clone(),
        ],
        &[Check::success()],
    );

    let temp_account1_data = account1_info.data.borrow().to_vec();
    let temp_account3_data = account3_info.data.borrow().to_vec();
    let temp_account2_data = account2_info.data.borrow().to_vec();
    let temp_account4_data = account4_info.data.borrow().to_vec();

    let batch_4_fail_instruction = batch_instruction(&[
        &transfer_to_3_instruction,
        &transfer_to_3_with_invalid_amount,
        &transfer_4_instruction,
    ])
    .unwrap();

    do_process_instruction_dups(
        batch_4_fail_instruction,
        vec![
            account1_info.clone(),
            account3_info.clone(),
            account_owner_info.clone(),
            account1_info.clone(),
            account3_info.clone(),
            account_owner_info.clone(),
            account2_info.clone(),
            mint_account2_info.clone(),
            account4_info.clone(),
            account_owner_info.clone(),
        ],
        &[Check::err(TokenError::InsufficientFunds.into())],
    );

    // reset accounts
    account1_info.data.borrow_mut().copy_from_slice(&temp_account1_data);
    account3_info.data.borrow_mut().copy_from_slice(&temp_account3_data);
    account2_info.data.borrow_mut().copy_from_slice(&temp_account2_data);
    account4_info.data.borrow_mut().copy_from_slice(&temp_account4_data);

    // with valid amount
    let batch_4_success_instruction =
        batch_instruction(&[&transfer_to_3_instruction, &transfer_4_instruction]).unwrap();

    do_process_instruction_dups(
        batch_4_success_instruction,
        vec![
            account1_info.clone(),
            account3_info.clone(),
            account_owner_info.clone(),
            account2_info.clone(),
            mint_account2_info.clone(),
            account4_info.clone(),
            account_owner_info.clone(),
        ],
        &[Check::success()],
    );
}

#[test]
fn test_burn_dups() {
    let program_id = crate::id();
    let account1_key = Address::new_unique();
    let mut account1_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account1_info: AccountInfo = (&account1_key, true, &mut account1_account).into();
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner_info: AccountInfo = (&owner_key, true, &mut owner_account).into();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint_info: AccountInfo = (&mint_key, true, &mut mint_account).into();
    let rent_key = rent::id();
    let mut rent_sysvar = rent_sysvar();
    let rent_info: AccountInfo = (&rent_key, false, &mut rent_sysvar).into();

    // create mint
    do_process_instruction_dups(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![mint_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // create account
    do_process_instruction_dups(
        initialize_account(&program_id, &account1_key, &mint_key, &account1_key).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), account1_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // mint to account
    do_process_instruction_dups(
        mint_to(&program_id, &mint_key, &account1_key, &owner_key, &[], 1000).unwrap(),
        vec![mint_info.clone(), account1_info.clone(), owner_info.clone()],
        &[Check::success()],
    );

    // source-owner burn
    do_process_instruction_dups(
        burn(&program_id, &account1_key, &mint_key, &account1_key, &[], 500).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), account1_info.clone()],
        &[Check::success()],
    );

    // source-owner burn_checked
    do_process_instruction_dups(
        burn_checked(&program_id, &account1_key, &mint_key, &account1_key, &[], 500, 2).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), account1_info.clone()],
        &[Check::success()],
    );

    // mint-owner burn
    do_process_instruction_dups(
        mint_to(&program_id, &mint_key, &account1_key, &owner_key, &[], 1000).unwrap(),
        vec![mint_info.clone(), account1_info.clone(), owner_info.clone()],
        &[Check::success()],
    );
    let mut account = Account::unpack_unchecked(&account1_info.data.borrow()).unwrap();
    account.owner = mint_key;
    Account::pack(account, &mut account1_info.data.borrow_mut()).unwrap();
    do_process_instruction_dups(
        burn(&program_id, &account1_key, &mint_key, &mint_key, &[], 500).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), mint_info.clone()],
        &[Check::success()],
    );

    // mint-owner burn_checked
    do_process_instruction_dups(
        burn_checked(&program_id, &account1_key, &mint_key, &mint_key, &[], 500, 2).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), mint_info.clone()],
        &[Check::success()],
    );

    // source-delegate burn
    do_process_instruction_dups(
        mint_to(&program_id, &mint_key, &account1_key, &owner_key, &[], 1000).unwrap(),
        vec![mint_info.clone(), account1_info.clone(), owner_info.clone()],
        &[Check::success()],
    );
    let mut account = Account::unpack_unchecked(&account1_info.data.borrow()).unwrap();
    account.delegated_amount = 1000;
    account.delegate = COption::Some(account1_key);
    account.owner = owner_key;
    Account::pack(account, &mut account1_info.data.borrow_mut()).unwrap();
    do_process_instruction_dups(
        burn(&program_id, &account1_key, &mint_key, &account1_key, &[], 500).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), account1_info.clone()],
        &[Check::success()],
    );

    // source-delegate burn_checked
    do_process_instruction_dups(
        burn_checked(&program_id, &account1_key, &mint_key, &account1_key, &[], 500, 2).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), account1_info.clone()],
        &[Check::success()],
    );

    // mint-delegate burn
    do_process_instruction_dups(
        mint_to(&program_id, &mint_key, &account1_key, &owner_key, &[], 1000).unwrap(),
        vec![mint_info.clone(), account1_info.clone(), owner_info.clone()],
        &[Check::success()],
    );
    let mut account = Account::unpack_unchecked(&account1_info.data.borrow()).unwrap();
    account.delegated_amount = 1000;
    account.delegate = COption::Some(mint_key);
    account.owner = owner_key;
    Account::pack(account, &mut account1_info.data.borrow_mut()).unwrap();
    do_process_instruction_dups(
        burn(&program_id, &account1_key, &mint_key, &mint_key, &[], 500).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), mint_info.clone()],
        &[Check::success()],
    );

    // mint-delegate burn_checked
    do_process_instruction_dups(
        burn_checked(&program_id, &account1_key, &mint_key, &mint_key, &[], 500, 2).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), mint_info.clone()],
        &[Check::success()],
    );
}

#[test]
fn test_burn() {
    let program_id = crate::id();
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account3_key = Address::new_unique();
    let mut account3_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let delegate_key = Address::new_unique();
    let mut delegate_account = SolanaAccount::default();
    let mismatch_key = Address::new_unique();
    let mut mismatch_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner2_key = Address::new_unique();
    let mut owner2_account = SolanaAccount::default();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint2_key = Address::new_unique();
    let mut rent_sysvar = rent_sysvar();

    // create new mint
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create account
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create another account
    do_process_instruction(
        initialize_account(&program_id, &account2_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account2_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create another account
    do_process_instruction(
        initialize_account(&program_id, &account3_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account3_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create mismatch account
    do_process_instruction(
        initialize_account(&program_id, &mismatch_key, &mint_key, &owner_key).unwrap(),
        vec![&mut mismatch_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // mint to account
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &owner_key, &[], 1000).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account],
        &[Check::success()],
    );

    // mint to mismatch account and change mint key
    do_process_instruction(
        mint_to(&program_id, &mint_key, &mismatch_key, &owner_key, &[], 1000).unwrap(),
        vec![&mut mint_account, &mut mismatch_account, &mut owner_account],
        &[Check::success()],
    );
    let mut account = Account::unpack_unchecked(&mismatch_account.data).unwrap();
    account.mint = mint2_key;
    Account::pack(account, &mut mismatch_account.data).unwrap();

    // missing signer
    let mut instruction =
        burn(&program_id, &account_key, &mint_key, &delegate_key, &[], 42).unwrap();
    instruction.accounts[1].is_signer = false;
    do_process_instruction(
        instruction,
        vec![&mut account_account, &mut mint_account, &mut delegate_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // missing owner
    do_process_instruction(
        burn(&program_id, &account_key, &mint_key, &owner2_key, &[], 42).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner2_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // account not owned by program
    let not_program_id = Address::new_unique();
    account_account.owner = not_program_id;
    do_process_instruction(
        burn(&program_id, &account_key, &mint_key, &owner_key, &[], 0).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account],
        &[Check::err(ProgramError::IncorrectProgramId)],
    );
    account_account.owner = program_id;

    // mint not owned by program
    let not_program_id = Address::new_unique();
    mint_account.owner = not_program_id;
    do_process_instruction(
        burn(&program_id, &account_key, &mint_key, &owner_key, &[], 0).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account],
        &[Check::err(ProgramError::IncorrectProgramId)],
    );
    mint_account.owner = program_id;

    // mint mismatch
    do_process_instruction(
        burn(&program_id, &mismatch_key, &mint_key, &owner_key, &[], 42).unwrap(),
        vec![&mut mismatch_account, &mut mint_account, &mut owner_account],
        &[Check::err(TokenError::MintMismatch.into())],
    );

    // burn
    do_process_instruction(
        burn(&program_id, &account_key, &mint_key, &owner_key, &[], 21).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account],
        &[Check::success()],
    );

    // burn_checked, with incorrect decimals
    do_process_instruction(
        burn_checked(&program_id, &account_key, &mint_key, &owner_key, &[], 21, 3).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account],
        &[Check::err(TokenError::MintDecimalsMismatch.into())],
    );

    // burn_checked
    do_process_instruction(
        burn_checked(&program_id, &account_key, &mint_key, &owner_key, &[], 21, 2).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account],
        &[Check::success()],
    );

    let mint = Mint::unpack_unchecked(&mint_account.data).unwrap();
    assert_eq!(mint.supply, 2000 - 42);
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.amount, 1000 - 42);

    // insufficient funds
    do_process_instruction(
        burn(&program_id, &account_key, &mint_key, &owner_key, &[], 100_000_000).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account],
        &[Check::err(TokenError::InsufficientFunds.into())],
    );

    // approve delegate
    do_process_instruction(
        approve(&program_id, &account_key, &delegate_key, &owner_key, &[], 84).unwrap(),
        vec![&mut account_account, &mut delegate_account, &mut owner_account],
        &[Check::success()],
    );

    // not a delegate of source account
    do_process_instruction(
        burn(
            &program_id,
            &account_key,
            &mint_key,
            &owner2_key, // <-- incorrect owner or delegate
            &[],
            1,
        )
        .unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner2_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // insufficient funds approved via delegate
    do_process_instruction(
        burn(&program_id, &account_key, &mint_key, &delegate_key, &[], 85).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut delegate_account],
        &[Check::err(TokenError::InsufficientFunds.into())],
    );

    // burn via delegate
    do_process_instruction(
        burn(&program_id, &account_key, &mint_key, &delegate_key, &[], 84).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut delegate_account],
        &[Check::success()],
    );

    // match
    let mint = Mint::unpack_unchecked(&mint_account.data).unwrap();
    assert_eq!(mint.supply, 2000 - 42 - 84);
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.amount, 1000 - 42 - 84);

    // insufficient funds approved via delegate
    do_process_instruction(
        burn(&program_id, &account_key, &mint_key, &delegate_key, &[], 1).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut delegate_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );
}

#[test]
fn test_multisig() {
    let program_id = crate::id();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let account_key = Address::new_unique();
    let mut account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let multisig_key = Address::new_unique();
    let mut multisig_account = SolanaAccount::new(42, Multisig::get_packed_len(), &program_id);
    let mut empty_multisig_account =
        SolanaAccount::new(42, Multisig::get_packed_len(), &program_id);
    let multisig_delegate_key = Address::new_unique();
    let mut multisig_delegate_account =
        SolanaAccount::new(multisig_minimum_balance(), Multisig::get_packed_len(), &program_id);
    let signer_keys = vec![Address::new_unique(); MAX_SIGNERS];
    let signer_key_refs: Vec<&Address> = signer_keys.iter().collect();
    let mut signer_accounts = vec![SolanaAccount::new(0, 0, &program_id); MAX_SIGNERS];
    let mut rent_sysvar = rent_sysvar();

    // multisig is not rent exempt
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        initialize_multisig(&program_id, &multisig_key, &[&signer_keys[0]], 1).unwrap(),
        vec![&mut multisig_account, &mut rent_sysvar, account_info_iter.next().unwrap()],
        &[Check::err(TokenError::NotRentExempt.into())],
    );

    multisig_account.lamports = multisig_minimum_balance();
    let mut multisig_account2 = multisig_account.clone();

    // single signer
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        initialize_multisig(&program_id, &multisig_key, &[&signer_keys[0]], 1).unwrap(),
        vec![&mut multisig_account, &mut rent_sysvar, account_info_iter.next().unwrap()],
        &[Check::success()],
    );

    // single signer using `initialize_multisig2`
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        initialize_multisig2(&program_id, &multisig_key, &[&signer_keys[0]], 1).unwrap(),
        vec![&mut multisig_account2, account_info_iter.next().unwrap()],
        &[Check::success()],
    );

    // multiple signer
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        initialize_multisig(
            &program_id,
            &multisig_delegate_key,
            &signer_key_refs,
            MAX_SIGNERS as u8,
        )
        .unwrap(),
        vec![
            &mut multisig_delegate_account,
            &mut rent_sysvar,
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
        ],
        &[Check::success()],
    );

    // create new mint with multisig owner
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &multisig_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create account with multisig owner
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &multisig_key).unwrap(),
        vec![&mut account, &mut mint_account, &mut multisig_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create another account with multisig owner
    do_process_instruction(
        initialize_account(&program_id, &account2_key, &mint_key, &multisig_delegate_key).unwrap(),
        vec![&mut account2_account, &mut mint_account, &mut multisig_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // mint to account
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &multisig_key, &[&signer_keys[0]], 1000)
            .unwrap(),
        vec![
            &mut mint_account,
            &mut account,
            &mut multisig_account,
            account_info_iter.next().unwrap(),
        ],
        &[Check::success()],
    );

    // fail with uninitialized multisig
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        approve(&program_id, &account_key, &multisig_delegate_key, &multisig_key, &[], 100)
            .unwrap(),
        vec![
            &mut account,
            &mut multisig_delegate_account,
            &mut empty_multisig_account,
            account_info_iter.next().unwrap(),
        ],
        &[Check::err(ProgramError::UninitializedAccount)],
    );

    // approve
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        approve(
            &program_id,
            &account_key,
            &multisig_delegate_key,
            &multisig_key,
            &[&signer_keys[0]],
            100,
        )
        .unwrap(),
        vec![
            &mut account,
            &mut multisig_delegate_account,
            &mut multisig_account,
            account_info_iter.next().unwrap(),
        ],
        &[Check::success()],
    );

    // transfer
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &account2_key, &multisig_key, &[&signer_keys[0]], 42)
            .unwrap(),
        vec![
            &mut account,
            &mut account2_account,
            &mut multisig_account,
            account_info_iter.next().unwrap(),
        ],
        &[Check::success()],
    );

    // transfer via delegate
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        #[allow(deprecated)]
        transfer(
            &program_id,
            &account_key,
            &account2_key,
            &multisig_delegate_key,
            &signer_key_refs,
            42,
        )
        .unwrap(),
        vec![
            &mut account,
            &mut account2_account,
            &mut multisig_delegate_account,
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
        ],
        &[Check::success()],
    );

    // mint to
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account2_key, &multisig_key, &[&signer_keys[0]], 42)
            .unwrap(),
        vec![
            &mut mint_account,
            &mut account2_account,
            &mut multisig_account,
            account_info_iter.next().unwrap(),
        ],
        &[Check::success()],
    );

    // burn
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        burn(&program_id, &account_key, &mint_key, &multisig_key, &[&signer_keys[0]], 42).unwrap(),
        vec![
            &mut account,
            &mut mint_account,
            &mut multisig_account,
            account_info_iter.next().unwrap(),
        ],
        &[Check::success()],
    );

    // burn via delegate
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        burn(&program_id, &account_key, &mint_key, &multisig_delegate_key, &signer_key_refs, 42)
            .unwrap(),
        vec![
            &mut account,
            &mut mint_account,
            &mut multisig_delegate_account,
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
            account_info_iter.next().unwrap(),
        ],
        &[Check::success()],
    );

    // freeze account
    let account3_key = Address::new_unique();
    let mut account3_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let mint2_key = Address::new_unique();
    let mut mint2_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    do_process_instruction(
        initialize_mint(&program_id, &mint2_key, &multisig_key, Some(&multisig_key), 2).unwrap(),
        vec![&mut mint2_account, &mut rent_sysvar],
        &[Check::success()],
    );
    do_process_instruction(
        initialize_account(&program_id, &account3_key, &mint2_key, &owner_key).unwrap(),
        vec![&mut account3_account, &mut mint2_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        mint_to(&program_id, &mint2_key, &account3_key, &multisig_key, &[&signer_keys[0]], 1000)
            .unwrap(),
        vec![
            &mut mint2_account,
            &mut account3_account,
            &mut multisig_account,
            account_info_iter.next().unwrap(),
        ],
        &[Check::success()],
    );
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        freeze_account(&program_id, &account3_key, &mint2_key, &multisig_key, &[&signer_keys[0]])
            .unwrap(),
        vec![
            &mut account3_account,
            &mut mint2_account,
            &mut multisig_account,
            account_info_iter.next().unwrap(),
        ],
        &[Check::success()],
    );

    // do SetAuthority on mint
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        set_authority(
            &program_id,
            &mint_key,
            Some(&owner_key),
            AuthorityType::MintTokens,
            &multisig_key,
            &[&signer_keys[0]],
        )
        .unwrap(),
        vec![&mut mint_account, &mut multisig_account, account_info_iter.next().unwrap()],
        &[Check::success()],
    );

    // do SetAuthority on account
    let account_info_iter = &mut signer_accounts.iter_mut();
    do_process_instruction(
        set_authority(
            &program_id,
            &account_key,
            Some(&owner_key),
            AuthorityType::AccountOwner,
            &multisig_key,
            &[&signer_keys[0]],
        )
        .unwrap(),
        vec![&mut account, &mut multisig_account, account_info_iter.next().unwrap()],
        &[Check::success()],
    );
}

#[test]
fn test_permissioned_burn_none_authority_errors() {
    let program_id = crate::id();
    let mint_key = Address::new_unique();
    let owner_key = Address::new_unique();
    let burn_authority_key = Address::new_unique();
    let account_key = Address::new_unique();

    let mint_size =
        ExtensionType::try_calculate_account_len::<PodMint>(&[ExtensionType::PermissionedBurn])
            .unwrap();

    let mut mint_account =
        SolanaAccount::new(Rent::default().minimum_balance(mint_size), mint_size, &program_id);
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let mut owner_account = SolanaAccount::default();
    let mut burn_authority_account = SolanaAccount::default();
    let mut rent_sysvar = rent_sysvar();

    do_process_instruction(
        permissioned_burn::instruction::initialize(&program_id, &mint_key, &burn_authority_key)
            .unwrap(),
        vec![&mut mint_account],
        &[Check::success()],
    );
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // Create account and mint some tokens.
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &owner_key, &[], 10).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account],
        &[Check::success()],
    );

    // Clear the permissioned burn authority.
    do_process_instruction(
        set_authority(
            &program_id,
            &mint_key,
            None,
            AuthorityType::PermissionedBurn,
            &burn_authority_key,
            &[],
        )
        .unwrap(),
        vec![&mut mint_account, &mut burn_authority_account],
        &[Check::success()],
    );

    // Attempt a permissioned burn should fail when authority is None.
    do_process_instruction(
        permissioned_burn::instruction::burn(
            &program_id,
            &account_key,
            &mint_key,
            &burn_authority_key,
            &owner_key,
            &[],
            1,
        )
        .unwrap(),
        vec![
            &mut account_account,
            &mut mint_account,
            &mut burn_authority_account,
            &mut owner_account,
        ],
        &[Check::err(TokenError::InvalidInstruction.into())],
    );

    // Standard burn should still succeed after authority is cleared.
    do_process_instruction(
        burn(&program_id, &account_key, &mint_key, &owner_key, &[], 1).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account],
        &[Check::success()],
    );
}

#[test]
fn test_validate_owner() {
    unsafe fn make_account_view(
        address: &Address,
        owner: &Address,
        is_signer: bool,
        data: &[u8],
    ) -> (Vec<u64>, AccountView) {
        // SAFETY: the caller retains the backing vector while using the view.
        unsafe { crate::runtime::testing::account(address, owner, is_signer, true, data) }
    }

    let program_id = crate::id();
    let owner_key = Address::new_unique();
    let account_to_validate = Address::new_unique();
    let mut signer_keys = [Address::default(); MAX_SIGNERS];
    for signer_key in signer_keys.iter_mut().take(MAX_SIGNERS) {
        *signer_key = Address::new_unique();
    }

    let signer_accounts: Vec<_> = signer_keys
        .iter()
        .chain(std::iter::once(&owner_key))
        // SAFETY: the backing vector stays alive until this account handle is no longer used.
        .map(|key| unsafe { make_account_view(key, &program_id, true, &[]) })
        .collect();
    let mut signers: Vec<_> = signer_accounts.iter().map(|(_, view)| view.clone()).collect();

    let mut data = vec![0; Multisig::get_packed_len()];
    let mut multisig = Multisig::unpack_unchecked(&data).unwrap();
    multisig.m = MAX_SIGNERS as u8;
    multisig.n = MAX_SIGNERS as u8;
    multisig.signers = signer_keys;
    multisig.is_initialized = true;
    Multisig::pack(multisig, &mut data).unwrap();

    let (_owner_backing, mut owner_account_info) =
        // SAFETY: the backing vector stays alive until this account handle is no longer used.
        unsafe { make_account_view(&owner_key, &program_id, false, &data) };
    let owner_account_data_len = owner_account_info.data_len();

    // no multisig, but the account is its own authority, and data is mutably
    // borrowed
    {
        let mut data = vec![0; Account::get_packed_len()];
        let mut account = Account::unpack_unchecked(&data).unwrap();
        account.owner = account_to_validate;
        Account::pack(account, &mut data).unwrap();
        let (_account_backing, account_info) =
            // SAFETY: the backing vector stays alive until this account handle is no longer used.
            unsafe { make_account_view(&account_to_validate, &program_id, true, &data) };
        let account_info_data_len = account_info.data_len();
        let mut account_alias = account_info.clone();
        let mut borrowed_data = account_alias.try_borrow_mut().unwrap();
        Processor::validate_owner(
            &program_id,
            &account_to_validate,
            &account_info,
            account_info_data_len,
            &[],
        )
        .unwrap();
        // modify the data to be sure that it wasn't silently dropped by the compiler
        borrowed_data[0] = 1;
    }

    // full 11 of 11
    Processor::validate_owner(
        &program_id,
        &owner_key,
        &owner_account_info,
        owner_account_data_len,
        &signers,
    )
    .unwrap();

    // full 11 of 11, multisig owned by tokenkeg (legacy spl-token program)
    {
        let tokenkeg_id = inline_spl_token::id();
        let mut data = vec![0; Multisig::get_packed_len()];
        let mut multisig = Multisig::unpack_unchecked(&data).unwrap();
        multisig.m = MAX_SIGNERS as u8;
        multisig.n = MAX_SIGNERS as u8;
        multisig.signers = signer_keys;
        multisig.is_initialized = true;
        Multisig::pack(multisig, &mut data).unwrap();
        let (_tokenkeg_backing, tokenkeg_multisig_info) =
            // SAFETY: the backing vector stays alive until this account handle is no longer used.
            unsafe { make_account_view(&owner_key, &tokenkeg_id, false, &data) };
        let tokenkeg_multisig_data_len = tokenkeg_multisig_info.data_len();
        Processor::validate_owner(
            &program_id,
            &owner_key,
            &tokenkeg_multisig_info,
            tokenkeg_multisig_data_len,
            &signers,
        )
        .unwrap();
    }

    // 1 of 11
    {
        let mut multisig =
            Multisig::unpack_unchecked(&owner_account_info.try_borrow().unwrap()).unwrap();
        multisig.m = 1;
        Multisig::pack(multisig, &mut owner_account_info.try_borrow_mut().unwrap()).unwrap();
    }
    Processor::validate_owner(
        &program_id,
        &owner_key,
        &owner_account_info,
        owner_account_data_len,
        &signers,
    )
    .unwrap();

    // 2:1
    {
        let mut multisig =
            Multisig::unpack_unchecked(&owner_account_info.try_borrow().unwrap()).unwrap();
        multisig.m = 2;
        multisig.n = 1;
        Multisig::pack(multisig, &mut owner_account_info.try_borrow_mut().unwrap()).unwrap();
    }
    assert_eq!(
        Err(ProgramError::MissingRequiredSignature),
        Processor::validate_owner(
            &program_id,
            &owner_key,
            &owner_account_info,
            owner_account_data_len,
            &signers
        )
    );

    // 0:11
    {
        let mut multisig =
            Multisig::unpack_unchecked(&owner_account_info.try_borrow().unwrap()).unwrap();
        multisig.m = 0;
        multisig.n = 11;
        Multisig::pack(multisig, &mut owner_account_info.try_borrow_mut().unwrap()).unwrap();
    }
    Processor::validate_owner(
        &program_id,
        &owner_key,
        &owner_account_info,
        owner_account_data_len,
        &signers,
    )
    .unwrap();

    // 2:11 but 0 provided
    {
        let mut multisig =
            Multisig::unpack_unchecked(&owner_account_info.try_borrow().unwrap()).unwrap();
        multisig.m = 2;
        multisig.n = 11;
        Multisig::pack(multisig, &mut owner_account_info.try_borrow_mut().unwrap()).unwrap();
    }
    assert_eq!(
        Err(ProgramError::MissingRequiredSignature),
        Processor::validate_owner(
            &program_id,
            &owner_key,
            &owner_account_info,
            owner_account_data_len,
            &[]
        )
    );
    // 2:11 but 1 provided
    {
        let mut multisig =
            Multisig::unpack_unchecked(&owner_account_info.try_borrow().unwrap()).unwrap();
        multisig.m = 2;
        multisig.n = 11;
        Multisig::pack(multisig, &mut owner_account_info.try_borrow_mut().unwrap()).unwrap();
    }
    assert_eq!(
        Err(ProgramError::MissingRequiredSignature),
        Processor::validate_owner(
            &program_id,
            &owner_key,
            &owner_account_info,
            owner_account_data_len,
            &signers[0..1]
        )
    );

    // 2:11, 2 from middle provided
    {
        let mut multisig =
            Multisig::unpack_unchecked(&owner_account_info.try_borrow().unwrap()).unwrap();
        multisig.m = 2;
        multisig.n = 11;
        Multisig::pack(multisig, &mut owner_account_info.try_borrow_mut().unwrap()).unwrap();
    }
    Processor::validate_owner(
        &program_id,
        &owner_key,
        &owner_account_info,
        owner_account_data_len,
        &signers[5..7],
    )
    .unwrap();

    // 11:11, one is not a signer
    {
        let mut multisig =
            Multisig::unpack_unchecked(&owner_account_info.try_borrow().unwrap()).unwrap();
        multisig.m = 11;
        multisig.n = 11;
        Multisig::pack(multisig, &mut owner_account_info.try_borrow_mut().unwrap()).unwrap();
    }
    let (_non_signer_backing, non_signer) =
        // SAFETY: the backing vector stays alive until this account handle is no longer used.
        unsafe { make_account_view(&signer_keys[5], &program_id, false, &[]) };
    signers[5] = non_signer;
    assert_eq!(
        Err(ProgramError::MissingRequiredSignature),
        Processor::validate_owner(
            &program_id,
            &owner_key,
            &owner_account_info,
            owner_account_data_len,
            &signers
        )
    );
    signers[5] = signer_accounts[5].1.clone();

    // 11:11, single signer signs multiple times
    {
        let signers = vec![signers[5].clone(); MAX_SIGNERS + 1];
        let mut multisig =
            Multisig::unpack_unchecked(&owner_account_info.try_borrow().unwrap()).unwrap();
        multisig.m = 11;
        multisig.n = 11;
        Multisig::pack(multisig, &mut owner_account_info.try_borrow_mut().unwrap()).unwrap();
        assert_eq!(
            Err(ProgramError::MissingRequiredSignature),
            Processor::validate_owner(
                &program_id,
                &owner_key,
                &owner_account_info,
                owner_account_data_len,
                &signers
            )
        );
    }
}

#[test]
fn test_owner_close_account_dups() {
    let program_id = crate::id();
    let owner_key = Address::new_unique();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint_info: AccountInfo = (&mint_key, false, &mut mint_account).into();
    let rent_key = rent::id();
    let mut rent_sysvar = rent_sysvar();
    let rent_info: AccountInfo = (&rent_key, false, &mut rent_sysvar).into();

    // create mint
    do_process_instruction_dups(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![mint_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    let to_close_key = Address::new_unique();
    let mut to_close_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let to_close_account_info: AccountInfo = (&to_close_key, true, &mut to_close_account).into();
    let destination_account_key = Address::new_unique();
    let mut destination_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let destination_account_info: AccountInfo =
        (&destination_account_key, true, &mut destination_account).into();
    // create account
    do_process_instruction_dups(
        initialize_account(&program_id, &to_close_key, &mint_key, &to_close_key).unwrap(),
        vec![
            to_close_account_info.clone(),
            mint_info.clone(),
            to_close_account_info.clone(),
            rent_info.clone(),
        ],
        &[Check::success()],
    );

    // source-owner close
    do_process_instruction_dups(
        close_account(&program_id, &to_close_key, &destination_account_key, &to_close_key, &[])
            .unwrap(),
        vec![
            to_close_account_info.clone(),
            destination_account_info.clone(),
            to_close_account_info.clone(),
        ],
        &[Check::success()],
    );
    assert!((*to_close_account_info.data.borrow()).is_empty());
}

#[test]
fn test_close_authority_close_account_dups() {
    let program_id = crate::id();
    let owner_key = Address::new_unique();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint_info: AccountInfo = (&mint_key, false, &mut mint_account).into();
    let rent_key = rent::id();
    let mut rent_sysvar = rent_sysvar();
    let rent_info: AccountInfo = (&rent_key, false, &mut rent_sysvar).into();

    // create mint
    do_process_instruction_dups(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![mint_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    let to_close_key = Address::new_unique();
    let mut to_close_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let to_close_account_info: AccountInfo = (&to_close_key, true, &mut to_close_account).into();
    let destination_account_key = Address::new_unique();
    let mut destination_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let destination_account_info: AccountInfo =
        (&destination_account_key, true, &mut destination_account).into();
    // create account
    do_process_instruction_dups(
        initialize_account(&program_id, &to_close_key, &mint_key, &to_close_key).unwrap(),
        vec![
            to_close_account_info.clone(),
            mint_info.clone(),
            to_close_account_info.clone(),
            rent_info.clone(),
        ],
        &[Check::success()],
    );
    let mut account = Account::unpack_unchecked(&to_close_account_info.data.borrow()).unwrap();
    account.close_authority = COption::Some(to_close_key);
    account.owner = owner_key;
    Account::pack(account, &mut to_close_account_info.data.borrow_mut()).unwrap();
    do_process_instruction_dups(
        close_account(&program_id, &to_close_key, &destination_account_key, &to_close_key, &[])
            .unwrap(),
        vec![
            to_close_account_info.clone(),
            destination_account_info.clone(),
            to_close_account_info.clone(),
        ],
        &[Check::success()],
    );
    assert!(to_close_account_info.data.borrow().is_empty());
}

#[test]
fn test_close_account() {
    let program_id = crate::id();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance() + 42, Account::get_packed_len(), &program_id);
    let account3_key = Address::new_unique();
    let mut account3_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner2_key = Address::new_unique();
    let mut owner2_account = SolanaAccount::default();
    let mut rent_sysvar = rent_sysvar();

    // uninitialized
    do_process_instruction(
        close_account(&program_id, &account_key, &account3_key, &owner2_key, &[]).unwrap(),
        vec![&mut account_account, &mut account3_account, &mut owner2_account],
        &[Check::err(ProgramError::UninitializedAccount)],
    );

    // initialize and mint to non-native account
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &owner_key, &[], 42).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.amount, 42);

    // initialize native account
    do_process_instruction(
        initialize_account(&program_id, &account2_key, &crate::native_mint::id(), &owner_key)
            .unwrap(),
        vec![&mut account2_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );
    let account = Account::unpack_unchecked(&account2_account.data).unwrap();
    assert!(account.is_native());
    assert_eq!(account.amount, 42);

    // close non-native account with balance
    do_process_instruction(
        close_account(&program_id, &account_key, &account3_key, &owner_key, &[]).unwrap(),
        vec![&mut account_account, &mut account3_account, &mut owner_account],
        &[Check::err(TokenError::NonNativeHasBalance.into())],
    );
    assert_eq!(account_account.lamports, account_minimum_balance());

    // empty account
    do_process_instruction(
        burn(&program_id, &account_key, &mint_key, &owner_key, &[], 42).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account],
        &[Check::success()],
    );

    // wrong owner
    do_process_instruction(
        close_account(&program_id, &account_key, &account3_key, &owner2_key, &[]).unwrap(),
        vec![&mut account_account, &mut account3_account, &mut owner2_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // close account
    do_process_instruction(
        close_account(&program_id, &account_key, &account3_key, &owner_key, &[]).unwrap(),
        vec![&mut account_account, &mut account3_account, &mut owner_account],
        &[Check::success()],
    );
    assert_eq!(account_account.lamports, 0);
    assert_eq!(account3_account.lamports, 2 * account_minimum_balance());
    assert!(account_account.data.is_empty());
    assert_eq!(account_account.owner, system_program::id());

    // fund and initialize new non-native account to test close authority
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let owner2_key = Address::new_unique();
    let mut owner2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );
    account_account.lamports = 2;

    do_process_instruction(
        set_authority(
            &program_id,
            &account_key,
            Some(&owner2_key),
            AuthorityType::CloseAccount,
            &owner_key,
            &[],
        )
        .unwrap(),
        vec![&mut account_account, &mut owner_account],
        &[Check::success()],
    );

    // account owner cannot authorize close if close_authority is set
    do_process_instruction(
        close_account(&program_id, &account_key, &account3_key, &owner_key, &[]).unwrap(),
        vec![&mut account_account, &mut account3_account, &mut owner_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // close non-native account with close_authority
    do_process_instruction(
        close_account(&program_id, &account_key, &account3_key, &owner2_key, &[]).unwrap(),
        vec![&mut account_account, &mut account3_account, &mut owner2_account],
        &[Check::success()],
    );
    assert_eq!(account_account.lamports, 0);
    assert_eq!(account3_account.lamports, 2 * account_minimum_balance() + 2);
    assert!(account_account.data.is_empty());
    assert_eq!(account_account.owner, system_program::id());

    // close native account
    do_process_instruction(
        close_account(&program_id, &account2_key, &account3_key, &owner_key, &[]).unwrap(),
        vec![&mut account2_account, &mut account3_account, &mut owner_account],
        &[Check::success()],
    );
    assert!(account2_account.data.is_empty());
    assert_eq!(account2_account.owner, system_program::id());
    assert_eq!(account3_account.lamports, 3 * account_minimum_balance() + 2 + 42);
}

#[test]
fn test_native_token() {
    let program_id = crate::id();
    let mut mint_account = native_mint();
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance() + 40, Account::get_packed_len(), &program_id);
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account3_key = Address::new_unique();
    let mut account3_account = SolanaAccount::new(account_minimum_balance(), 0, &program_id);
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner2_key = Address::new_unique();
    let mut owner2_account = SolanaAccount::default();
    let owner3_key = Address::new_unique();
    let mut rent_sysvar = rent_sysvar();

    // initialize native account
    do_process_instruction(
        initialize_account(&program_id, &account_key, &crate::native_mint::id(), &owner_key)
            .unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert!(account.is_native());
    assert_eq!(account.amount, 40);

    // initialize native account
    do_process_instruction(
        initialize_account(&program_id, &account2_key, &crate::native_mint::id(), &owner_key)
            .unwrap(),
        vec![&mut account2_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );
    let account = Account::unpack_unchecked(&account2_account.data).unwrap();
    assert!(account.is_native());
    assert_eq!(account.amount, 0);

    // mint_to unsupported
    do_process_instruction(
        mint_to(&program_id, &crate::native_mint::id(), &account_key, &owner_key, &[], 42).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account],
        &[Check::err(TokenError::NativeNotSupported.into())],
    );

    // burn unsupported
    let bogus_mint_key = Address::new_unique();
    let mut bogus_mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    do_process_instruction(
        initialize_mint(&program_id, &bogus_mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut bogus_mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    do_process_instruction(
        burn(&program_id, &account_key, &bogus_mint_key, &owner_key, &[], 42).unwrap(),
        vec![&mut account_account, &mut bogus_mint_account, &mut owner_account],
        &[Check::err(TokenError::NativeNotSupported.into())],
    );

    // ensure can't transfer below rent-exempt reserve
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &account2_key, &owner_key, &[], 50).unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner_account],
        &[Check::err(TokenError::InsufficientFunds.into())],
    );

    // transfer between native accounts
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &account2_key, &owner_key, &[], 40).unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner_account],
        &[Check::success()],
    );
    assert_eq!(account_account.lamports, account_minimum_balance());
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert!(account.is_native());
    assert_eq!(account.amount, 0);
    assert_eq!(account2_account.lamports, account_minimum_balance() + 40);
    let account = Account::unpack_unchecked(&account2_account.data).unwrap();
    assert!(account.is_native());
    assert_eq!(account.amount, 40);

    // set close authority
    do_process_instruction(
        set_authority(
            &program_id,
            &account_key,
            Some(&owner3_key),
            AuthorityType::CloseAccount,
            &owner_key,
            &[],
        )
        .unwrap(),
        vec![&mut account_account, &mut owner_account],
        &[Check::success()],
    );
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.close_authority, COption::Some(owner3_key));

    // set new account owner
    do_process_instruction(
        set_authority(
            &program_id,
            &account_key,
            Some(&owner2_key),
            AuthorityType::AccountOwner,
            &owner_key,
            &[],
        )
        .unwrap(),
        vec![&mut account_account, &mut owner_account],
        &[Check::success()],
    );

    // close authority cleared
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.close_authority, COption::None);

    // close native account
    do_process_instruction(
        close_account(&program_id, &account_key, &account3_key, &owner2_key, &[]).unwrap(),
        vec![&mut account_account, &mut account3_account, &mut owner2_account],
        &[Check::success()],
    );
    assert_eq!(account_account.lamports, 0);
    assert_eq!(account3_account.lamports, 2 * account_minimum_balance());
    assert!(account_account.data.is_empty());
    assert_eq!(account_account.owner, system_program::id());
}

#[test]
fn test_overflow() {
    let program_id = crate::id();
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner2_key = Address::new_unique();
    let mut owner2_account = SolanaAccount::default();
    let mint_owner_key = Address::new_unique();
    let mut mint_owner_account = SolanaAccount::default();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // create new mint with owner
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &mint_owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create an account
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create another account
    do_process_instruction(
        initialize_account(&program_id, &account2_key, &mint_key, &owner2_key).unwrap(),
        vec![&mut account2_account, &mut mint_account, &mut owner2_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // mint the max to an account
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &mint_owner_key, &[], u64::MAX).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut mint_owner_account],
        &[Check::success()],
    );
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.amount, u64::MAX);

    // attempt to mint one more to account
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &mint_owner_key, &[], 1).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut mint_owner_account],
        &[Check::err(TokenError::Overflow.into())],
    );
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.amount, u64::MAX);

    // attempt to mint one more to the other account
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account2_key, &mint_owner_key, &[], 1).unwrap(),
        vec![&mut mint_account, &mut account2_account, &mut mint_owner_account],
        &[Check::err(TokenError::Overflow.into())],
    );

    // burn some of the supply
    do_process_instruction(
        burn(&program_id, &account_key, &mint_key, &owner_key, &[], 100).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account],
        &[Check::success()],
    );
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.amount, u64::MAX - 100);

    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &mint_owner_key, &[], 100).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut mint_owner_account],
        &[Check::success()],
    );
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.amount, u64::MAX);

    // manipulate account balance to attempt overflow transfer
    let mut account = Account::unpack_unchecked(&account2_account.data).unwrap();
    account.amount = 1;
    Account::pack(account, &mut account2_account.data).unwrap();

    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account2_key, &account_key, &owner2_key, &[], 1).unwrap(),
        vec![&mut account2_account, &mut account_account, &mut owner2_account],
        &[Check::err(TokenError::Overflow.into())],
    );
}

#[test]
fn test_frozen() {
    let program_id = crate::id();
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account2_key = Address::new_unique();
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // create new mint and fund first account
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create account
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create another account
    do_process_instruction(
        initialize_account(&program_id, &account2_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account2_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // fund first account
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &owner_key, &[], 1000).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account],
        &[Check::success()],
    );

    // no transfer if either account is frozen
    let mut account = Account::unpack_unchecked(&account2_account.data).unwrap();
    account.state = AccountState::Frozen;
    Account::pack(account, &mut account2_account.data).unwrap();
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &account2_key, &owner_key, &[], 500).unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner_account],
        &[Check::err(TokenError::AccountFrozen.into())],
    );

    let mut account = Account::unpack_unchecked(&account_account.data).unwrap();
    account.state = AccountState::Initialized;
    Account::pack(account, &mut account_account.data).unwrap();
    let mut account = Account::unpack_unchecked(&account2_account.data).unwrap();
    account.state = AccountState::Frozen;
    Account::pack(account, &mut account2_account.data).unwrap();
    do_process_instruction(
        #[allow(deprecated)]
        transfer(&program_id, &account_key, &account2_key, &owner_key, &[], 500).unwrap(),
        vec![&mut account_account, &mut account2_account, &mut owner_account],
        &[Check::err(TokenError::AccountFrozen.into())],
    );

    // no approve if account is frozen
    let mut account = Account::unpack_unchecked(&account_account.data).unwrap();
    account.state = AccountState::Frozen;
    Account::pack(account, &mut account_account.data).unwrap();
    let delegate_key = Address::new_unique();
    let mut delegate_account = SolanaAccount::default();
    do_process_instruction(
        approve(&program_id, &account_key, &delegate_key, &owner_key, &[], 100).unwrap(),
        vec![&mut account_account, &mut delegate_account, &mut owner_account],
        &[Check::err(TokenError::AccountFrozen.into())],
    );

    // no revoke if account is frozen
    let mut account = Account::unpack_unchecked(&account_account.data).unwrap();
    account.delegate = COption::Some(delegate_key);
    account.delegated_amount = 100;
    Account::pack(account, &mut account_account.data).unwrap();
    do_process_instruction(
        revoke(&program_id, &account_key, &owner_key, &[]).unwrap(),
        vec![&mut account_account, &mut owner_account],
        &[Check::err(TokenError::AccountFrozen.into())],
    );

    // no set authority if account is frozen
    let new_owner_key = Address::new_unique();
    do_process_instruction(
        set_authority(
            &program_id,
            &account_key,
            Some(&new_owner_key),
            AuthorityType::AccountOwner,
            &owner_key,
            &[],
        )
        .unwrap(),
        vec![&mut account_account, &mut owner_account],
        &[Check::err(TokenError::AccountFrozen.into())],
    );

    // no mint_to if destination account is frozen
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &owner_key, &[], 100).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account],
        &[Check::err(TokenError::AccountFrozen.into())],
    );

    // no burn if account is frozen
    do_process_instruction(
        burn(&program_id, &account_key, &mint_key, &owner_key, &[], 100).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account],
        &[Check::err(TokenError::AccountFrozen.into())],
    );
}

#[test]
fn test_freeze_thaw_dups() {
    let program_id = crate::id();
    let account1_key = Address::new_unique();
    let mut account1_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account1_info: AccountInfo = (&account1_key, true, &mut account1_account).into();
    let owner_key = Address::new_unique();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint_info: AccountInfo = (&mint_key, true, &mut mint_account).into();
    let rent_key = rent::id();
    let mut rent_sysvar = rent_sysvar();
    let rent_info: AccountInfo = (&rent_key, false, &mut rent_sysvar).into();

    // create mint
    do_process_instruction_dups(
        initialize_mint(&program_id, &mint_key, &owner_key, Some(&account1_key), 2).unwrap(),
        vec![mint_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // create account
    do_process_instruction_dups(
        initialize_account(&program_id, &account1_key, &mint_key, &account1_key).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), account1_info.clone(), rent_info.clone()],
        &[Check::success()],
    );

    // freeze where mint freeze_authority is account
    do_process_instruction_dups(
        freeze_account(&program_id, &account1_key, &mint_key, &account1_key, &[]).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), account1_info.clone()],
        &[Check::success()],
    );

    // thaw where mint freeze_authority is account
    let mut account = Account::unpack_unchecked(&account1_info.data.borrow()).unwrap();
    account.state = AccountState::Frozen;
    Account::pack(account, &mut account1_info.data.borrow_mut()).unwrap();
    do_process_instruction_dups(
        thaw_account(&program_id, &account1_key, &mint_key, &account1_key, &[]).unwrap(),
        vec![account1_info.clone(), mint_info.clone(), account1_info.clone()],
        &[Check::success()],
    );
}

#[test]
fn test_freeze_account() {
    let program_id = crate::id();
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let account_owner_key = Address::new_unique();
    let mut account_owner_account = SolanaAccount::default();
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let owner2_key = Address::new_unique();
    let mut owner2_account = SolanaAccount::default();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // create new mint with owner different from account owner
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // create account
    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &account_owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut account_owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // mint to account
    do_process_instruction(
        mint_to(&program_id, &mint_key, &account_key, &owner_key, &[], 1000).unwrap(),
        vec![&mut mint_account, &mut account_account, &mut owner_account],
        &[Check::success()],
    );

    // mint cannot freeze
    do_process_instruction(
        freeze_account(&program_id, &account_key, &mint_key, &owner_key, &[]).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account],
        &[Check::err(TokenError::MintCannotFreeze.into())],
    );

    // missing freeze_authority
    let mut mint = Mint::unpack_unchecked(&mint_account.data).unwrap();
    mint.freeze_authority = COption::Some(owner_key);
    Mint::pack(mint, &mut mint_account.data).unwrap();
    do_process_instruction(
        freeze_account(&program_id, &account_key, &mint_key, &owner2_key, &[]).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner2_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // check explicit thaw
    do_process_instruction(
        thaw_account(&program_id, &account_key, &mint_key, &owner2_key, &[]).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner2_account],
        &[Check::err(TokenError::InvalidState.into())],
    );

    // freeze
    do_process_instruction(
        freeze_account(&program_id, &account_key, &mint_key, &owner_key, &[]).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account],
        &[Check::success()],
    );
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.state, AccountState::Frozen);

    // check explicit freeze
    do_process_instruction(
        freeze_account(&program_id, &account_key, &mint_key, &owner_key, &[]).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account],
        &[Check::err(TokenError::InvalidState.into())],
    );

    // check thaw authority
    do_process_instruction(
        thaw_account(&program_id, &account_key, &mint_key, &owner2_key, &[]).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner2_account],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    // thaw
    do_process_instruction(
        thaw_account(&program_id, &account_key, &mint_key, &owner_key, &[]).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account],
        &[Check::success()],
    );
    let account = Account::unpack_unchecked(&account_account.data).unwrap();
    assert_eq!(account.state, AccountState::Initialized);
}

#[test]
fn test_initialize_account2_and_3() {
    let program_id = crate::id();
    let account_key = Address::new_unique();
    let mut account_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let mut account2_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let mut account3_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // create mint
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    do_process_instruction(
        initialize_account(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    do_process_instruction(
        initialize_account2(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account2_account, &mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    assert_eq!(account_account, account2_account);

    do_process_instruction(
        initialize_account3(&program_id, &account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut account3_account, &mut mint_account],
        &[Check::success()],
    );

    assert_eq!(account_account, account3_account);
}

#[test]
fn initialize_account_on_non_transferable_mint() {
    let program_id = crate::id();
    let account = Address::new_unique();
    let account_len =
        ExtensionType::try_calculate_account_len::<Mint>(&[ExtensionType::NonTransferableAccount])
            .unwrap();
    let mut account_without_enough_length =
        SolanaAccount::new(Rent::default().minimum_balance(account_len), account_len, &program_id);

    let account2 = Address::new_unique();
    let account2_len = ExtensionType::try_calculate_account_len::<Mint>(&[
        ExtensionType::NonTransferableAccount,
        ExtensionType::ImmutableOwner,
    ])
    .unwrap();
    let mut account_with_enough_length = SolanaAccount::new(
        Rent::default().minimum_balance(account2_len),
        account2_len,
        &program_id,
    );

    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let mint_key = Address::new_unique();
    let mint_len =
        ExtensionType::try_calculate_account_len::<Mint>(&[ExtensionType::NonTransferable])
            .unwrap();
    let mut mint_account =
        SolanaAccount::new(Rent::default().minimum_balance(mint_len), mint_len, &program_id);
    let mut rent_sysvar = rent_sysvar();

    // create a non-transferable mint
    do_process_instruction(
        initialize_non_transferable_mint(&program_id, &mint_key).unwrap(),
        vec![&mut mint_account],
        &[Check::success()],
    );
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    //fail when account space is not enough for adding the immutable ownership
    // extension
    do_process_instruction(
        initialize_account(&program_id, &account, &mint_key, &owner_key).unwrap(),
        vec![
            &mut account_without_enough_length,
            &mut mint_account,
            &mut owner_account,
            &mut rent_sysvar,
        ],
        &[Check::err(ProgramError::InvalidAccountData)],
    );

    //success to initialize an account with enough data space
    do_process_instruction(
        initialize_account(&program_id, &account2, &mint_key, &owner_key).unwrap(),
        vec![
            &mut account_with_enough_length,
            &mut mint_account,
            &mut owner_account,
            &mut rent_sysvar,
        ],
        &[Check::success()],
    );
}

#[test]
fn test_sync_native() {
    let program_id = crate::id();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let native_account_key = Address::new_unique();
    let lamports = 40;
    let mut native_account = SolanaAccount::new(
        account_minimum_balance() + lamports,
        Account::get_packed_len(),
        &program_id,
    );
    let non_native_account_key = Address::new_unique();
    let mut non_native_account =
        SolanaAccount::new(account_minimum_balance() + 50, Account::get_packed_len(), &program_id);

    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let mut rent_sysvar = rent_sysvar();

    // initialize non-native mint
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // initialize non-native account
    do_process_instruction(
        initialize_account(&program_id, &non_native_account_key, &mint_key, &owner_key).unwrap(),
        vec![&mut non_native_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    let account = Account::unpack_unchecked(&non_native_account.data).unwrap();
    assert!(!account.is_native());
    assert_eq!(account.amount, 0);

    // fail sync non-native
    do_process_instruction(
        sync_native(&program_id, &non_native_account_key).unwrap(),
        vec![&mut non_native_account],
        &[Check::err(TokenError::NonNativeNotSupported.into())],
    );

    // fail sync uninitialized
    do_process_instruction(
        sync_native(&program_id, &native_account_key).unwrap(),
        vec![&mut native_account],
        &[Check::err(ProgramError::UninitializedAccount)],
    );

    // wrap native account
    do_process_instruction(
        initialize_account(&program_id, &native_account_key, &crate::native_mint::id(), &owner_key)
            .unwrap(),
        vec![&mut native_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    // fail sync, not owned by program
    let not_program_id = Address::new_unique();
    native_account.owner = not_program_id;
    do_process_instruction(
        sync_native(&program_id, &native_account_key).unwrap(),
        vec![&mut native_account],
        &[Check::err(ProgramError::IncorrectProgramId)],
    );
    native_account.owner = program_id;

    let account = Account::unpack_unchecked(&native_account.data).unwrap();
    assert!(account.is_native());
    assert_eq!(account.amount, lamports);

    // sync, no change
    do_process_instruction(
        sync_native(&program_id, &native_account_key).unwrap(),
        vec![&mut native_account],
        &[Check::success()],
    );
    let account = Account::unpack_unchecked(&native_account.data).unwrap();
    assert_eq!(account.amount, lamports);

    // transfer sol
    let new_lamports = lamports + 50;
    native_account.lamports = account_minimum_balance() + new_lamports;

    // success sync
    do_process_instruction(
        sync_native(&program_id, &native_account_key).unwrap(),
        vec![&mut native_account],
        &[Check::success()],
    );
    let account = Account::unpack_unchecked(&native_account.data).unwrap();
    assert_eq!(account.amount, new_lamports);

    // reduce sol
    native_account.lamports -= 1;

    // succeed sync with reduced value
    do_process_instruction(
        sync_native(&program_id, &native_account_key).unwrap(),
        vec![&mut native_account],
        &[Check::success()],
    );
    let account = Account::unpack_unchecked(&native_account.data).unwrap();
    assert_eq!(account.amount, new_lamports - 1);
}

#[test]
fn test_get_account_data_size() {
    let program_id = crate::id();
    let owner_key = Address::new_unique();
    let mut owner_account = SolanaAccount::default();
    let mut rent_sysvar = rent_sysvar();

    // Base mint
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mint_key = Address::new_unique();
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    do_process_instruction(
        get_account_data_size(&program_id, &mint_key, &[]).unwrap(),
        vec![&mut mint_account],
        &[
            Check::success(),
            Check::return_data(
                &ExtensionType::try_calculate_account_len::<Account>(&[]).unwrap().to_le_bytes(),
            ),
        ],
    );

    do_process_instruction(
        get_account_data_size(
            &program_id,
            &mint_key,
            &[
                ExtensionType::TransferFeeAmount,
                ExtensionType::TransferFeeAmount, // Duplicate user input ignored...
            ],
        )
        .unwrap(),
        vec![&mut mint_account],
        &[
            Check::success(),
            Check::return_data(
                &ExtensionType::try_calculate_account_len::<Account>(&[
                    ExtensionType::TransferFeeAmount,
                ])
                .unwrap()
                .to_le_bytes(),
            ),
        ],
    );

    // Native mint
    let mut mint_account = native_mint();

    do_process_instruction(
        get_account_data_size(&program_id, &mint_key, &[]).unwrap(),
        vec![&mut mint_account],
        &[
            Check::success(),
            Check::return_data(
                &ExtensionType::try_calculate_account_len::<Account>(&[]).unwrap().to_le_bytes(),
            ),
        ],
    );

    // Extended mint
    let mint_len =
        ExtensionType::try_calculate_account_len::<Mint>(&[ExtensionType::TransferFeeConfig])
            .unwrap();
    let mut extended_mint_account =
        SolanaAccount::new(Rent::default().minimum_balance(mint_len), mint_len, &program_id);
    let extended_mint_key = Address::new_unique();
    do_process_instruction(
        initialize_transfer_fee_config(&program_id, &extended_mint_key, None, None, 10, 4242)
            .unwrap(),
        vec![&mut extended_mint_account],
        &[Check::success()],
    );
    do_process_instruction(
        initialize_mint(&program_id, &extended_mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut extended_mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    do_process_instruction(
        get_account_data_size(&program_id, &mint_key, &[]).unwrap(),
        vec![&mut extended_mint_account],
        &[
            Check::success(),
            Check::return_data(
                &ExtensionType::try_calculate_account_len::<Account>(&[
                    ExtensionType::TransferFeeAmount,
                ])
                .unwrap()
                .to_le_bytes(),
            ),
        ],
    );

    do_process_instruction(
        get_account_data_size(
            &program_id,
            &mint_key,
            // User extension that's also added by the mint ignored...
            &[ExtensionType::TransferFeeAmount],
        )
        .unwrap(),
        vec![&mut extended_mint_account],
        &[
            Check::success(),
            Check::return_data(
                &ExtensionType::try_calculate_account_len::<Account>(&[
                    ExtensionType::TransferFeeAmount,
                ])
                .unwrap()
                .to_le_bytes(),
            ),
        ],
    );

    // Invalid mint
    let mut invalid_mint_account =
        SolanaAccount::new(account_minimum_balance(), Account::get_packed_len(), &program_id);
    let invalid_mint_key = Address::new_unique();
    do_process_instruction(
        initialize_account(&program_id, &invalid_mint_key, &mint_key, &owner_key).unwrap(),
        vec![&mut invalid_mint_account, &mut mint_account, &mut owner_account, &mut rent_sysvar],
        &[Check::success()],
    );

    do_process_instruction(
        get_account_data_size(&program_id, &invalid_mint_key, &[]).unwrap(),
        vec![&mut invalid_mint_account],
        &[Check::err(TokenError::InvalidMint.into())],
    );

    // Invalid mint owner
    let invalid_program_id = Address::new_unique();
    let mut invalid_mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let invalid_mint_key = Address::new_unique();
    let instruction = initialize_mint(&program_id, &invalid_mint_key, &owner_key, None, 2).unwrap();
    do_process_instruction(
        instruction,
        vec![&mut invalid_mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    invalid_mint_account.owner = invalid_program_id;

    do_process_instruction(
        get_account_data_size(&program_id, &invalid_mint_key, &[]).unwrap(),
        vec![&mut invalid_mint_account],
        &[Check::err(ProgramError::IncorrectProgramId)],
    );

    // Invalid Extension Type for mint and uninitialized account
    do_process_instruction(
        get_account_data_size(&program_id, &mint_key, &[ExtensionType::Uninitialized]).unwrap(),
        vec![&mut mint_account],
        &[Check::err(TokenError::ExtensionTypeMismatch.into())],
    );
    do_process_instruction(
        get_account_data_size(
            &program_id,
            &mint_key,
            &[ExtensionType::MemoTransfer, ExtensionType::MintCloseAuthority],
        )
        .unwrap(),
        vec![&mut mint_account],
        &[Check::err(TokenError::ExtensionTypeMismatch.into())],
    );
}

#[test]
fn test_amount_to_ui_amount() {
    let program_id = crate::id();
    let owner_key = Address::new_unique();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // fail if an invalid mint is passed in
    do_process_instruction(
        amount_to_ui_amount(&program_id, &mint_key, 110).unwrap(),
        vec![&mut mint_account],
        &[Check::err(TokenError::InvalidMint.into())],
    );

    // create mint
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    do_process_instruction(
        amount_to_ui_amount(&program_id, &mint_key, 23).unwrap(),
        vec![&mut mint_account],
        &[Check::success(), Check::return_data("0.23".as_bytes())],
    );

    do_process_instruction(
        amount_to_ui_amount(&program_id, &mint_key, 110).unwrap(),
        vec![&mut mint_account],
        &[Check::success(), Check::return_data("1.1".as_bytes())],
    );

    do_process_instruction(
        amount_to_ui_amount(&program_id, &mint_key, 4200).unwrap(),
        vec![&mut mint_account],
        &[Check::success(), Check::return_data("42".as_bytes())],
    );

    do_process_instruction(
        amount_to_ui_amount(&program_id, &mint_key, 0).unwrap(),
        vec![&mut mint_account],
        &[Check::success(), Check::return_data("0".as_bytes())],
    );
}

#[test]
fn test_ui_amount_to_amount() {
    let program_id = crate::id();
    let owner_key = Address::new_unique();
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);
    let mut rent_sysvar = rent_sysvar();

    // fail if an invalid mint is passed in
    do_process_instruction(
        ui_amount_to_amount(&program_id, &mint_key, "1.1").unwrap(),
        vec![&mut mint_account],
        &[Check::err(TokenError::InvalidMint.into())],
    );

    // create mint
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    do_process_instruction(
        ui_amount_to_amount(&program_id, &mint_key, "0.23").unwrap(),
        vec![&mut mint_account],
        &[Check::success(), Check::return_data(&23u64.to_le_bytes())],
    );

    do_process_instruction(
        ui_amount_to_amount(&program_id, &mint_key, "0.20").unwrap(),
        vec![&mut mint_account],
        &[Check::success(), Check::return_data(&20u64.to_le_bytes())],
    );

    do_process_instruction(
        ui_amount_to_amount(&program_id, &mint_key, "0.2000").unwrap(),
        vec![&mut mint_account],
        &[Check::success(), Check::return_data(&20u64.to_le_bytes())],
    );

    do_process_instruction(
        ui_amount_to_amount(&program_id, &mint_key, ".20").unwrap(),
        vec![&mut mint_account],
        &[Check::success(), Check::return_data(&20u64.to_le_bytes())],
    );

    do_process_instruction(
        ui_amount_to_amount(&program_id, &mint_key, "1.1").unwrap(),
        vec![&mut mint_account],
        &[Check::success(), Check::return_data(&110u64.to_le_bytes())],
    );

    do_process_instruction(
        ui_amount_to_amount(&program_id, &mint_key, "1.10").unwrap(),
        vec![&mut mint_account],
        &[Check::success(), Check::return_data(&110u64.to_le_bytes())],
    );

    do_process_instruction(
        ui_amount_to_amount(&program_id, &mint_key, "42").unwrap(),
        vec![&mut mint_account],
        &[Check::success(), Check::return_data(&4200u64.to_le_bytes())],
    );

    do_process_instruction(
        ui_amount_to_amount(&program_id, &mint_key, "42.").unwrap(),
        vec![&mut mint_account],
        &[Check::success(), Check::return_data(&4200u64.to_le_bytes())],
    );

    do_process_instruction(
        ui_amount_to_amount(&program_id, &mint_key, "0").unwrap(),
        vec![&mut mint_account],
        &[Check::success(), Check::return_data(&0u64.to_le_bytes())],
    );

    // fail if invalid ui_amount passed in
    do_process_instruction(
        ui_amount_to_amount(&program_id, &mint_key, "").unwrap(),
        vec![&mut mint_account],
        &[Check::err(ProgramError::InvalidArgument)],
    );
    do_process_instruction(
        ui_amount_to_amount(&program_id, &mint_key, ".").unwrap(),
        vec![&mut mint_account],
        &[Check::err(ProgramError::InvalidArgument)],
    );
    do_process_instruction(
        ui_amount_to_amount(&program_id, &mint_key, "0.111").unwrap(),
        vec![&mut mint_account],
        &[Check::err(ProgramError::InvalidArgument)],
    );
    do_process_instruction(
        ui_amount_to_amount(&program_id, &mint_key, "0.t").unwrap(),
        vec![&mut mint_account],
        &[Check::err(ProgramError::InvalidArgument)],
    );
}

#[test]
fn test_withdraw_excess_lamports_from_multisig() {
    let program_id = crate::id();

    let mut lamports = 0;
    let mut destination_data = vec![];
    let system_program_id = system_program::id();
    let destination_key = Address::new_unique();
    let destination_info = AccountInfo::new(
        &destination_key,
        true,
        false,
        &mut lamports,
        &mut destination_data,
        &system_program_id,
        false,
    );

    let multisig_key = Address::new_unique();
    let mut multisig_account = SolanaAccount::new(0, Multisig::get_packed_len(), &program_id);
    let excess_lamports = 4_000_000_000_000;
    multisig_account.lamports = excess_lamports + multisig_minimum_balance();
    let mut signer_keys = [Address::default(); MAX_SIGNERS];

    for signer_key in signer_keys.iter_mut().take(MAX_SIGNERS) {
        *signer_key = Address::new_unique();
    }
    let signer_refs: Vec<&Address> = signer_keys.iter().collect();
    let mut signer_lamports = 0;
    let mut signer_data = vec![];
    let mut signers: Vec<AccountInfo<'_>> = vec![
        AccountInfo::new(
            &destination_key,
            true,
            false,
            &mut signer_lamports,
            &mut signer_data,
            &program_id,
            false,
        );
        MAX_SIGNERS + 1
    ];
    for (signer, key) in signers.iter_mut().zip(&signer_keys) {
        signer.key = key;
    }

    let mut multisig = Multisig::unpack_unchecked(&vec![0; Multisig::get_packed_len()]).unwrap();
    multisig.m = MAX_SIGNERS as u8;
    multisig.n = MAX_SIGNERS as u8;
    multisig.signers = signer_keys;
    multisig.is_initialized = true;
    Multisig::pack(multisig, &mut multisig_account.data).unwrap();

    let multisig_info: AccountInfo = (&multisig_key, true, &mut multisig_account).into();

    let mut signers_infos =
        vec![multisig_info.clone(), destination_info.clone(), multisig_info.clone()];
    signers_infos.extend(signers);
    do_process_instruction_dups(
        withdraw_excess_lamports(
            &program_id,
            &multisig_key,
            &destination_key,
            &multisig_key,
            &signer_refs,
        )
        .unwrap(),
        signers_infos,
        &[Check::success()],
    );

    assert_eq!(destination_info.lamports(), excess_lamports);
}

#[test]
fn test_withdraw_excess_lamports_from_account() {
    let excess_lamports = 4_000_000_000_000;

    let program_id = crate::id();
    let account_key = Address::new_unique();
    let mut account_account = SolanaAccount::new(
        excess_lamports + account_minimum_balance(),
        Account::get_packed_len(),
        &program_id,
    );

    let system_program_id = system_program::id();
    let owner_key = Address::new_unique();

    let mut destination_lamports = 0;
    let mut destination_data = vec![];
    let destination_key = Address::new_unique();
    let destination_info = AccountInfo::new(
        &destination_key,
        true,
        false,
        &mut destination_lamports,
        &mut destination_data,
        &system_program_id,
        false,
    );
    let mint_key = Address::new_unique();
    let mut mint_account =
        SolanaAccount::new(mint_minimum_balance(), Mint::get_packed_len(), &program_id);

    let mut rent_sysvar = rent_sysvar();
    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &owner_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    let mint_info = AccountInfo::new(
        &mint_key,
        true,
        false,
        &mut mint_account.lamports,
        &mut mint_account.data,
        &program_id,
        false,
    );

    let account_info: AccountInfo = (&account_key, true, &mut account_account).into();

    do_process_instruction_dups(
        initialize_account3(&program_id, &account_key, &mint_key, &account_key).unwrap(),
        vec![account_info.clone(), mint_info.clone()],
        &[Check::success()],
    );

    do_process_instruction_dups(
        withdraw_excess_lamports(&program_id, &account_key, &destination_key, &account_key, &[])
            .unwrap(),
        vec![account_info.clone(), destination_info.clone(), account_info.clone()],
        &[Check::success()],
    );

    assert_eq!(destination_info.lamports(), excess_lamports);
}

#[test]
fn test_withdraw_excess_lamports_from_mint() {
    let excess_lamports = 4_000_000_000_000;

    let program_id = crate::id();
    let system_program_id = system_program::id();

    let mut destination_lamports = 0;
    let mut destination_data = vec![];
    let destination_key = Address::new_unique();
    let destination_info = AccountInfo::new(
        &destination_key,
        true,
        false,
        &mut destination_lamports,
        &mut destination_data,
        &system_program_id,
        false,
    );
    let mint_key = Address::new_unique();
    let mut mint_account = SolanaAccount::new(
        excess_lamports + mint_minimum_balance(),
        Mint::get_packed_len(),
        &program_id,
    );
    let mut rent_sysvar = rent_sysvar();

    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &mint_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    let mint_info: AccountInfo = (&mint_key, true, &mut mint_account).into();

    do_process_instruction_dups(
        withdraw_excess_lamports(&program_id, &mint_key, &destination_key, &mint_key, &[]).unwrap(),
        vec![mint_info.clone(), destination_info.clone(), mint_info.clone()],
        &[Check::success()],
    );

    assert_eq!(destination_info.lamports(), excess_lamports);
}

#[test]
fn test_withdraw_excess_lamports_from_mint_with_no_mint_authority() {
    let excess_lamports = 4_000_000_000_000;

    let program_id = crate::id();
    let system_program_id = system_program::id();

    let mut destination_lamports = 0;
    let mut destination_data = vec![];
    let destination_key = Address::new_unique();
    let destination_info = AccountInfo::new(
        &destination_key,
        true,
        false,
        &mut destination_lamports,
        &mut destination_data,
        &system_program_id,
        false,
    );
    let mint_key = Address::new_unique();
    let mut mint_account = SolanaAccount::new(
        excess_lamports + mint_minimum_balance(),
        Mint::get_packed_len(),
        &program_id,
    );
    let mut mint_authority_lamports = 0;
    let mut mint_authority_data = vec![];
    let mint_authority_key = Address::new_unique();
    let mint_authority_info = AccountInfo::new(
        &mint_authority_key,
        true,
        false,
        &mut mint_authority_lamports,
        &mut mint_authority_data,
        &system_program_id,
        false,
    );
    let mut rent_sysvar = rent_sysvar();

    do_process_instruction(
        initialize_mint(&program_id, &mint_key, &mint_authority_key, None, 2).unwrap(),
        vec![&mut mint_account, &mut rent_sysvar],
        &[Check::success()],
    );

    let mint_info: AccountInfo = (&mint_key, true, &mut mint_account).into();

    // fail when withdrawing with the mint as authority when there is
    // a mint authority set
    do_process_instruction_dups(
        withdraw_excess_lamports(&program_id, &mint_key, &destination_key, &mint_key, &[]).unwrap(),
        vec![mint_info.clone(), destination_info.clone(), mint_info.clone()],
        &[Check::err(TokenError::OwnerMismatch.into())],
    );

    do_process_instruction_dups(
        set_authority(
            &program_id,
            &mint_key,
            None,
            AuthorityType::MintTokens,
            &mint_authority_key,
            &[],
        )
        .unwrap(),
        vec![mint_info.clone(), mint_authority_info.clone()],
        &[Check::success()],
    );

    // fail when withdrawing with the previous mint authority
    do_process_instruction_dups(
        withdraw_excess_lamports(
            &program_id,
            &mint_key,
            &destination_key,
            &mint_authority_key,
            &[],
        )
        .unwrap(),
        vec![mint_info.clone(), destination_info.clone(), mint_authority_info.clone()],
        &[Check::err(TokenError::AuthorityTypeNotSupported.into())],
    );

    do_process_instruction_dups(
        withdraw_excess_lamports(&program_id, &mint_key, &destination_key, &mint_key, &[]).unwrap(),
        vec![mint_info.clone(), destination_info.clone(), mint_info.clone()],
        &[Check::success()],
    );

    assert_eq!(destination_info.lamports(), excess_lamports);
}
