//! System instructions invoked through Quasar.
use super::{invoke, AccountView};
use solana_address::Address;
use solana_program_error::ProgramResult;

pub struct Transfer<'a> {
    pub from: &'a AccountView,
    pub to: &'a AccountView,
    pub lamports: u64,
}
impl Transfer<'_> {
    pub fn invoke(&self) -> ProgramResult {
        invoke(
            solana_system_interface::instruction::transfer(
                self.from.address(),
                self.to.address(),
                self.lamports,
            ),
            &[self.from, self.to],
            &[],
        )
    }
}

pub struct Funding<'a> {
    pub from: &'a AccountView,
    pub lamports: u64,
}
pub struct CreateAccountAllowPrefund<'a> {
    pub funding: Option<Funding<'a>>,
    pub to: &'a AccountView,
    pub space: u64,
    pub owner: &'a Address,
}
impl CreateAccountAllowPrefund<'_> {
    pub fn invoke_signed(&self, seeds: &[&[&[u8]]]) -> ProgramResult {
        // Agave 3.1 predates system instruction 13; the signed operations are atomic
        // with the outer instruction and preserve an already-funded PDA's lamports.
        if let Some(funding) = &self.funding {
            invoke(
                solana_system_interface::instruction::transfer(
                    funding.from.address(),
                    self.to.address(),
                    funding.lamports,
                ),
                &[funding.from, self.to],
                seeds,
            )?;
        }
        invoke(
            solana_system_interface::instruction::allocate(self.to.address(), self.space),
            &[self.to],
            seeds,
        )?;
        invoke(
            solana_system_interface::instruction::assign(self.to.address(), self.owner),
            &[self.to],
            seeds,
        )
    }
}
