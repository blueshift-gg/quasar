/// # Make Instruction
///
/// Creates a new escrow and deposits tokens into the vault.
///
/// | # | Account       | Signer | Writable | Description                    |
/// |---|---------------|--------|----------|--------------------------------|
/// | 0 | maker         | yes    | yes      | Escrow creator                 |
/// | 1 | escrow        | no     | yes      | PDA: `["escrow", maker]`       |
/// | 2 | maker_ta_a    | no     | yes      | Maker's token-A account        |
/// | 3 | maker_ta_b    | no     | no       | Maker's token-B account        |
/// | 4 | vault_ta_a    | no     | yes      | Vault token-A account          |
/// | 5 | rent          | no     | no       | Rent sysvar                    |
/// | 6 | token_program | no     | no       | SPL Token program              |
/// | 7 | system_program| no     | no       | System program                 |
use pinocchio::{cpi::Signer, AccountView, ProgramResult};
use pinocchio_system::create_account_with_minimum_balance_signed;
use pinocchio_token::instructions::Transfer;

use crate::errors::EscrowError;
use crate::state::EscrowAccount;
use crate::utils::pda::{escrow_pda, escrow_seeds};
use crate::utils::Context;

#[cfg(target_os = "solana")]
use pinocchio::syscalls::sol_log_data;

#[allow(dead_code)]
pub struct MakeAccounts<'info> {
    maker: &'info AccountView,
    escrow: &'info AccountView,
    maker_ta_a: &'info AccountView,
    maker_ta_b: &'info AccountView,
    vault_ta_a: &'info AccountView,
    rent: &'info AccountView,
    token_program: &'info AccountView,
    system_program: &'info AccountView,
}

pub struct Make<'info> {
    accounts: MakeAccounts<'info>,
    deposit: u64,
    receive: u64,
    bump: u8,
}

impl<'info> TryFrom<Context<'info>> for Make<'info> {
    type Error = EscrowError;

    fn try_from(ctx: Context<'info>) -> Result<Self, EscrowError> {
        let [maker, escrow, maker_ta_a, maker_ta_b, vault_ta_a, rent, token_program, system_program, ..] =
            ctx.accounts
        else {
            return Err(EscrowError::NotEnoughAccountKeys);
        };

        if !maker.is_signer() {
            return Err(EscrowError::MissingRequiredSignature);
        }

        if token_program.address() != &pinocchio_token::ID {
            return Err(EscrowError::IncorrectTokenProgram);
        }
        if system_program.address() != &pinocchio_system::ID {
            return Err(EscrowError::IncorrectSystemProgram);
        }

        if !maker_ta_a.owned_by(&pinocchio_token::ID) {
            return Err(EscrowError::InvalidAccountOwner);
        }
        if !maker_ta_b.owned_by(&pinocchio_token::ID) {
            return Err(EscrowError::InvalidAccountOwner);
        }
        if !vault_ta_a.owned_by(&pinocchio_token::ID) {
            return Err(EscrowError::InvalidAccountOwner);
        }

        if ctx.data.len() < 16 {
            return Err(EscrowError::InvalidInstructionData);
        }
        // SAFETY: Bounds-checked above, SBF is little-endian. The u64 casts are technically
        // misaligned (data pointer is align 1), but SBF handles unaligned access natively.
        let deposit = unsafe { *(ctx.data.as_ptr() as *const u64) };
        let receive = unsafe { *(ctx.data.as_ptr().add(8) as *const u64) };

        let (expected, bump) = escrow_pda(maker.address().as_ref());
        if expected.ne(escrow.address()) {
            return Err(EscrowError::InvalidPDA);
        }

        Ok(Self {
            accounts: MakeAccounts {
                maker,
                escrow,
                maker_ta_a,
                maker_ta_b,
                vault_ta_a,
                rent,
                token_program,
                system_program,
            },
            deposit,
            receive,
            bump,
        })
    }
}

impl<'info> Make<'info> {
    pub fn process(ctx: Context<'info>) -> ProgramResult {
        Self::try_from(ctx)
            .map_err(Into::into)
            .and_then(|m| m.execute())
    }

    #[inline(always)]
    fn execute(&self) -> ProgramResult {
        self.create_escrow()?;
        self.emit_event()?;
        self.deposit_tokens()
    }

    #[inline(always)]
    fn create_escrow(&self) -> ProgramResult {
        let a = &self.accounts;
        let bump_bytes = [self.bump];
        let seeds = escrow_seeds(a.maker.address().as_ref(), &bump_bytes);
        let signers = [Signer::from(&seeds)];

        create_account_with_minimum_balance_signed(
            a.escrow,
            EscrowAccount::LEN,
            &crate::ID,
            a.maker,
            Some(a.rent),
            &signers,
        )?;

        let mut state = EscrowAccount::init(a.escrow)?;
        state.set_maker(a.maker.address().as_ref());

        // Safety: no mutable borrows active on token accounts,
        // token account data is always >= 32 bytes (mint at offset 0)
        unsafe {
            let ta_a = a.maker_ta_a.borrow_unchecked();
            state.set_mint_a(&ta_a[..32]);

            let ta_b = a.maker_ta_b.borrow_unchecked();
            state.set_mint_b(&ta_b[..32]);
        }

        state.set_maker_ta_b(a.maker_ta_b.address().as_ref());
        state.set_receive(self.receive);
        state.set_bump(self.bump);

        Ok(())
    }

    #[inline(always)]
    fn emit_event(&self) -> ProgramResult {
        #[cfg(target_os = "solana")]
        {
            let a = &self.accounts;
            let fields: [&[u8]; 7] = [
                &[0u8],
                a.escrow.address().as_ref(),
                a.maker.address().as_ref(),
                &unsafe { a.maker_ta_a.borrow_unchecked() }[0..32],
                &unsafe { a.maker_ta_b.borrow_unchecked() }[0..32],
                &self.deposit.to_le_bytes(),
                &self.receive.to_le_bytes(),
            ];
            unsafe { sol_log_data(fields.as_ptr() as *const u8, fields.len() as u64) };
        }
        Ok(())
    }

    #[inline(always)]
    fn deposit_tokens(&self) -> ProgramResult {
        let a = &self.accounts;
        Transfer {
            from: a.maker_ta_a,
            to: a.vault_ta_a,
            authority: a.maker,
            amount: self.deposit,
        }
        .invoke()
    }
}
