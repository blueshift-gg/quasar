//! A test hook checks CPI privileges and the transferring flags, then records the amount.
use quasar_lang::{Address, ProgramError};
const ID: Address = Address::new_from_array([31; 32]);
quasar_lang::export_entrypoint! {
    /// # Safety
    /// Input and instruction data are supplied by the SBF runtime.
    pub unsafe extern "C" fn entrypoint(input: *mut u8, data: *const u8) -> u64 {
        // SAFETY: this is the sole cursor for the runtime's input.
        unsafe { quasar_lang::context::entrypoint::<0>(input, data, &ID, |_, data, cursor| {
            let mut accounts = cursor.into_shared()?;
            if data.len() != 16 || accounts.len() != 6 { return Err(ProgramError::InvalidInstructionData); }
            for account in &accounts[..5] {
                if account.is_signer() || account.is_writable() { return Err(ProgramError::Custom(700)); }
            }
            // These test token accounts contain only the one-byte TransferHookAccount extension.
            for index in [0,2] {
                if accounts[index].try_borrow()?.last() != Some(&1) { return Err(ProgramError::Custom(700)); }
            }
            if accounts[5].is_signer() || !accounts[5].is_writable() { return Err(ProgramError::Custom(700)); }
            let amount = u64::from_le_bytes(data[8..].try_into().map_err(|_| ProgramError::InvalidInstructionData)?);
            if amount == 13 { return Err(ProgramError::Custom(701)); }
            accounts[5].try_borrow_mut()?.copy_from_slice(&amount.to_le_bytes());
            for account in &accounts { account.flush()?; }
            Ok(())
        }) }
    }
}
