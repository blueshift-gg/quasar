extern crate std;
use {
    quasar_lang::client::{DynString, DynVec},
    quasar_multisig_client::*,
    quasar_test::prelude::*,
};

const CREATOR: Pubkey = Pubkey::new_from_array([11; 32]);
const SIGNER1: Pubkey = Pubkey::new_from_array([12; 32]);
const SIGNER2: Pubkey = Pubkey::new_from_array([13; 32]);
const SIGNER3: Pubkey = Pubkey::new_from_array([14; 32]);
const DEPOSITOR: Pubkey = Pubkey::new_from_array([15; 32]);
const RECIPIENT: Pubkey = Pubkey::new_from_array([16; 32]);
const MAX_ELF_BYTES: usize = 26_600;
const MAX_CREATE_CU: u64 = 3_397;
const MAX_DEPOSIT_CU: u64 = 2_268;
const MAX_SET_LABEL_CU: u64 = 2_212;
const MAX_EXECUTE_TRANSFER_CU: u64 = 2_744;

#[test]
fn elf_size_stays_within_budget() {
    let bytes = std::fs::read("../../target/deploy/quasar_multisig.so").unwrap();
    assert!(
        bytes.len() <= MAX_ELF_BYTES,
        "multisig ELF grew to {} bytes; budget is {MAX_ELF_BYTES}",
        bytes.len()
    );
}

struct ConfigFixture<'a> {
    address: Pubkey,
    creator: Pubkey,
    threshold: u8,
    bump: u8,
    label: &'a str,
    signers: &'a [Pubkey],
}

impl Fixture for ConfigFixture<'_> {
    type Output = Pubkey;

    fn install(self, test: &mut Test) -> Self::Output {
        let config = MultisigConfig {
            creator: self.creator,
            threshold: self.threshold,
            bump: self.bump,
            label: DynString::<u8>::new(self.label),
            signers: DynVec::<Pubkey, u16>::new(self.signers.to_vec()),
        };
        test.add(Account::new(
            self.address,
            crate::ID,
            1_000_000,
            wincode::serialize(&config).unwrap(),
        ))
    }
}

fn config_fixture<'a>(
    address: Pubkey,
    creator: Pubkey,
    threshold: u8,
    bump: u8,
    label: &'a str,
    signers: &'a [Pubkey],
) -> ConfigFixture<'a> {
    ConfigFixture {
        address,
        creator,
        threshold,
        bump,
        label,
        signers,
    }
}

#[quasar_test]
fn create_initializes_dynamic_config(test: &mut Test) {
    test.add(Wallet::new().at(CREATOR));
    let config = find_config_address(&CREATOR, &crate::ID).0;

    let outcome = test.send(CreateInstruction {
        creator: CREATOR,
        threshold: 2,
        remaining_accounts: co_signers(&[SIGNER1, SIGNER2, SIGNER3]),
    });

    outcome.succeeds().cu_at_most(MAX_CREATE_CU);
    let ProgramAccount::MultisigConfig(state) = outcome.account_as(config, decode_account).unwrap();
    assert_eq!(state.creator, CREATOR);
    assert_eq!(state.threshold, 2);
    assert_eq!(state.signers.len(), 3);
    assert!(outcome
        .account_changes()
        .iter()
        .any(|change| change.address() == config && change.was_created()));
}

#[quasar_test]
fn deposit_funds_the_multisig_vault(test: &mut Test) {
    test.add(Wallet::new().at(DEPOSITOR));
    let (config, bump) = find_config_address(&CREATOR, &crate::ID);
    let vault = find_vault_address(&config, &crate::ID).0;
    test.add(config_fixture(
        config,
        CREATOR,
        2,
        bump,
        "",
        &[SIGNER1, SIGNER2],
    ));

    test.send(DepositInstruction {
        depositor: DEPOSITOR,
        config,
        amount: 1_000_000_000,
    })
    .succeeds()
    .cu_at_most(MAX_DEPOSIT_CU)
    .has_lamports(vault, 1_000_000_000);
}

#[quasar_test]
fn set_label_updates_dynamic_state(test: &mut Test) {
    test.add(Wallet::new().at(CREATOR));
    let (config, bump) = find_config_address(&CREATOR, &crate::ID);
    test.add(config_fixture(config, CREATOR, 1, bump, "", &[SIGNER1]));

    let outcome = test.send(SetLabelInstruction {
        creator: CREATOR,
        label: DynString::<u8>::new("Treasury"),
    });

    outcome.succeeds().cu_at_most(MAX_SET_LABEL_CU);
    let ProgramAccount::MultisigConfig(state) = outcome.account_as(config, decode_account).unwrap();
    assert_eq!(state.label.as_bytes(), b"Treasury");
}

fn transfer_world(test: &mut Test) -> (Pubkey, Pubkey) {
    let (config, bump) = find_config_address(&CREATOR, &crate::ID);
    let vault = find_vault_address(&config, &crate::ID).0;
    test.add(config_fixture(
        config,
        CREATOR,
        2,
        bump,
        "",
        &[SIGNER1, SIGNER2, SIGNER3],
    ));
    test.add(Wallet::new().at(vault).lamports(5_000_000_000));
    (config, vault)
}

fn transfer_instruction(signers: &[Pubkey]) -> ExecuteTransferInstruction {
    ExecuteTransferInstruction {
        creator: CREATOR,
        recipient: RECIPIENT,
        amount: 1_000_000_000,
        remaining_accounts: co_signers(signers),
    }
}

#[quasar_test]
fn execute_transfer_accepts_the_threshold(test: &mut Test) {
    let (_, vault) = transfer_world(test);

    test.send(transfer_instruction(&[SIGNER1, SIGNER2]))
        .succeeds()
        .cu_at_most(MAX_EXECUTE_TRANSFER_CU)
        .has_lamports(vault, 4_000_000_000)
        .has_lamports(RECIPIENT, 1_000_000_000);
}

#[quasar_test]
fn execute_transfer_rejects_too_few_signers(test: &mut Test) {
    transfer_world(test);

    test.send(transfer_instruction(&[SIGNER1]))
        .fails(ProgramError::MissingRequiredSignature);
}

#[quasar_test]
fn execute_transfer_counts_a_duplicate_once(test: &mut Test) {
    transfer_world(test);

    test.send(transfer_instruction(&[SIGNER1, SIGNER1]))
        .fails(ProgramError::MissingRequiredSignature);
}
