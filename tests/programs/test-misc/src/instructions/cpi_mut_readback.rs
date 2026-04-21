use {
    crate::state::{SimpleAccount, SimpleAccountInner},
    quasar_lang::prelude::*,
};

/// CPI pointer safety under mutable data access: the handler writes to
/// account data via `set_inner()` (which uses `data_mut_ptr()` — raw pointer
/// write, no borrow tracking), then passes the SAME account into a system
/// transfer CPI as the writable destination.
///
/// `cpi_account_from_view()` extracts raw `*const` pointers from the
/// `AccountView` without checking `borrow_state`. This test verifies:
///
///   1. The data write from `set_inner()` survives the CPI round-trip (SVM
///      serialize → execute → deserialize doesn't clobber it).
///   2. The CPI's lamport change is visible through the same `AccountView`
///      after CPI returns.
///   3. A second `set_inner()` after CPI still writes correctly (the
///      `data_mut_ptr()` is still valid).
#[derive(Accounts)]
pub struct CpiMutReadback {
    #[account(mut)]
    pub account: Account<SimpleAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,
}

impl CpiMutReadback {
    #[inline(always)]
    pub fn handler(&mut self, new_value: u64) -> Result<(), ProgramError> {
        let authority = self.account.authority;
        let bump = self.account.bump;
        let initial_lamports = self.account.to_account_view().lamports();

        // --- Step 1: Write to account data via set_inner ---
        // Uses data_mut_ptr() internally — raw pointer, no borrow tracking.
        self.account.set_inner(SimpleAccountInner {
            authority,
            value: new_value,
            bump,
        });

        // Verify the write landed
        if self.account.value != new_value {
            return Err(ProgramError::Custom(1)); // set_inner write failed
        }

        // --- Step 2: CPI transfer lamports TO the same account ---
        // cpi_account_from_view() extracts raw pointers from the AccountView
        // without checking borrow_state. The SVM writes to the same
        // RuntimeAccount memory that set_inner just wrote to.
        self.system_program
            .transfer(&self.payer, &self.account, 1_000u64)
            .invoke()?;

        // --- Step 3: Read back through the same references ---
        // Custom(2): lamports not updated after CPI
        if self.account.to_account_view().lamports() != initial_lamports + 1_000 {
            return Err(ProgramError::Custom(2));
        }
        // Custom(3): data clobbered by CPI round-trip
        if self.account.value != new_value {
            return Err(ProgramError::Custom(3));
        }

        // --- Step 4: Second set_inner after CPI ---
        // data_mut_ptr() must still point to valid memory.
        let second_value = new_value.wrapping_add(1);
        self.account.set_inner(SimpleAccountInner {
            authority,
            value: second_value,
            bump,
        });

        // Custom(4): second set_inner failed
        if self.account.value != second_value {
            return Err(ProgramError::Custom(4));
        }

        Ok(())
    }
}
