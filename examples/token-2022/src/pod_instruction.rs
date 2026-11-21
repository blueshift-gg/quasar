// Adapted from solana-program/token-2022 at e8ca680ca8aeb12f9598524b003c776c6a4168f2.
// Modified to execute on Quasar; see ../README.md for provenance and compatibility.
//! Rewrites of the instruction data types represented as Pods

use {
    bytemuck::{Pod, Zeroable},
    core::mem::size_of,
    num_enum::{IntoPrimitive, TryFromPrimitive},
    solana_address::{Address, ADDRESS_BYTES},
    solana_program_error::ProgramError,
    solana_zero_copy::unaligned::U64,
    spl_token_2022_interface::pod::PodCOption,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct InitializeMintData {
    /// Number of base 10 digits to the right of the decimal place.
    pub(crate) decimals: u8,
    /// The authority/multisignature to mint tokens.
    pub(crate) mint_authority: Address,
    // The freeze authority option comes later, but cannot be included as
    // plain old data in this struct
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct InitializeMultisigData {
    /// The number of signers (M) required to validate this multisignature
    /// account.
    pub(crate) m: u8,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct AmountData {
    /// The amount of tokens to transfer.
    pub(crate) amount: U64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct AmountCheckedData {
    /// The amount of tokens to transfer.
    pub(crate) amount: U64,
    /// Decimals of the mint
    pub(crate) decimals: u8,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct SetAuthorityData {
    /// The type of authority to update.
    pub(crate) authority_type: u8,
    // The new authority option comes later, but cannot be included as
    // plain old data in this struct
}

/// All of the base instructions in Token-2022, reduced down to their one-byte
/// discriminant.
///
/// All instructions that expect data afterwards include a comment with the data
/// type expected. For example, `PodTokenInstruction::InitializeMint` expects
/// `InitializeMintData`.
#[derive(Clone, Copy, Debug, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub(crate) enum PodTokenInstruction {
    // 0
    InitializeMint, // InitializeMintData
    InitializeAccount,
    InitializeMultisig, // InitializeMultisigData
    Transfer,           // AmountData
    Approve,            // AmountData
    // 5
    Revoke,
    SetAuthority, // SetAuthorityData
    MintTo,       // AmountData
    Burn,         // AmountData
    CloseAccount,
    // 10
    FreezeAccount,
    ThawAccount,
    TransferChecked, // AmountCheckedData
    ApproveChecked,  // AmountCheckedData
    MintToChecked,   // AmountCheckedData
    // 15
    BurnChecked,        // AmountCheckedData
    InitializeAccount2, // Address
    SyncNative,
    InitializeAccount3,  // Address
    InitializeMultisig2, // InitializeMultisigData
    // 20
    InitializeMint2,    // InitializeMintData
    GetAccountDataSize, // &[ExtensionType]
    InitializeImmutableOwner,
    AmountToUiAmount, // AmountData
    UiAmountToAmount, // &str
    // 25
    InitializeMintCloseAuthority, // COption<Address>
    TransferFeeExtension,
    ConfidentialTransferExtension,
    DefaultAccountStateExtension,
    Reallocate, // &[ExtensionType]
    // 30
    MemoTransferExtension,
    CreateNativeMint,
    InitializeNonTransferableMint,
    InterestBearingMintExtension,
    CpiGuardExtension,
    // 35
    InitializePermanentDelegate, // Address
    TransferHookExtension,
    ConfidentialTransferFeeExtension,
    WithdrawExcessLamports,
    MetadataPointerExtension,
    // 40
    GroupPointerExtension,
    GroupMemberPointerExtension,
    ConfidentialMintBurnExtension,
    ScaledUiAmountExtension,
    PausableExtension,
    // 45
    UnwrapLamports,
    PermissionedBurnExtension,
    // 255
    Batch = 255,
}

fn unpack_pubkey_option(input: &[u8]) -> Result<PodCOption<Address>, ProgramError> {
    match input.split_first() {
        Option::Some((&0, _)) => Ok(PodCOption::none()),
        Option::Some((&1, rest)) => {
            let pk = rest
                .get(..ADDRESS_BYTES)
                .and_then(|x| Address::try_from(x).ok())
                .ok_or(ProgramError::InvalidInstructionData)?;
            Ok(PodCOption::some(pk))
        }
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

const U64_BYTES: usize = 8;
fn unpack_u64_option(input: &[u8]) -> Result<PodCOption<u64>, ProgramError> {
    match input.split_first() {
        Option::Some((&0, _)) => Ok(PodCOption::none()),
        Option::Some((&1, rest)) => {
            let amount = rest
                .get(..U64_BYTES)
                .and_then(|x| x.try_into().map(u64::from_le_bytes).ok())
                .ok_or(ProgramError::InvalidInstructionData)?;
            Ok(PodCOption::some(amount))
        }
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

/// Specialty function for deserializing `Pod` data and a `COption<Address>`
///
/// `COption<T>` is not `Pod` compatible when serialized in an instruction, but
/// since it is always at the end of an instruction, so we can do this safely
pub(crate) fn decode_instruction_data_with_coption_pubkey<T: Pod>(
    input_with_type: &[u8],
) -> Result<(&T, PodCOption<Address>), ProgramError> {
    let end_of_t = size_of::<T>().saturating_add(1);
    let value =
        input_with_type.get(1..end_of_t).ok_or(ProgramError::InvalidInstructionData).and_then(
            |bytes| bytemuck::try_from_bytes(bytes).map_err(|_| ProgramError::InvalidArgument),
        )?;
    let pubkey = unpack_pubkey_option(&input_with_type[end_of_t..])?;
    Ok((value, pubkey))
}

/// Specialty function for deserializing `Pod` data and a `COption<u64>`
///
/// `COption<T>` is not `Pod` compatible when serialized in an instruction, but
/// since it is always at the end of an instruction, so we can do this safely
pub(crate) fn decode_instruction_data_with_coption_u64<T: Pod>(
    input_with_type: &[u8],
) -> Result<(&T, PodCOption<u64>), ProgramError> {
    let end_of_t = size_of::<T>().saturating_add(1);
    let value =
        input_with_type.get(1..end_of_t).ok_or(ProgramError::InvalidInstructionData).and_then(
            |bytes| bytemuck::try_from_bytes(bytes).map_err(|_| ProgramError::InvalidArgument),
        )?;
    let amount = unpack_u64_option(&input_with_type[end_of_t..])?;
    Ok((value, amount))
}
