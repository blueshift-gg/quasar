pub fn approve(context: ::quasar_lang::context::Context) -> Result<(), ProgramError> {
    __approve_body(
        unsafe {
            <::quasar_lang::context::CtxWithRemaining<Approve>>::new_unchecked(context)
        }?,
    )
}
#[inline(always)]
fn __approve_body(
    mut ctx: ::quasar_lang::context::CtxWithRemaining<Approve>,
) -> Result<(), ::quasar_lang::__solana_program_error::ProgramError> {
    let __quasar_epilogue_data = ctx.data;
    #[repr(C)]
    struct __InstructionDataZc {
        amount: <u64 as ::quasar_lang::instruction_arg::InstructionArg>::Zc,
    }
    const _: () = assert!(
        core::mem::align_of:: < __InstructionDataZc > () == 1,
        "fixed instruction data ZC layout must have alignment 1"
    );
    const __INSTRUCTION_DATA_SIZE: usize = core::mem::size_of::<__InstructionDataZc>();
    if ctx.data.len() < __INSTRUCTION_DATA_SIZE {
        return Err(
            ::quasar_lang::__solana_program_error::ProgramError::InvalidInstructionData,
        );
    }
    let __zc = unsafe { &*(ctx.data.as_ptr() as *const __InstructionDataZc) };
    <u64 as ::quasar_lang::instruction_arg::InstructionArg>::validate_zc(&__zc.amount)
        .map_err(|_| {
            ::quasar_lang::__solana_program_error::ProgramError::InvalidInstructionData
        })?;
    let amount = <u64 as ::quasar_lang::instruction_arg::InstructionArg>::from_zc(
        &__zc.amount,
    );
    let mut ctx: CtxWithRemaining<Approve, Signer, 10> = ctx.into_typed()?;
    ctx.data = &[];
    {
        let __user_result: Result<
            (),
            ::quasar_lang::__solana_program_error::ProgramError,
        > = { ctx.accounts.handler(amount, ctx.remaining) };
        __user_result?;
        if <Approve as ::quasar_lang::traits::ParseAccounts>::HAS_EPILOGUE {
            ctx.accounts.epilogue_with_context(&ctx.bumps, __quasar_epilogue_data)?;
        }
        Ok(())
    }
}
