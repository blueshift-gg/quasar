/// # Take Instruction
///
/// Completes the escrow: taker sends token-B to maker, receives token-A from vault.
///
/// | # | Account       | Signer | Writable | Description                    |
/// |---|---------------|--------|----------|--------------------------------|
/// | 0 | taker         | yes    | yes      | Escrow taker                   |
/// | 1 | escrow        | no     | yes      | PDA: `["escrow", maker]`       |
/// | 2 | maker         | no     | yes      | Original escrow creator        |
/// | 3 | taker_ta_a    | no     | yes      | Taker's token-A account        |
/// | 4 | taker_ta_b    | no     | yes      | Taker's token-B account        |
/// | 5 | maker_ta_b    | no     | yes      | Maker's token-B account        |
/// | 6 | vault_ta_a    | no     | yes      | Vault token-A account          |
/// | 7 | token_program | no     | no       | SPL Token program              |
use pinocchio::{cpi::Signer, AccountView, Address, ProgramResult};
use pinocchio_token::instructions::{CloseAccount, Transfer};

use crate::errors::EscrowError;
use crate::state::EscrowAccount;
use crate::utils::pda::escrow_seeds;
use crate::utils::Context;

#[cfg(target_os = "solana")]
use pinocchio::syscalls::sol_log_data;

#[allow(dead_code)]
pub struct TakeAccounts<'info> {
    taker: &'info AccountView,
    escrow: &'info AccountView,
    maker: &'info AccountView,
    taker_ta_a: &'info AccountView,
    taker_ta_b: &'info AccountView,
    maker_ta_b: &'info AccountView,
    vault_ta_a: &'info AccountView,
    token_program: &'info AccountView,
}

pub struct Take<'info> {
    accounts: TakeAccounts<'info>,
    receive: u64,
    bump: u8,
}

impl<'info> TryFrom<Context<'info>> for Take<'info> {
    type Error = EscrowError;

    fn try_from(ctx: Context<'info>) -> Result<Self, EscrowError> {
        let [taker, escrow, maker, taker_ta_a, taker_ta_b, maker_ta_b, vault_ta_a, token_program, ..] =
            ctx.accounts
        else {
            return Err(EscrowError::NotEnoughAccountKeys);
        };

        if !taker.is_signer() {
            return Err(EscrowError::MissingRequiredSignature);
        }

        if token_program.address() != &pinocchio_token::ID {
            return Err(EscrowError::IncorrectTokenProgram);
        }

        if !taker_ta_a.owned_by(&pinocchio_token::ID) {
            return Err(EscrowError::InvalidAccountOwner);
        }
        if !taker_ta_b.owned_by(&pinocchio_token::ID) {
            return Err(EscrowError::InvalidAccountOwner);
        }
        if !maker_ta_b.owned_by(&pinocchio_token::ID) {
            return Err(EscrowError::InvalidMakerTokenAccount);
        }
        if !vault_ta_a.owned_by(&pinocchio_token::ID) {
            return Err(EscrowError::InvalidAccountOwner);
        }

        let state = EscrowAccount::load(escrow).map_err(|_| EscrowError::InvalidEscrowState)?;

        if state.maker().as_ref() != maker.address().as_ref() {
            return Err(EscrowError::InvalidMaker);
        }

        if state.maker_ta_b().as_ref() != maker_ta_b.address().as_ref() {
            return Err(EscrowError::InvalidMakerTokenAccount);
        }

        if state.receive() == 0 {
            return Err(EscrowError::ZeroReceiveAmount);
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

        let receive = state.receive();
        let bump = state.bump();
        drop(state);

        Ok(Self {
            accounts: TakeAccounts {
                taker,
                escrow,
                maker,
                taker_ta_a,
                taker_ta_b,
                maker_ta_b,
                vault_ta_a,
                token_program,
            },
            receive,
            bump,
        })
    }
}

impl<'info> Take<'info> {
    pub fn process(ctx: Context<'info>) -> ProgramResult {
        Self::try_from(ctx)
            .map_err(Into::into)
            .and_then(|t| t.execute())
    }

    #[inline(always)]
    fn execute(self) -> ProgramResult {
        self.transfer_tokens()?;
        self.withdraw_tokens_and_close()?;
        self.emit_event()?;
        self.close_escrow()
    }

    #[inline(always)]
    fn transfer_tokens(&self) -> ProgramResult {
        let a = &self.accounts;
        Transfer {
            from: a.taker_ta_b,
            to: a.maker_ta_b,
            authority: a.taker,
            amount: self.receive,
        }
        .invoke()
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
            to: a.taker_ta_a,
            authority: a.escrow,
            amount: vault_amount,
        }
        .invoke_signed(&signers)?;

        CloseAccount {
            account: a.vault_ta_a,
            destination: a.taker,
            authority: a.escrow,
        }
        .invoke_signed(&signers)
    }

    #[inline(always)]
    fn emit_event(&self) -> ProgramResult {
        #[cfg(target_os = "solana")]
        {
            let fields: [&[u8]; 2] = [&[1u8], self.accounts.escrow.address().as_ref()];
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
