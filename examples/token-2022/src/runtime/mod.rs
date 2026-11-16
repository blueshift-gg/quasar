//! Quasar implementations of the account-facing operations used by Token-2022.
pub use quasar_lang::shared::SharedAccount as AccountView;
pub mod extensions;
pub mod hook;
pub mod proof;
pub mod system;

pub mod account {
    use solana_program_error::ProgramError;
    pub fn next_account_view<T>(iter: &mut impl Iterator<Item = T>) -> Result<T, ProgramError> {
        iter.next().ok_or(ProgramError::NotEnoughAccountKeys)
    }
}

pub mod sysvars {
    pub use quasar_lang::sysvar::SysvarData as Sysvar;
    pub mod clock {
        pub use quasar_lang::sysvar::Clock;
    }
    pub mod rent {
        use super::super::AccountView;
        use quasar_lang::{
            sysvar::{Rent as QuasarRent, SysvarData},
            zeropod::ZeroPod,
            ProgramError, Result,
        };
        pub struct Rent(QuasarRent);
        impl Rent {
            pub fn get() -> Result<Self> {
                QuasarRent::get().map(Self)
            }
            pub fn from_account_view(account: &AccountView) -> Result<Self> {
                if account.address() != &QuasarRent::ID {
                    return Err(ProgramError::InvalidArgument);
                }
                let bytes = account.try_borrow()?;
                if bytes.len() < 17 {
                    return Err(ProgramError::InvalidAccountData);
                }
                // SAFETY: holds validated the complete rent layout.
                Ok(Self(QuasarRent::own(unsafe { QuasarRent::read(&bytes) })))
            }
            pub fn try_minimum_balance(&self, len: usize) -> Result<u64> {
                Ok(self.0.minimum_balance(len))
            }
            pub fn is_exempt(&self, lamports: u64, len: usize) -> bool {
                lamports >= self.0.minimum_balance(len)
            }
        }
    }
}

pub fn invoke(
    instruction: solana_instruction::Instruction,
    accounts: &[&AccountView],
    seeds: &[&[&[u8]]],
) -> quasar_lang::Result {
    let metas: Vec<_> = instruction
        .accounts
        .iter()
        .map(|meta| quasar_lang::shared::AccountMeta {
            address: meta.pubkey,
            is_signer: meta.is_signer,
            is_writable: meta.is_writable,
        })
        .collect();
    quasar_lang::shared::invoke_signed(
        &instruction.program_id,
        &metas,
        &instruction.data,
        accounts,
        seeds,
    )
}

#[cfg(test)]
pub(crate) mod testing;
