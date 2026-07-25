pub fn transfer(context: ::quasar_lang::context::Context) -> Result<(), ProgramError> {
    __transfer_body(unsafe { <Ctx<Transfer>>::new_unchecked(context) }?)
}
#[inline(always)]
fn __transfer_body(
    mut ctx: Ctx<Transfer>,
) -> Result<(), ::quasar_lang::__solana_program_error::ProgramError> {
    let __quasar_epilogue_data = ctx.data;
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
        if <Transfer as ::quasar_lang::traits::ParseAccounts>::HAS_EPILOGUE {
            ctx.accounts.epilogue_with_context(&ctx.bumps, __quasar_epilogue_data)?;
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
    let mut __buf = core::mem::MaybeUninit::<
        [::quasar_lang::__internal::AccountView; <Transfer as ::quasar_lang::traits::AccountCount>::COUNT],
    >::uninit();
    let _ = unsafe {
        <Transfer as ::quasar_lang::traits::ParseAccountsRaw>::parse_accounts_raw(
            __accounts_start,
            __buf.as_mut_ptr() as *mut ::quasar_lang::__internal::AccountView,
            0usize,
            __program_id_addr,
        )?
    };
    let mut __accounts_buf = unsafe { __buf.assume_init() };
    let (__accounts, __bumps) = unsafe {
        <Transfer as ::quasar_lang::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
            &mut __accounts_buf,
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
