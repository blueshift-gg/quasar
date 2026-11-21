//! The upstream wire dispatcher running on Quasar's account runtime.
use crate::{processor::Processor, ID};

quasar_lang::export_entrypoint! {
    /// # Safety
    /// Both pointers must be the inputs supplied by the SBF runtime (SIMD-0321).
    pub unsafe extern "C" fn entrypoint(input: *mut u8, data: *const u8) -> u64 {
        // SAFETY: the runtime supplied both pointers; this is their sole cursor.
        unsafe {
            quasar_lang::context::entrypoint::<0>(input, data, &ID, |_, bytes, cursor| {
                let mut accounts = cursor.into_shared()?;
                Processor::process(&ID, &mut accounts, bytes).inspect_err(|error| {
                    solana_msg::msg!(error.to_str::<spl_token_2022_interface::error::TokenError>());
                })?;
                for account in &accounts { account.flush()?; }
                Ok(())
            })
        }
    }
}
