//! Transfer-hook resolution with native Quasar account data and CPI.
use super::{invoke, AccountView};
use solana_address::Address;
use solana_instruction::AccountMeta;
use solana_program_error::ProgramResult;
use spl_tlv_account_resolution::{error::AccountResolutionError, state::ExtraAccountMetaList};
use spl_transfer_hook_interface::{
    get_extra_account_metas_address,
    instruction::{self, ExecuteInstruction},
};
use spl_type_length_value::state::TlvStateBorrowed;

pub struct Execute<'a> {
    pub program_id: &'a Address,
    pub source: &'a AccountView,
    pub mint: &'a AccountView,
    pub destination: &'a AccountView,
    pub authority: &'a AccountView,
    pub additional_accounts: &'a [AccountView],
    pub amount: u64,
}
impl Execute<'_> {
    pub fn invoke(&self) -> ProgramResult {
        let mut instruction = instruction::execute(
            self.program_id,
            self.source.address(),
            self.mint.address(),
            self.destination.address(),
            self.authority.address(),
            self.amount,
        );
        let validation = get_extra_account_metas_address(self.mint.address(), self.program_id);
        let mut infos = vec![self.source, self.mint, self.destination, self.authority];
        if let Some(validation) =
            self.additional_accounts.iter().find(|account| account.address() == &validation)
        {
            instruction.accounts.push(AccountMeta::new_readonly(*validation.address(), false));
            infos.push(validation);
            let bytes = validation.try_borrow()?;
            let state = TlvStateBorrowed::unpack(&bytes)?;
            let extras = ExtraAccountMetaList::unpack_with_tlv_state::<ExecuteInstruction>(&state)?;
            let mut borrowed = Vec::with_capacity(infos.len() + extras.len());
            for account in &infos {
                borrowed.push((*account, account.try_borrow()?));
            }
            for extra in extras.iter() {
                let mut meta = extra.resolve(&instruction.data, self.program_id, |index| {
                    borrowed
                        .get(index)
                        .map(|(account, bytes)| (account.address(), Some(&bytes[..])))
                })?;
                // Hook extras never inherit signer privileges, including aliases.
                meta.is_signer = false;
                if let Some(writable) = instruction
                    .accounts
                    .iter()
                    .filter(|old| old.pubkey == meta.pubkey)
                    .map(|old| old.is_writable)
                    .reduce(|a, b| a || b)
                {
                    meta.is_writable &= writable;
                }
                let account = self
                    .additional_accounts
                    .iter()
                    .find(|account| account.address() == &meta.pubkey)
                    .ok_or(AccountResolutionError::IncorrectAccount)?;
                instruction.accounts.push(meta);
                borrowed.push((account, account.try_borrow()?));
                infos.push(account);
            }
        }
        invoke(instruction, &infos, &[])
    }
}
