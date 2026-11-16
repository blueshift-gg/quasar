//! Account allocation helpers ported from the pinned Token-2022 interface.
use super::AccountView;
use bytemuck::Pod;
use solana_program_error::ProgramError;
use spl_token_2022_interface::{
    error::TokenError,
    extension::{
        set_account_type, BaseState, BaseStateWithExtensions, BaseStateWithExtensionsMut,
        Extension, PodStateWithExtensions, PodStateWithExtensionsMut,
    },
};
use spl_type_length_value::variable_len_pack::VariableLenPack;
const BASE_ACCOUNT_LENGTH: usize = 165;

pub fn alloc_and_serialize<S: BaseState + Pod, V: Default + Extension + Pod>(
    account_info: &mut AccountView,
    new_extension: &V,
    overwrite: bool,
) -> Result<(), ProgramError> {
    let previous_account_len = account_info.data_len();
    let new_account_len = {
        let data = account_info.try_borrow()?;
        let state = PodStateWithExtensions::<S>::unpack(&data)?;
        state.try_get_new_account_len::<V>()?
    };

    // Realloc the account first, if needed
    if new_account_len > previous_account_len {
        account_info.resize(new_account_len)?;
    }
    let mut buffer = account_info.try_borrow_mut()?;
    if previous_account_len <= BASE_ACCOUNT_LENGTH {
        set_account_type::<S>(&mut buffer)?;
    }
    let mut state = PodStateWithExtensionsMut::<S>::unpack(&mut buffer)?;

    // Write the extension
    let extension = state.init_extension::<V>(overwrite)?;
    *extension = *new_extension;

    Ok(())
}

/// Packs a variable-length extension into a TLV space
///
/// This function reallocates the account as needed to accommodate for the
/// change in space, then reallocates in the TLV buffer, and finally writes the
/// bytes.
///
/// NOTE: Unlike the `reallocate` instruction, this function will reduce the
/// size of an account if it has too many bytes allocated for the given value.
pub fn alloc_and_serialize_variable_len_extension<
    S: BaseState + Pod,
    V: Extension + VariableLenPack,
>(
    account_info: &mut AccountView,
    new_extension: &V,
    overwrite: bool,
) -> Result<(), ProgramError> {
    let previous_account_len = account_info.data_len();
    let (new_account_len, extension_already_exists) = {
        let data = account_info.try_borrow()?;
        let state = PodStateWithExtensions::<S>::unpack(&data)?;
        let new_account_len =
            state.try_get_new_account_len_for_variable_len_extension(new_extension)?;
        let extension_already_exists = state.get_extension_bytes::<V>().is_ok();
        (new_account_len, extension_already_exists)
    };

    if extension_already_exists && !overwrite {
        return Err(TokenError::ExtensionAlreadyInitialized.into());
    }

    if previous_account_len < new_account_len {
        // account size increased, so realloc the account, then the TLV entry, then
        // write data
        account_info.resize(new_account_len)?;
        let mut buffer = account_info.try_borrow_mut()?;
        if extension_already_exists {
            let mut state = PodStateWithExtensionsMut::<S>::unpack(&mut buffer)?;
            state.realloc_variable_len_extension(new_extension)?;
        } else {
            if previous_account_len <= BASE_ACCOUNT_LENGTH {
                set_account_type::<S>(&mut buffer)?;
            }
            // now alloc in the TLV buffer and write the data
            let mut state = PodStateWithExtensionsMut::<S>::unpack(&mut buffer)?;
            state.init_variable_len_extension(new_extension, false)?;
        }
    } else {
        // do it backwards otherwise, write the state, realloc TLV, then the account
        let mut buffer = account_info.try_borrow_mut()?;
        let mut state = PodStateWithExtensionsMut::<S>::unpack(&mut buffer)?;
        if extension_already_exists {
            state.realloc_variable_len_extension(new_extension)?;
        } else {
            // this situation can happen if we have an overallocated buffer
            state.init_variable_len_extension(new_extension, false)?;
        }

        let removed_bytes = previous_account_len
            .checked_sub(new_account_len)
            .ok_or(ProgramError::AccountDataTooSmall)?;
        if removed_bytes > 0 {
            // this is probably fine, but be safe and avoid invalidating references
            drop(buffer);
            account_info.resize(new_account_len)?;
        }
    }
    Ok(())
}
