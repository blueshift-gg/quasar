use quasar_core::borsh::BorshString;
use quasar_core::cpi::{BufCpiCall, InstructionAccount};
use quasar_core::prelude::*;

const UPDATE_METADATA_ACCOUNTS_V2: u8 = 15;

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub fn update_metadata_accounts_v2<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    update_authority: &'a AccountView,
    new_update_authority: Option<&Address>,
    name: Option<BorshString<'_>>,
    symbol: Option<BorshString<'_>>,
    uri: Option<BorshString<'_>>,
    seller_fee_basis_points: Option<u16>,
    primary_sale_happened: Option<bool>,
    is_mutable: Option<bool>,
) -> BufCpiCall<'a, 2, 512> {
    if let Some(ref n) = name {
        if n.0.len() > super::MAX_NAME_LEN {
            metadata_field_panic();
        }
    }
    if let Some(ref s) = symbol {
        if s.0.len() > super::MAX_SYMBOL_LEN {
            metadata_field_panic();
        }
    }
    if let Some(ref u) = uri {
        if u.0.len() > super::MAX_URI_LEN {
            metadata_field_panic();
        }
    }

    let mut data = [0u8; 512];
    let mut offset = 0;

    unsafe {
        let ptr = data.as_mut_ptr();

        core::ptr::write(ptr, UPDATE_METADATA_ACCOUNTS_V2);
        offset += 1;

        // Option<DataV2>
        match (name, symbol, uri) {
            (Some(n), Some(s), Some(u)) => {
                core::ptr::write(ptr.add(offset), 1u8); // Some
                offset += 1;

                offset = n.write_to(ptr, offset);
                offset = s.write_to(ptr, offset);
                offset = u.write_to(ptr, offset);

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

    BufCpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::readonly_signer(update_authority.address()),
        ],
        [metadata, update_authority],
        data,
        offset,
    )
}

#[cold]
#[inline(never)]
fn metadata_field_panic() -> ! {
    panic!("metadata field lengths exceed Metaplex limits");
}
