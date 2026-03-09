use quasar_core::borsh::CpiEncode;
use quasar_core::cpi::{BufCpiCall, InstructionAccount};
use quasar_core::prelude::*;

const CREATE_METADATA_ACCOUNTS_V3: u8 = 33;

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub fn create_metadata_accounts_v3<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    mint: &'a AccountView,
    mint_authority: &'a AccountView,
    payer: &'a AccountView,
    update_authority: &'a AccountView,
    system_program: &'a AccountView,
    rent: &'a AccountView,
    name: impl CpiEncode<4>,
    symbol: impl CpiEncode<4>,
    uri: impl CpiEncode<4>,
    seller_fee_basis_points: u16,
    is_mutable: bool,
    update_authority_is_signer: bool,
) -> BufCpiCall<'a, 7, 512> {
    let name_len = name.encoded_len() - 4;
    let symbol_len = symbol.encoded_len() - 4;
    let uri_len = uri.encoded_len() - 4;
    if name_len > super::MAX_NAME_LEN
        || symbol_len > super::MAX_SYMBOL_LEN
        || uri_len > super::MAX_URI_LEN
    {
        metadata_field_panic();
    }

    // Borsh-serialize: discriminator + DataV2 + is_mutable + collection_details
    // DataV2 = name(String) + symbol(String) + uri(String) + seller_fee(u16) + creators(Option<Vec>) + collection(Option) + uses(Option)
    let mut data = [0u8; 512];
    let mut offset = 0;

    unsafe {
        let ptr = data.as_mut_ptr();

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

    BufCpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::readonly(mint.address()),
            InstructionAccount::readonly_signer(mint_authority.address()),
            InstructionAccount::writable_signer(payer.address()),
            if update_authority_is_signer {
                InstructionAccount::readonly_signer(update_authority.address())
            } else {
                InstructionAccount::readonly(update_authority.address())
            },
            InstructionAccount::readonly(system_program.address()),
            InstructionAccount::readonly(&super::RENT_SYSVAR),
        ],
        [
            metadata,
            mint,
            mint_authority,
            payer,
            update_authority,
            system_program,
            rent,
        ],
        data,
        offset,
    )
}

#[cold]
#[inline(never)]
fn metadata_field_panic() -> ! {
    panic!("metadata field lengths exceed Metaplex limits");
}
