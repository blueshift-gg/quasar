use {crate::compat::Instruction, crate::helpers::*, quasar_test_misc::cpi::*};

#[test]
fn cpi_invoke_with_return_round_trips_u64() {
    let mut svm = svm_misc();
    let ix: Instruction = CpiInvokeWithReturnInstruction {}.into();

    let result = svm.process_instruction(&ix, &[]);
    assert!(result.is_ok(), "u64 return: {:?}", result.raw_result);
}

#[test]
fn cpi_invoke_with_return_round_trips_struct() {
    let mut svm = svm_misc();
    let ix: Instruction = CpiInvokeStructReturnInstruction {}.into();

    let result = svm.process_instruction(&ix, &[]);
    assert!(result.is_ok(), "struct return: {:?}", result.raw_result);
}

#[test]
fn cpi_plain_invoke_ignores_return_data() {
    let mut svm = svm_misc();
    let ix: Instruction = CpiInvokeIgnoreReturnInstruction {}.into();

    let result = svm.process_instruction(&ix, &[]);
    assert!(result.is_ok(), "plain invoke: {:?}", result.raw_result);
}

#[test]
fn cpi_invoke_with_return_detects_missing_return_after_prior_success() {
    let mut svm = svm_misc();
    let ix: Instruction = CpiInvokeMissingReturnInstruction {}.into();

    let result = svm.process_instruction(&ix, &[]);
    assert!(result.is_ok(), "missing return: {:?}", result.raw_result);
}

// Rejection paths, driven through the test-errors fixtures so the framework
// error propagates to the transaction level instead of being caught
// in-program (compare cpi_invoke_with_return_detects_missing_return_...).

use {
    crate::compat::ProgramError, quasar_lang::prelude::QuasarError,
    quasar_test_errors::cpi as err_cpi,
};

#[test]
fn cpi_missing_return_data_rejects() {
    // Callee succeeds but sets no return data: invoke_with_return must fail
    // with MissingReturnData rather than hand back stale bytes.
    let mut svm = svm_errors();
    let ix: Instruction = err_cpi::CpiMissingReturnInstruction {}.into();
    let result = svm.process_instruction(&ix, &[]);
    result.assert_error(ProgramError::Custom(QuasarError::MissingReturnData as u32));
}

#[test]
fn cpi_return_length_mismatch_rejects() {
    // Callee returns 12 bytes; caller decodes u64 (8 bytes): the typed decode
    // must fail with InvalidReturnData rather than read a prefix.
    let mut svm = svm_errors();
    let ix: Instruction = err_cpi::CpiDecodeMismatchInstruction {}.into();
    let result = svm.process_instruction(&ix, &[]);
    result.assert_error(ProgramError::Custom(QuasarError::InvalidReturnData as u32));
}

#[test]
fn cpi_return_data_from_wrong_program_rejects() {
    // The invoked callee leaves return data stamped by a different program
    // (its own nested CPI): the program-id check on returned data must fail
    // with ReturnDataFromWrongProgram rather than trust foreign bytes.
    let mut svm = svm_errors_with_misc();
    let ix: Instruction = err_cpi::CpiWrongReturnProgramInstruction {
        misc_program: quasar_test_misc::ID,
    }
    .into();
    let result = svm.process_instruction(&ix, &[]);
    result.assert_error(ProgramError::Custom(
        QuasarError::ReturnDataFromWrongProgram as u32,
    ));
}
