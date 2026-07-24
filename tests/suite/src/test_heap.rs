use {
    crate::compat::{Instruction, InstructionError, Pubkey},
    crate::helpers::*,
    quasar_test_heap::cpi::*,
};

#[test]
fn no_heap_instruction_succeeds() {
    let mut svm = svm_heap();
    let signer = Pubkey::new_unique();
    let ix: Instruction = NoHeapOkInstruction { signer }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(
        result.is_ok(),
        "non-heap endpoint should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn heap_instruction_can_alloc() {
    let mut svm = svm_heap();
    let signer = Pubkey::new_unique();
    let ix: Instruction = HeapVecOkInstruction { signer }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(
        result.is_ok(),
        "heap endpoint should allocate successfully: {:?}",
        result.raw_result
    );
}

#[test]
fn no_heap_alloc_aborts() {
    let mut svm = svm_heap();
    let signer = Pubkey::new_unique();
    let ix: Instruction = NoHeapAllocAttemptInstruction { signer }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    // Non-heap endpoint attempts vec![1u8; 64]. In release builds (no debug
    // feature), the heap cursor is set past end of heap, so alloc returns null,
    // triggering abort. The exact error code depends on SVM handling of
    // abort_program().
    assert_eq!(
        result.raw_result,
        Err(InstructionError::ProgramFailedToComplete),
        "non-heap endpoint should abort on allocation attempt"
    );
}

/// When a program has `any_heap = true` (because at least one instruction uses
/// `#[instruction(heap)]`), the entrypoint skips heap cursor initialization,
/// deferring to per-arm init. Event handling (0xFF discriminator prefix) runs
/// through `__handle_event` BEFORE any arm's cursor init. This test verifies
/// that event emission works correctly in that codegen path.
#[test]
fn emit_event_works_with_any_heap() {
    let mut svm = svm_heap();
    let signer = Pubkey::new_unique();
    let ix: Instruction = EmitEventOkInstruction { signer }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(
        result.is_ok(),
        "event emission should succeed in any_heap program: {:?}",
        result.raw_result
    );
}

#[test]
fn heap_alloc_beyond_cap_aborts() {
    let mut svm = svm_heap();
    let signer = Pubkey::new_unique();
    let ix: Instruction = HeapAllocBeyondCapInstruction { signer }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    // A heap-enabled arm allocating 256 KiB + 1 exceeds MAX_HEAP_LENGTH: the
    // bump allocator returns null -> handle_alloc_error -> abort. Unlike
    // no_heap_alloc_aborts (poisoned cursor), this exercises the cap guard.
    assert_eq!(
        result.raw_result,
        Err(InstructionError::ProgramFailedToComplete),
        "allocation beyond the 256 KiB heap cap must abort",
    );
}
