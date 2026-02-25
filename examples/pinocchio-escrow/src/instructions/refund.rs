#[cfg(target_os = "solana")]
use pinocchio::syscalls::sol_log_data;
/// # Refund Instruction
///
/// Cancels the escrow: returns deposited tokens to the maker.
///
/// | # | Account       | Signer | Writable | Description                    |
/// |---|---------------|--------|----------|--------------------------------|
/// | 0 | maker         | yes    | yes      | Original escrow creator        |
/// | 1 | escrow        | no     | yes      | PDA: `["escrow", maker]`       |
/// | 2 | maker_ta_a    | no     | yes      | Maker's token-A account        |
/// | 3 | vault_ta_a    | no     | yes      | Vault token-A account          |
/// | 4 | token_program | no     | no       | SPL Token program              |
use pinocchio::{cpi::Signer, AccountView, Address, ProgramResult};
use {
    crate::{
        errors::EscrowError,
        state::EscrowAccount,
        utils::{pda::escrow_seeds, Context},
    },
    pinocchio_token::instructions::{CloseAccount, Transfer},
};

#[allow(dead_code)]
pub struct RefundAccounts<'info> {
    maker: &'info AccountView,
    escrow: &'info AccountView,
    maker_ta_a: &'info AccountView,
    vault_ta_a: &'info AccountView,
    token_program: &'info AccountView,
}

pub struct Refund<'info> {
    accounts: RefundAccounts<'info>,
    bump: u8,
}

impl<'info> TryFrom<Context<'info>> for Refund<'info> {
    type Error = EscrowError;

    fn try_from(ctx: Context<'info>) -> Result<Self, EscrowError> {
        let [maker, escrow, maker_ta_a, vault_ta_a, token_program, ..] = ctx.accounts else {
            return Err(EscrowError::NotEnoughAccountKeys);
        };

        if !maker.is_signer() {
            return Err(EscrowError::MissingRequiredSignature);
        }

        if token_program.address() != &pinocchio_token::ID {
            return Err(EscrowError::IncorrectTokenProgram);
        }

        if !maker_ta_a.owned_by(&pinocchio_token::ID) {
            return Err(EscrowError::InvalidAccountOwner);
        }
        if !vault_ta_a.owned_by(&pinocchio_token::ID) {
            return Err(EscrowError::InvalidAccountOwner);
        }

        let state = EscrowAccount::load(escrow).map_err(|_| EscrowError::InvalidEscrowState)?;

        if state.maker().as_ref() != maker.address().as_ref() {
            return Err(EscrowError::InvalidMaker);
        }

        let expected = Address::create_program_address(
            &[
                EscrowAccount::SEEDS_PREFIX,
                maker.address().as_ref(),
                &[state.bump()],
            ],
            &crate::ID,
        )
        .map_err(|_| EscrowError::InvalidPDA)?;

        if expected.ne(escrow.address()) {
            return Err(EscrowError::InvalidPDA);
        }

        let bump = state.bump();
        drop(state);

        Ok(Self {
            accounts: RefundAccounts {
                maker,
                escrow,
                maker_ta_a,
                vault_ta_a,
                token_program,
            },
            bump,
        })
    }
}

impl<'info> Refund<'info> {
    pub fn process(ctx: Context<'info>) -> ProgramResult {
        Self::try_from(ctx)
            .map_err(Into::into)
            .and_then(|r| r.execute())
    }

    #[inline(always)]
    fn execute(self) -> ProgramResult {
        self.withdraw_tokens_and_close()?;
        self.emit_event()?;
        self.close_escrow()
    }

    #[inline(always)]
    fn withdraw_tokens_and_close(&self) -> ProgramResult {
        let a = &self.accounts;
        let bump_bytes = [self.bump];
        let seeds = escrow_seeds(a.maker.address().as_ref(), &bump_bytes);
        let signers = [Signer::from(&seeds)];

        // SAFETY: No mutable borrows active, offset 64 is the amount field in SPL token layout.
        // The u64 cast is technically misaligned (align 1 data), but SBF handles unaligned
        // access natively.
        let vault_amount =
            unsafe { *(a.vault_ta_a.borrow_unchecked().as_ptr().add(64) as *const u64) };

        Transfer {
            from: a.vault_ta_a,
            to: a.maker_ta_a,
            authority: a.escrow,
            amount: vault_amount,
        }
        .invoke_signed(&signers)?;

        CloseAccount {
            account: a.vault_ta_a,
            destination: a.maker,
            authority: a.escrow,
        }
        .invoke_signed(&signers)
    }

    #[inline(always)]
    fn emit_event(&self) -> ProgramResult {
        #[cfg(target_os = "solana")]
        {
            let fields: [&[u8]; 2] = [&[2u8], self.accounts.escrow.address().as_ref()];
            unsafe { sol_log_data(fields.as_ptr() as *const u8, fields.len() as u64) };
        }
        Ok(())
    }

    #[inline(always)]
    fn close_escrow(&self) -> ProgramResult {
        let a = &self.accounts;
        let src_lamports = a.escrow.lamports();
        a.maker.set_lamports(a.maker.lamports() + src_lamports);
        a.escrow.set_lamports(0);
        let mut data = a.escrow.try_borrow_mut()?;
        data.fill(0);
        Ok(())
    }
}
