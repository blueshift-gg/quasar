use features::ID;
use quasar_lang::ProgramError;
use quasar_test::Test;

#[test]
fn a_signer_is_accepted() {
    let mut t = Test::new(ID, "features");
    let user = t.wallet(1_000_000);

    t.run(features::greet::instruction(user, ())).succeeds();
}

#[test]
fn an_account_that_did_not_sign_is_rejected() {
    let mut t = Test::new(ID, "features");
    let user = t.wallet(1_000_000);
    let mut ix = features::greet::instruction(user, ());
    ix.accounts[0].is_signer = false;

    t.run(ix).fails(ProgramError::MissingRequiredSignature);
}

#[test]
fn an_unknown_discriminator_is_rejected() {
    let mut t = Test::new(ID, "features");
    let user = t.wallet(1_000_000);
    let mut ix = features::greet::instruction(user, ());
    ix.data = vec![u8::MAX];

    t.run(ix).fails(ProgramError::InvalidInstructionData);
}

#[test]
fn trailing_instruction_data_is_rejected() {
    let mut t = Test::new(ID, "features");
    let user = t.wallet(1_000_000);
    let mut ix = features::greet::instruction(user, ());
    ix.data.push(0);

    t.run(ix).fails(ProgramError::InvalidInstructionData);
}
