// Adapted from solana-program/token-2022 at e8ca680ca8aeb12f9598524b003c776c6a4168f2.
// Modified to execute on Quasar; see ../README.md for provenance and compatibility.
use {
    crate::runtime::{account::next_account_view, AccountView},
    crate::{
        extension::confidential_mint_burn::processor::process_confidential_burn,
        pod_instruction::{AmountCheckedData, AmountData},
        processor::{BurnInstructionVariant, InstructionVariant, Processor},
    },
    solana_address::Address,
    solana_msg::msg,
    solana_program_error::{ProgramError, ProgramResult},
    spl_token_2022_interface::{
        check_program_account,
        extension::{
            confidential_mint_burn::instruction::BurnInstructionData,
            permissioned_burn::{
                instruction::{InitializeInstructionData, PermissionedBurnInstruction},
                PermissionedBurnConfig,
            },
            BaseStateWithExtensionsMut, PodStateWithExtensionsMut,
        },
        instruction::{decode_instruction_data, decode_instruction_type},
        pod::PodMint,
    },
};

fn process_initialize(
    _program_id: &Address,
    accounts: &mut [AccountView],
    authority: &Address,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter_mut();
    let mint_account_info = next_account_view(account_info_iter)?;
    check_program_account(&mint_account_info.owner())?;

    let mut mint_data = mint_account_info.try_borrow_mut()?;
    let mut mint = PodStateWithExtensionsMut::<PodMint>::unpack_uninitialized(&mut mint_data)?;

    let extension = mint.init_extension::<PermissionedBurnConfig>(true)?;
    extension.authority = Some(*authority).try_into().map_err(|_| ProgramError::InvalidArgument)?;

    Ok(())
}

pub(crate) fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    input: &[u8],
) -> ProgramResult {
    check_program_account(program_id)?;

    match decode_instruction_type(input)? {
        PermissionedBurnInstruction::Initialize => {
            msg!("PermissionedBurnInstruction::Initialize");
            let InitializeInstructionData { authority } = decode_instruction_data(input)?;
            process_initialize(program_id, accounts, authority)
        }
        PermissionedBurnInstruction::Burn => {
            msg!("PermissionedBurnInstruction::Burn");
            let data = decode_instruction_data::<AmountData>(input)?;
            Processor::process_burn(
                program_id,
                accounts,
                data.amount.into(),
                BurnInstructionVariant::Permissioned,
                InstructionVariant::Unchecked,
            )
        }
        PermissionedBurnInstruction::BurnChecked => {
            msg!("PermissionedBurnInstruction::BurnChecked");
            let data = decode_instruction_data::<AmountCheckedData>(input)?;
            Processor::process_burn(
                program_id,
                accounts,
                data.amount.into(),
                BurnInstructionVariant::Permissioned,
                InstructionVariant::Checked { decimals: data.decimals },
            )
        }
        PermissionedBurnInstruction::ConfidentialBurn => {
            msg!("PermissionedBurnInstruction::ConfidentialBurn");
            let data = decode_instruction_data::<BurnInstructionData>(input)?;
            process_confidential_burn(
                program_id,
                accounts,
                data,
                BurnInstructionVariant::Permissioned,
            )
        }
    }
}
