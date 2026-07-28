use quasar_lang::client::{DynString, DynVec};
use solana_address::Address;
pub mod ping;
pub mod log;
pub mod create_account;
pub mod transfer;
pub mod unchecked_accounts;
pub mod accounts;
pub mod create_pda_static;
pub mod verify_pda_static;
pub mod create_pda_dynamic;
pub mod verify_pda_dynamic;
pub mod dynamic_readback;
pub mod dynamic_mutate;
pub mod dynamic_view_mut;
pub mod two_dyn;
pub mod dyn_bytes_check;
pub mod dyn_str_check;
pub mod create_account_cpi;
pub mod close_account;
pub mod realloc_check;
pub mod assign_test;
pub mod space_override;
pub mod init_mint;
pub mod init_token_account;
pub mod init_ata;
pub mod transfer_checked;
pub mod mint_to;
pub mod burn;
pub mod approve;
pub mod revoke;
pub mod close_token_account;
pub mod sweep_and_close;
pub mod init_mint_t22;
pub mod transfer_checked_interface;
pub mod header_nodup_signer;
pub mod header_dup_signer;
pub mod two_accounts_check;
pub mod has_one_default;
pub mod owner_check;
pub mod program_check;
pub mod require_eq_check;
pub mod remaining_typed_check;
pub mod emit_empty_event;
pub mod emit_u64_event;
pub mod emit_multi_field;
pub mod emit_large_event;
pub mod emit_two_events;
pub mod emit_via_cpi;
pub mod heap_vec_ok;
pub mod read_clock;
pub mod read_clock_from_account;
pub mod read_rent;
pub mod read_rent_calc;
pub mod return_u64;
pub mod cpi_invoke_with_return;
pub mod cpi_invoke_ignore_return;
pub mod cpi_mut_readback;

pub use ping::*;
pub use log::*;
pub use create_account::*;
pub use transfer::*;
pub use unchecked_accounts::*;
pub use accounts::*;
pub use create_pda_static::*;
pub use verify_pda_static::*;
pub use create_pda_dynamic::*;
pub use verify_pda_dynamic::*;
pub use dynamic_readback::*;
pub use dynamic_mutate::*;
pub use dynamic_view_mut::*;
pub use two_dyn::*;
pub use dyn_bytes_check::*;
pub use dyn_str_check::*;
pub use create_account_cpi::*;
pub use close_account::*;
pub use realloc_check::*;
pub use assign_test::*;
pub use space_override::*;
pub use init_mint::*;
pub use init_token_account::*;
pub use init_ata::*;
pub use transfer_checked::*;
pub use mint_to::*;
pub use burn::*;
pub use approve::*;
pub use revoke::*;
pub use close_token_account::*;
pub use sweep_and_close::*;
pub use init_mint_t22::*;
pub use transfer_checked_interface::*;
pub use header_nodup_signer::*;
pub use header_dup_signer::*;
pub use two_accounts_check::*;
pub use has_one_default::*;
pub use owner_check::*;
pub use program_check::*;
pub use require_eq_check::*;
pub use remaining_typed_check::*;
pub use emit_empty_event::*;
pub use emit_u64_event::*;
pub use emit_multi_field::*;
pub use emit_large_event::*;
pub use emit_two_events::*;
pub use emit_via_cpi::*;
pub use heap_vec_ok::*;
pub use read_clock::*;
pub use read_clock_from_account::*;
pub use read_rent::*;
pub use read_rent_calc::*;
pub use return_u64::*;
pub use cpi_invoke_with_return::*;
pub use cpi_invoke_ignore_return::*;
pub use cpi_mut_readback::*;

pub enum ProgramInstruction {
    Ping,
    Log,
    CreateAccount,
    Transfer { amount: u64 },
    UncheckedAccounts,
    Accounts,
    CreatePdaStatic,
    VerifyPdaStatic,
    CreatePdaDynamic,
    VerifyPdaDynamic,
    DynamicReadback { expected_name_len: u8, expected_tags_count: u8 },
    DynamicMutate { new_name: DynString<u8> },
    DynamicViewMut { new_name: DynString<u8>, new_tags: DynVec<Address, u16> },
    TwoDyn { tag: u64, a: DynString<u8>, b: DynString<u8> },
    DynBytesCheck { expected_len: u8 },
    DynStrCheck { expected_len: u8 },
    CreateAccountCpi { lamports: u64, space: u64, owner: Address },
    CloseAccount,
    ReallocCheck { new_space: u64 },
    AssignTest { owner: Address },
    SpaceOverride { value: u64 },
    InitMint,
    InitTokenAccount,
    InitAta,
    TransferChecked { amount: u64, decimals: u8 },
    MintTo { amount: u64 },
    Burn { amount: u64 },
    Approve { amount: u64 },
    Revoke,
    CloseTokenAccount,
    SweepAndClose,
    InitMintT22,
    TransferCheckedInterface { amount: u64, decimals: u8 },
    HeaderNodupSigner,
    HeaderDupSigner,
    TwoAccountsCheck,
    HasOneDefault,
    OwnerCheck,
    ProgramCheck,
    RequireEqCheck { a: u64, b: u64 },
    RemainingTypedCheck,
    EmitEmptyEvent,
    EmitU64Event { value: u64 },
    EmitMultiField { a: u64, b: u64, c: Address },
    EmitLargeEvent { a: u64, b: u64, c: u64, d: u64, e: Address, f: Address, g: u128, h: u128 },
    EmitTwoEvents { first: u64, second: u64 },
    EmitViaCpi { value: u64 },
    HeapVecOk,
    ReadClock,
    ReadClockFromAccount,
    ReadRent,
    ReadRentCalc { data_len: u64 },
    ReturnU64,
    CpiInvokeWithReturn,
    CpiInvokeIgnoreReturn,
    CpiMutReadback { new_value: u64 },
}

fn quasar_take<'a>(data: &'a [u8], offset: &mut usize, len: usize) -> Option<&'a [u8]> {
    let end = offset.checked_add(len)?;
    let bytes = data.get(*offset..end)?;
    *offset = end;
    Some(bytes)
}

fn quasar_read_len(data: &[u8], offset: &mut usize, width: usize) -> Option<usize> {
    let mut buf = [0u8; 8];
    buf.get_mut(..width)?.copy_from_slice(quasar_take(data, offset, width)?);
    usize::try_from(u64::from_le_bytes(buf)).ok()
}

pub fn decode_instruction(data: &[u8]) -> Option<ProgramInstruction> {
    let disc = *data.first()?;
    match disc {
        0 => if data.len() == 1 { Some(ProgramInstruction::Ping) } else { None },
        1 => if data.len() == 1 { Some(ProgramInstruction::Log) } else { None },
        2 => if data.len() == 1 { Some(ProgramInstruction::CreateAccount) } else { None },
        3 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let amount: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let amount_size = usize::try_from(wincode::serialized_size(&amount).ok()?).ok()?;
            quasar_take(payload, &mut offset, amount_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::Transfer { amount })
        }
        4 => if data.len() == 1 { Some(ProgramInstruction::UncheckedAccounts) } else { None },
        5 => if data.len() == 1 { Some(ProgramInstruction::Accounts) } else { None },
        6 => if data.len() == 1 { Some(ProgramInstruction::CreatePdaStatic) } else { None },
        7 => if data.len() == 1 { Some(ProgramInstruction::VerifyPdaStatic) } else { None },
        8 => if data.len() == 1 { Some(ProgramInstruction::CreatePdaDynamic) } else { None },
        9 => if data.len() == 1 { Some(ProgramInstruction::VerifyPdaDynamic) } else { None },
        22 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let expected_name_len: u8 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let expected_name_len_size = usize::try_from(wincode::serialized_size(&expected_name_len).ok()?).ok()?;
            quasar_take(payload, &mut offset, expected_name_len_size)?;
            let expected_tags_count: u8 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let expected_tags_count_size = usize::try_from(wincode::serialized_size(&expected_tags_count).ok()?).ok()?;
            quasar_take(payload, &mut offset, expected_tags_count_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::DynamicReadback { expected_name_len, expected_tags_count })
        }
        23 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let new_name_len = quasar_read_len(payload, &mut offset, 1)?;
            let new_name_bytes = quasar_take(payload, &mut offset, new_name_len)?;
            let new_name: DynString<u8> = core::str::from_utf8(new_name_bytes).ok()?.into();
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::DynamicMutate { new_name })
        }
        24 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let new_name_len = quasar_read_len(payload, &mut offset, 1)?;
            let new_tags_len = quasar_read_len(payload, &mut offset, 2)?;
            let new_name_bytes = quasar_take(payload, &mut offset, new_name_len)?;
            let new_name: DynString<u8> = core::str::from_utf8(new_name_bytes).ok()?.into();
            let new_tags: DynVec<Address, u16> = {
                if new_tags_len > 10 * 1024 * 1024 { return None; }
                let mut items = Vec::with_capacity(new_tags_len.min(4096));
                for _ in 0..new_tags_len {
                    let item: Address = wincode::deserialize(payload.get(offset..)?).ok()?;
                    let item_size = usize::try_from(wincode::serialized_size(&item).ok()?).ok()?;
                    quasar_take(payload, &mut offset, item_size)?;
                    items.push(item);
                }
                items.into()
            };
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::DynamicViewMut { new_name, new_tags })
        }
        25 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let tag: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let tag_size = usize::try_from(wincode::serialized_size(&tag).ok()?).ok()?;
            quasar_take(payload, &mut offset, tag_size)?;
            let a_len = quasar_read_len(payload, &mut offset, 1)?;
            let b_len = quasar_read_len(payload, &mut offset, 1)?;
            let a_bytes = quasar_take(payload, &mut offset, a_len)?;
            let a: DynString<u8> = core::str::from_utf8(a_bytes).ok()?.into();
            let b_bytes = quasar_take(payload, &mut offset, b_len)?;
            let b: DynString<u8> = core::str::from_utf8(b_bytes).ok()?.into();
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::TwoDyn { tag, a, b })
        }
        26 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let expected_len: u8 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let expected_len_size = usize::try_from(wincode::serialized_size(&expected_len).ok()?).ok()?;
            quasar_take(payload, &mut offset, expected_len_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::DynBytesCheck { expected_len })
        }
        27 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let expected_len: u8 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let expected_len_size = usize::try_from(wincode::serialized_size(&expected_len).ok()?).ok()?;
            quasar_take(payload, &mut offset, expected_len_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::DynStrCheck { expected_len })
        }
        28 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let lamports: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let lamports_size = usize::try_from(wincode::serialized_size(&lamports).ok()?).ok()?;
            quasar_take(payload, &mut offset, lamports_size)?;
            let space: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let space_size = usize::try_from(wincode::serialized_size(&space).ok()?).ok()?;
            quasar_take(payload, &mut offset, space_size)?;
            let owner: Address = wincode::deserialize(payload.get(offset..)?).ok()?;
            let owner_size = usize::try_from(wincode::serialized_size(&owner).ok()?).ok()?;
            quasar_take(payload, &mut offset, owner_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::CreateAccountCpi { lamports, space, owner })
        }
        29 => if data.len() == 1 { Some(ProgramInstruction::CloseAccount) } else { None },
        30 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let new_space: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let new_space_size = usize::try_from(wincode::serialized_size(&new_space).ok()?).ok()?;
            quasar_take(payload, &mut offset, new_space_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::ReallocCheck { new_space })
        }
        32 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let owner: Address = wincode::deserialize(payload.get(offset..)?).ok()?;
            let owner_size = usize::try_from(wincode::serialized_size(&owner).ok()?).ok()?;
            quasar_take(payload, &mut offset, owner_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::AssignTest { owner })
        }
        33 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let value: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let value_size = usize::try_from(wincode::serialized_size(&value).ok()?).ok()?;
            quasar_take(payload, &mut offset, value_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::SpaceOverride { value })
        }
        40 => if data.len() == 1 { Some(ProgramInstruction::InitMint) } else { None },
        41 => if data.len() == 1 { Some(ProgramInstruction::InitTokenAccount) } else { None },
        42 => if data.len() == 1 { Some(ProgramInstruction::InitAta) } else { None },
        43 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let amount: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let amount_size = usize::try_from(wincode::serialized_size(&amount).ok()?).ok()?;
            quasar_take(payload, &mut offset, amount_size)?;
            let decimals: u8 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let decimals_size = usize::try_from(wincode::serialized_size(&decimals).ok()?).ok()?;
            quasar_take(payload, &mut offset, decimals_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::TransferChecked { amount, decimals })
        }
        44 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let amount: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let amount_size = usize::try_from(wincode::serialized_size(&amount).ok()?).ok()?;
            quasar_take(payload, &mut offset, amount_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::MintTo { amount })
        }
        45 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let amount: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let amount_size = usize::try_from(wincode::serialized_size(&amount).ok()?).ok()?;
            quasar_take(payload, &mut offset, amount_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::Burn { amount })
        }
        46 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let amount: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let amount_size = usize::try_from(wincode::serialized_size(&amount).ok()?).ok()?;
            quasar_take(payload, &mut offset, amount_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::Approve { amount })
        }
        47 => if data.len() == 1 { Some(ProgramInstruction::Revoke) } else { None },
        48 => if data.len() == 1 { Some(ProgramInstruction::CloseTokenAccount) } else { None },
        49 => if data.len() == 1 { Some(ProgramInstruction::SweepAndClose) } else { None },
        50 => if data.len() == 1 { Some(ProgramInstruction::InitMintT22) } else { None },
        51 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let amount: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let amount_size = usize::try_from(wincode::serialized_size(&amount).ok()?).ok()?;
            quasar_take(payload, &mut offset, amount_size)?;
            let decimals: u8 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let decimals_size = usize::try_from(wincode::serialized_size(&decimals).ok()?).ok()?;
            quasar_take(payload, &mut offset, decimals_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::TransferCheckedInterface { amount, decimals })
        }
        81 => if data.len() == 1 { Some(ProgramInstruction::HeaderNodupSigner) } else { None },
        83 => if data.len() == 1 { Some(ProgramInstruction::HeaderDupSigner) } else { None },
        85 => if data.len() == 1 { Some(ProgramInstruction::TwoAccountsCheck) } else { None },
        87 => if data.len() == 1 { Some(ProgramInstruction::HasOneDefault) } else { None },
        88 => if data.len() == 1 { Some(ProgramInstruction::OwnerCheck) } else { None },
        89 => if data.len() == 1 { Some(ProgramInstruction::ProgramCheck) } else { None },
        91 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let a: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let a_size = usize::try_from(wincode::serialized_size(&a).ok()?).ok()?;
            quasar_take(payload, &mut offset, a_size)?;
            let b: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let b_size = usize::try_from(wincode::serialized_size(&b).ok()?).ok()?;
            quasar_take(payload, &mut offset, b_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::RequireEqCheck { a, b })
        }
        92 => if data.len() == 1 { Some(ProgramInstruction::RemainingTypedCheck) } else { None },
        93 => if data.len() == 1 { Some(ProgramInstruction::EmitEmptyEvent) } else { None },
        94 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let value: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let value_size = usize::try_from(wincode::serialized_size(&value).ok()?).ok()?;
            quasar_take(payload, &mut offset, value_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::EmitU64Event { value })
        }
        95 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let a: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let a_size = usize::try_from(wincode::serialized_size(&a).ok()?).ok()?;
            quasar_take(payload, &mut offset, a_size)?;
            let b: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let b_size = usize::try_from(wincode::serialized_size(&b).ok()?).ok()?;
            quasar_take(payload, &mut offset, b_size)?;
            let c: Address = wincode::deserialize(payload.get(offset..)?).ok()?;
            let c_size = usize::try_from(wincode::serialized_size(&c).ok()?).ok()?;
            quasar_take(payload, &mut offset, c_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::EmitMultiField { a, b, c })
        }
        96 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let a: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let a_size = usize::try_from(wincode::serialized_size(&a).ok()?).ok()?;
            quasar_take(payload, &mut offset, a_size)?;
            let b: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let b_size = usize::try_from(wincode::serialized_size(&b).ok()?).ok()?;
            quasar_take(payload, &mut offset, b_size)?;
            let c: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let c_size = usize::try_from(wincode::serialized_size(&c).ok()?).ok()?;
            quasar_take(payload, &mut offset, c_size)?;
            let d: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let d_size = usize::try_from(wincode::serialized_size(&d).ok()?).ok()?;
            quasar_take(payload, &mut offset, d_size)?;
            let e: Address = wincode::deserialize(payload.get(offset..)?).ok()?;
            let e_size = usize::try_from(wincode::serialized_size(&e).ok()?).ok()?;
            quasar_take(payload, &mut offset, e_size)?;
            let f: Address = wincode::deserialize(payload.get(offset..)?).ok()?;
            let f_size = usize::try_from(wincode::serialized_size(&f).ok()?).ok()?;
            quasar_take(payload, &mut offset, f_size)?;
            let g: u128 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let g_size = usize::try_from(wincode::serialized_size(&g).ok()?).ok()?;
            quasar_take(payload, &mut offset, g_size)?;
            let h: u128 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let h_size = usize::try_from(wincode::serialized_size(&h).ok()?).ok()?;
            quasar_take(payload, &mut offset, h_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::EmitLargeEvent { a, b, c, d, e, f, g, h })
        }
        97 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let first: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let first_size = usize::try_from(wincode::serialized_size(&first).ok()?).ok()?;
            quasar_take(payload, &mut offset, first_size)?;
            let second: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let second_size = usize::try_from(wincode::serialized_size(&second).ok()?).ok()?;
            quasar_take(payload, &mut offset, second_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::EmitTwoEvents { first, second })
        }
        98 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let value: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let value_size = usize::try_from(wincode::serialized_size(&value).ok()?).ok()?;
            quasar_take(payload, &mut offset, value_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::EmitViaCpi { value })
        }
        100 => if data.len() == 1 { Some(ProgramInstruction::HeapVecOk) } else { None },
        101 => if data.len() == 1 { Some(ProgramInstruction::ReadClock) } else { None },
        102 => if data.len() == 1 { Some(ProgramInstruction::ReadClockFromAccount) } else { None },
        103 => if data.len() == 1 { Some(ProgramInstruction::ReadRent) } else { None },
        104 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let data_len: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let data_len_size = usize::try_from(wincode::serialized_size(&data_len).ok()?).ok()?;
            quasar_take(payload, &mut offset, data_len_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::ReadRentCalc { data_len })
        }
        105 => if data.len() == 1 { Some(ProgramInstruction::ReturnU64) } else { None },
        106 => if data.len() == 1 { Some(ProgramInstruction::CpiInvokeWithReturn) } else { None },
        107 => if data.len() == 1 { Some(ProgramInstruction::CpiInvokeIgnoreReturn) } else { None },
        108 => {
            let payload = &data[1..];
            let mut offset = 0usize;
            let new_value: u64 = wincode::deserialize(payload.get(offset..)?).ok()?;
            let new_value_size = usize::try_from(wincode::serialized_size(&new_value).ok()?).ok()?;
            quasar_take(payload, &mut offset, new_value_size)?;
            if offset != payload.len() { return None; }
            Some(ProgramInstruction::CpiMutReadback { new_value })
        }
        _ => None,
    }
}
