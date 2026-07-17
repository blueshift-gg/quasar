//! Builds the Metaplex `UpdateMetadataAccountV2` instruction
//! (discriminator 15).

use {
    crate::codec::CpiEncode,
    quasar_lang::{cpi::CpiDynamic, prelude::*},
};

const UPDATE_METADATA_ACCOUNTS_V2: u8 = 15;

#[cold]
#[inline(never)]
fn metadata_field_too_long() -> ProgramError {
    ProgramError::InvalidInstructionData
}

/// Update mutable fields of an existing metadata account.
///
/// Each argument serializes as a Borsh `Option`: `None` writes a single
/// zero tag byte. The `DataV2` block is emitted only when `name`,
/// `symbol`, and `uri` are all `Some`; if any is missing the entire
/// `DataV2` is written as `None` and the other three are ignored.
///
/// ### Accounts:
///   0. `[WRITE]` Metadata account
///   1. `[SIGNER]` Update authority
///
/// ### Instruction data (dynamic, 5-byte minimum):
/// ```text
/// discriminator                u8 = 15
/// Option<DataV2>               tag 0x01 iff name+symbol+uri all set,
///                              else 0x00
///   name                       Borsh string: u32 LE len + UTF-8 bytes
///   symbol                     Borsh string: u32 LE len + UTF-8 bytes
///   uri                        Borsh string: u32 LE len + UTF-8 bytes
///   seller_fee_basis_points    u16 LE (0 when unset)
///   creators                   Option<..> = None (tag 0x00)
///   collection                 Option<..> = None (tag 0x00)
///   uses                       Option<..> = None (tag 0x00)
/// Option<Pubkey> new_update_authority
///                              0x01 + 32-byte address, else 0x00
/// Option<bool> primary_sale_happened
///                              0x01 + 1-byte bool, else 0x00
/// Option<bool> is_mutable      0x01 + 1-byte bool, else 0x00
/// ```
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(super) fn update_metadata_accounts_v2<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    update_authority: &'a AccountView,
    new_update_authority: Option<&Address>,
    name: Option<&[u8]>,
    symbol: Option<&[u8]>,
    uri: Option<&[u8]>,
    seller_fee_basis_points: Option<u16>,
    primary_sale_happened: Option<bool>,
    is_mutable: Option<bool>,
) -> Result<CpiDynamic<'a, 2, 512>, ProgramError> {
    if let Some(n) = name {
        if n.len() > super::MAX_NAME_LEN {
            return Err(metadata_field_too_long());
        }
    }
    if let Some(s) = symbol {
        if s.len() > super::MAX_SYMBOL_LEN {
            return Err(metadata_field_too_long());
        }
    }
    if let Some(u) = uri {
        if u.len() > super::MAX_URI_LEN {
            return Err(metadata_field_too_long());
        }
    }

    let mut cpi = CpiDynamic::<2, 512>::new(program.address());

    // Push accounts.
    cpi.push_account(metadata, false, true)?;
    cpi.push_account(update_authority, true, false)?;

    let mut offset = 0;

    // SAFETY: Writing serialized instruction data into the uninitialized buffer.
    // All bytes [0..offset] are written before set_data_len() is called.
    unsafe {
        let ptr = cpi.data_mut() as *mut u8;

        core::ptr::write(ptr, UPDATE_METADATA_ACCOUNTS_V2);
        offset += 1;

        // Option<DataV2>
        match (name, symbol, uri) {
            (Some(n), Some(s), Some(u)) => {
                core::ptr::write(ptr.add(offset), 1u8); // Some
                offset += 1;

                offset = CpiEncode::<4>::write_to(&n, ptr, offset);
                offset = CpiEncode::<4>::write_to(&s, ptr, offset);
                offset = CpiEncode::<4>::write_to(&u, ptr, offset);

                // seller_fee_basis_points
                let fee = seller_fee_basis_points.unwrap_or(0);
                core::ptr::copy_nonoverlapping(fee.to_le_bytes().as_ptr(), ptr.add(offset), 2);
                offset += 2;

                // creators: None, collection: None, uses: None
                core::ptr::write(ptr.add(offset), 0u8);
                offset += 1;
                core::ptr::write(ptr.add(offset), 0u8);
                offset += 1;
                core::ptr::write(ptr.add(offset), 0u8);
                offset += 1;
            }
            _ => {
                core::ptr::write(ptr.add(offset), 0u8); // None
                offset += 1;
            }
        }

        // new_update_authority: Option<Pubkey>
        match new_update_authority {
            Some(addr) => {
                core::ptr::write(ptr.add(offset), 1u8);
                offset += 1;
                core::ptr::copy_nonoverlapping(addr.as_ref().as_ptr(), ptr.add(offset), 32);
                offset += 32;
            }
            None => {
                core::ptr::write(ptr.add(offset), 0u8);
                offset += 1;
            }
        }

        // primary_sale_happened: Option<bool>
        match primary_sale_happened {
            Some(v) => {
                core::ptr::write(ptr.add(offset), 1u8);
                offset += 1;
                core::ptr::write(ptr.add(offset), v as u8);
                offset += 1;
            }
            None => {
                core::ptr::write(ptr.add(offset), 0u8);
                offset += 1;
            }
        }

        // is_mutable: Option<bool>
        match is_mutable {
            Some(v) => {
                core::ptr::write(ptr.add(offset), 1u8);
                offset += 1;
                core::ptr::write(ptr.add(offset), v as u8);
                offset += 1;
            }
            None => {
                core::ptr::write(ptr.add(offset), 0u8);
                offset += 1;
            }
        }
    }

    // SAFETY: The serialization block above initialized every byte in
    // `data[0..offset]`.
    unsafe { cpi.set_data_len(offset) }?;
    Ok(cpi)
}
