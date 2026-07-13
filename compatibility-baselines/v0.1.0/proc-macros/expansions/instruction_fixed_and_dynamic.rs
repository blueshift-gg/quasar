pub fn transfer(
    mut context: ::quasar_lang::context::Context,
) -> Result<(), ProgramError> {
    __transfer_body(unsafe { <Ctx<Transfer>>::new_unchecked(context) }?)
}
#[inline(always)]
fn __transfer_body(
    mut ctx: Ctx<Transfer>,
) -> Result<(), ::quasar_lang::__solana_program_error::ProgramError> {
    use ::quasar_lang::__zeropod as zeropod;
    #[derive(zeropod::ZeroPod)]
    #[zeropod(compact)]
    struct __InstructionDataCompact {
        amount: u64,
        memo: zeropod::pod::PodString<8, 1usize>,
    }
    <__InstructionDataCompact as ::quasar_lang::ZeroPodCompact>::validate(&ctx.data)
        .map_err(|_| {
            ::quasar_lang::__solana_program_error::ProgramError::InvalidInstructionData
        })?;
    let __ref = unsafe { __InstructionDataCompactRef::new_unchecked(&ctx.data) };
    <u64 as ::quasar_lang::instruction_arg::InstructionArg>::validate_zc(&__ref.amount)
        .map_err(|_| {
            ::quasar_lang::__solana_program_error::ProgramError::InvalidInstructionData
        })?;
    let amount = <u64 as ::quasar_lang::instruction_arg::InstructionArg>::from_zc(
        &__ref.amount,
    );
    let memo = __ref.memo();
    ctx.data = &[];
    {
        let __user_result: Result<
            (),
            ::quasar_lang::__solana_program_error::ProgramError,
        > = { ctx.accounts.handler(amount, memo) };
        __user_result?;
        if ctx.has_epilogue() {
            ctx.accounts.epilogue()?;
        }
        Ok(())
    }
}
#[inline(always)]
fn __quasar_direct_transfer(
    __program_id: &[u8; 32],
    __accounts_start: *mut u8,
    __ix_data: &[u8],
) -> Result<(), ::quasar_lang::__solana_program_error::ProgramError> {
    let __program_id_addr = unsafe {
        &*(__program_id as *const [u8; 32] as *const ::quasar_lang::prelude::Address)
    };
    let (__accounts, __bumps) = unsafe {
        <Transfer>::parse_direct_with_instruction_data_unchecked(
            __accounts_start,
            __ix_data,
            __program_id_addr,
        )?
    };
    __transfer_body(::quasar_lang::context::Ctx {
        accounts: __accounts,
        bumps: __bumps,
        program_id: __program_id,
        data: __ix_data,
    })
}
