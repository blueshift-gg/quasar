//! Builds the Metaplex `CreateMetadataAccountV3` instruction
//! (discriminator 33).

use {
    crate::codec::{BorshCpiEncode, BORSH_PREFIX_LEN},
    quasar_lang::{cpi::CpiDynamic, prelude::*},
};

const CREATE_METADATA_ACCOUNTS_V3: u8 = 33;

#[cold]
#[inline(never)]
fn metadata_field_too_long() -> ProgramError {
    ProgramError::InvalidInstructionData
}

#[inline(always)]
fn borsh_payload_len(field: &impl BorshCpiEncode) -> Result<usize, ProgramError> {
    field
        .encoded_len()
        .checked_sub(BORSH_PREFIX_LEN)
        .ok_or_else(metadata_field_too_long)
}

/// Create a metadata account for an SPL Token mint.
///
/// `name`, `symbol`, and `uri` are bounded by the Metaplex maximums
/// (32 / 10 / 200 bytes); `creators`, `collection`, `uses`, and
/// `collection_details` are always serialized as `None`.
///
/// ### Accounts:
///   0. `[WRITE]` Metadata PDA (initialized)
///   1. `[]`      SPL Token mint
///   2. `[SIGNER]` Mint authority
///   3. `[WRITE, SIGNER]` Payer funding the metadata account
///   4. `[]`      Update authority; becomes `[SIGNER]` when
///      `update_authority_is_signer` is set
///   5. `[]`      System program
///   6. `[]`      Rent sysvar
///
/// ### Instruction data (20 + name + symbol + uri bytes):
/// ```text
/// discriminator          u8 = 33
/// DataV2.name            Borsh string: u32 LE len + UTF-8 bytes
/// DataV2.symbol          Borsh string: u32 LE len + UTF-8 bytes
/// DataV2.uri             Borsh string: u32 LE len + UTF-8 bytes
/// DataV2.seller_fee      u16 LE
/// DataV2.creators        Option<Vec<Creator>> = None (tag 0x00)
/// DataV2.collection      Option<Collection>   = None (tag 0x00)
/// DataV2.uses            Option<Uses>         = None (tag 0x00)
/// is_mutable             bool (1 byte)
/// collection_details     Option<..>           = None (tag 0x00)
/// ```
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_metadata_accounts_v3<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    mint: &'a AccountView,
    mint_authority: &'a AccountView,
    payer: &'a AccountView,
    update_authority: &'a AccountView,
    system_program: &'a AccountView,
    rent: &'a AccountView,
    name: impl BorshCpiEncode,
    symbol: impl BorshCpiEncode,
    uri: impl BorshCpiEncode,
    seller_fee_basis_points: u16,
    is_mutable: bool,
    update_authority_is_signer: bool,
) -> Result<CpiDynamic<'a, 7, 512>, ProgramError> {
    let name_len = borsh_payload_len(&name)?;
    let symbol_len = borsh_payload_len(&symbol)?;
    let uri_len = borsh_payload_len(&uri)?;
    if name_len > super::MAX_NAME_LEN
        || symbol_len > super::MAX_SYMBOL_LEN
        || uri_len > super::MAX_URI_LEN
    {
        return Err(metadata_field_too_long());
    }

    let mut cpi = CpiDynamic::<7, 512>::new(program.address());

    // Push accounts.
    cpi.push_account(metadata, false, true)?;
    cpi.push_account(mint, false, false)?;
    cpi.push_account(mint_authority, true, false)?;
    cpi.push_account(payer, true, true)?;
    cpi.push_account(update_authority, update_authority_is_signer, false)?;
    cpi.push_account(system_program, false, false)?;
    cpi.push_account(rent, false, false)?;

    // Borsh-serialize: discriminator + DataV2 + is_mutable + collection_details
    // DataV2 = name(String) + symbol(String) + uri(String) + seller_fee(u16) +
    // creators(Option<Vec>) + collection(Option) + uses(Option)
    let mut offset = 0;

    // SAFETY: Writing serialized instruction data into the uninitialized buffer.
    // All bytes [0..offset] are written before set_data_len() is called.
    unsafe {
        let ptr = cpi.data_mut() as *mut u8;

        // Discriminator
        core::ptr::write(ptr, CREATE_METADATA_ACCOUNTS_V3);
        offset += 1;

        // DataV2.name, symbol, uri (Borsh strings: u32 LE length + bytes)
        offset = name.write_to(ptr, offset);
        offset = symbol.write_to(ptr, offset);
        offset = uri.write_to(ptr, offset);

        // DataV2.seller_fee_basis_points
        core::ptr::copy_nonoverlapping(
            seller_fee_basis_points.to_le_bytes().as_ptr(),
            ptr.add(offset),
            2,
        );
        offset += 2;

        // DataV2.creators: Option<Vec<Creator>> = None
        core::ptr::write(ptr.add(offset), 0u8);
        offset += 1;

        // DataV2.collection: Option<Collection> = None
        core::ptr::write(ptr.add(offset), 0u8);
        offset += 1;

        // DataV2.uses: Option<Uses> = None
        core::ptr::write(ptr.add(offset), 0u8);
        offset += 1;

        // is_mutable
        core::ptr::write(ptr.add(offset), is_mutable as u8);
        offset += 1;

        // collection_details: Option<CollectionDetails> = None
        core::ptr::write(ptr.add(offset), 0u8);
        offset += 1;
    }

    // SAFETY: The serialization block above initialized every byte in
    // `data[0..offset]`.
    unsafe { cpi.set_data_len(offset) }?;
    Ok(cpi)
}
