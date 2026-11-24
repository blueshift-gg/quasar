//! Proof-context verification using Quasar account borrows.
use super::{account::next_account_view, AccountView};
use bytemuck::Pod;
use solana_address::Address;
use solana_program_error::ProgramError;
use solana_zk_elgamal_proof_interface::{
    instruction::ProofInstruction, proof_data::ZkProofData, state::ProofContextState,
};
use spl_token_confidential_transfer_proof_extraction::instruction::zk_proof_type_to_instruction;
use std::slice::IterMut;

pub fn verify_and_extract_context<T: Pod + ZkProofData<U>, U: Pod>(
    accounts: &mut IterMut<'_, AccountView>,
    offset: i64,
    sysvar: Option<&AccountView>,
) -> Result<U, ProgramError> {
    if offset == 0 {
        let account = next_account_view(accounts)?;
        crate::check_zk_elgamal_proof_program_account(&account.owner())?;
        let data = account.try_borrow()?;
        let state = bytemuck::try_from_bytes::<ProofContextState<U>>(&data)
            .map_err(|_| ProgramError::InvalidArgument)?;
        if state.proof_type != T::PROOF_TYPE.into() {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(state.proof_context)
    } else {
        let sysvar = match sysvar {
            Some(account) => account,
            None => next_account_view(accounts)?,
        };
        if sysvar.address() != &solana_sdk_ids::sysvar::instructions::ID {
            return Err(ProgramError::UnsupportedSysvar);
        }
        let bytes = sysvar.try_borrow()?;
        let (program, data) = relative_instruction(&bytes, offset)?;
        if program != &solana_zk_elgamal_proof_interface::ID
            || ProofInstruction::instruction_type(data)
                != Some(zk_proof_type_to_instruction(T::PROOF_TYPE)?)
        {
            return Err(ProgramError::InvalidInstructionData);
        }
        ProofInstruction::proof_data::<T, U>(data)
            .map(|proof| *proof.context_data())
            .ok_or(ProgramError::InvalidInstructionData)
    }
}

fn u16_at(data: &[u8], at: usize) -> Result<usize, ProgramError> {
    let bytes = data.get(at..at + 2).ok_or(ProgramError::InvalidInstructionData)?;
    Ok(usize::from(u16::from_le_bytes([bytes[0], bytes[1]])))
}

/// Borrow only the proof payload, avoiding an allocation for every transaction instruction.
fn relative_instruction(data: &[u8], offset: i64) -> Result<(&Address, &[u8]), ProgramError> {
    let current =
        u16_at(data, data.len().checked_sub(2).ok_or(ProgramError::InvalidInstructionData)?)?;
    let index = (current as i64).saturating_add(offset);
    let count = u16_at(data, 0)?;
    if index < 0 || index as usize >= count {
        return Err(ProgramError::InvalidArgument);
    }
    let start = u16_at(data, 2 + index as usize * 2)?;
    let accounts = u16_at(data, start)?;
    let program_start = start + 2 + accounts * 33;
    let program =
        data.get(program_start..program_start + 32).ok_or(ProgramError::InvalidInstructionData)?;
    let program =
        bytemuck::try_from_bytes(program).map_err(|_| ProgramError::InvalidInstructionData)?;
    let len = u16_at(data, program_start + 32)?;
    let payload = data
        .get(program_start + 34..program_start + 34 + len)
        .ok_or(ProgramError::InvalidInstructionData)?;
    Ok((program, payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relative_proofs_reject_truncation_and_invalid_indices() {
        for len in 0..70 {
            let bytes = vec![255; len];
            assert!(relative_instruction(&bytes, 0).is_err());
        }
        let mut bytes = vec![1, 0, 4, 0, 0, 0];
        bytes.extend_from_slice(solana_zk_elgamal_proof_interface::ID.as_ref());
        bytes.extend_from_slice(&[2, 0, 7, 8, 0, 0]);
        assert_eq!(
            relative_instruction(&bytes, 0).unwrap(),
            (&solana_zk_elgamal_proof_interface::ID, &[7, 8][..])
        );
        assert!(relative_instruction(&bytes, -1).is_err());
        assert!(relative_instruction(&bytes, 1).is_err());
        assert!(relative_instruction(&bytes, i64::MAX).is_err());
    }
}
