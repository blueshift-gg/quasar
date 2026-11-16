// Adapted from solana-program/token-2022 at e8ca680ca8aeb12f9598524b003c776c6a4168f2.
// Modified to execute on Quasar; see ../README.md for provenance and compatibility.
//! Program state processor

use {
    crate::runtime::hook::Execute,
    crate::runtime::system::{CreateAccountAllowPrefund, Funding},
    crate::runtime::{
        account::next_account_view,
        sysvars::{clock::Clock, rent::Rent, Sysvar},
        AccountView,
    },
    crate::{
        extension::{
            confidential_mint_burn, confidential_transfer, confidential_transfer_fee,
            cpi_guard::{self, in_cpi},
            default_account_state, group_member_pointer, group_pointer, interest_bearing_mint,
            memo_transfer::{self, check_previous_sibling_instruction_is_memo},
            metadata_pointer, pausable, permissioned_burn, reallocate, scaled_ui_amount,
            token_group, token_metadata, transfer_fee, transfer_hook,
        },
        pod_instruction::{
            decode_instruction_data_with_coption_pubkey, decode_instruction_data_with_coption_u64,
            AmountCheckedData, AmountData, InitializeMintData, InitializeMultisigData,
            PodTokenInstruction, SetAuthorityData,
        },
    },
    solana_address::Address,
    solana_cpi::set_return_data,
    solana_msg::msg,
    solana_program_error::{ProgramError, ProgramResult},
    solana_program_pack::Pack,
    solana_zero_copy::unaligned::U64,
    spl_token_2022_interface::{
        check_program_account,
        error::TokenError,
        extension::{
            account_len::{
                try_calculate_account_len_from_mint_data,
                try_for_each_required_init_account_extension,
            },
            confidential_mint_burn::ConfidentialMintBurn,
            confidential_transfer::{ConfidentialTransferAccount, ConfidentialTransferMint},
            confidential_transfer_fee::{
                ConfidentialTransferFeeAmount, ConfidentialTransferFeeConfig,
            },
            cpi_guard::CpiGuard,
            default_account_state::DefaultAccountState,
            group_member_pointer::GroupMemberPointer,
            group_pointer::GroupPointer,
            immutable_owner::ImmutableOwner,
            interest_bearing_mint::InterestBearingConfig,
            memo_transfer::memo_required,
            metadata_pointer::MetadataPointer,
            mint_close_authority::MintCloseAuthority,
            non_transferable::{NonTransferable, NonTransferableAccount},
            pausable::{PausableAccount, PausableConfig},
            permanent_delegate::{get_permanent_delegate, PermanentDelegate},
            permissioned_burn::PermissionedBurnConfig,
            scaled_ui_amount::ScaledUiAmountConfig,
            transfer_fee::{TransferFeeAmount, TransferFeeConfig},
            transfer_hook::{TransferHook, TransferHookAccount},
            BaseStateWithExtensions, BaseStateWithExtensionsMut, ExtensionType,
            PodStateWithExtensions, PodStateWithExtensionsMut,
        },
        inline_spl_token,
        instruction::{
            decode_instruction_data, decode_instruction_type, is_valid_signer_index, AuthorityType,
            MAX_SIGNERS,
        },
        native_mint::{self, PROGRAM_ADDRESS_SEEDS},
        pod::{PodAccount, PodCOption, PodMint, PodMultisig},
        state::{AccountState, Mint, PackedSizeOf},
    },
    spl_token_group_interface::instruction::TokenGroupInstruction,
    spl_token_metadata_interface::instruction::TokenMetadataInstruction,
    std::convert::{TryFrom, TryInto},
};

pub(crate) enum TransferInstruction {
    Unchecked,
    Checked { decimals: u8 },
    CheckedWithFee { decimals: u8, fee: u64 },
}

pub(crate) enum InstructionVariant {
    Unchecked,
    Checked { decimals: u8 },
}

/// Burn instruction variant. Standard variants must not be used with the
/// permissioned burn extension.
///
/// Permissioned variants require the extra authority to sign.
pub(crate) enum BurnInstructionVariant {
    Standard,
    Permissioned,
}

/// Program state handler.
pub struct Processor {}
impl Processor {
    fn _process_initialize_mint(
        accounts: &mut [AccountView],
        decimals: u8,
        mint_authority: &Address,
        freeze_authority: PodCOption<Address>,
        rent_sysvar_account: bool,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();
        let mint_info = next_account_view(account_info_iter)?;
        let mint_data_len = mint_info.data_len();

        // Note: Storing the value of the owner so borrow checker doesn't complain about
        // multiple borrows of `mint_info`.
        let mint_owner = mint_info.owner();
        let mint_lamports = mint_info.lamports();

        let mut mint_data = mint_info.try_borrow_mut()?;

        check_program_account(&mint_owner)?;

        let rent = if rent_sysvar_account {
            Rent::from_account_view(next_account_view(account_info_iter)?)?
        } else {
            Rent::get()?
        };

        if !rent.is_exempt(mint_lamports, mint_data_len) {
            return Err(TokenError::NotRentExempt.into());
        }

        let mut mint = PodStateWithExtensionsMut::<PodMint>::unpack_uninitialized(&mut mint_data)?;
        let extension_types = mint.get_extension_types()?;
        if ExtensionType::try_calculate_account_len::<Mint>(&extension_types)? != mint_data_len {
            return Err(ProgramError::InvalidAccountData);
        }
        ExtensionType::check_for_invalid_mint_extension_combinations(&extension_types)?;

        if let Ok(default_account_state) = mint.get_extension_mut::<DefaultAccountState>() {
            let default_account_state = AccountState::try_from(default_account_state.state)
                .or(Err(ProgramError::InvalidAccountData))?;
            if default_account_state == AccountState::Frozen && freeze_authority.is_none() {
                return Err(TokenError::MintCannotFreeze.into());
            }
        }

        mint.base.mint_authority = PodCOption::some(*mint_authority);
        mint.base.decimals = decimals;
        mint.base.is_initialized = true.into();
        mint.base.freeze_authority = freeze_authority;
        mint.init_account_type()?;

        Ok(())
    }

    /// Processes an [`InitializeMint`](enum.TokenInstruction.html) instruction.
    pub fn process_initialize_mint(
        accounts: &mut [AccountView],
        decimals: u8,
        mint_authority: &Address,
        freeze_authority: PodCOption<Address>,
    ) -> ProgramResult {
        Self::_process_initialize_mint(accounts, decimals, mint_authority, freeze_authority, true)
    }

    /// Processes an [`InitializeMint2`](enum.TokenInstruction.html)
    /// instruction.
    pub fn process_initialize_mint2(
        accounts: &mut [AccountView],
        decimals: u8,
        mint_authority: &Address,
        freeze_authority: PodCOption<Address>,
    ) -> ProgramResult {
        Self::_process_initialize_mint(accounts, decimals, mint_authority, freeze_authority, false)
    }

    fn _process_initialize_account(
        accounts: &mut [AccountView],
        owner: Option<&Address>,
        rent_sysvar_account: bool,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();
        let new_account_info = next_account_view(account_info_iter)?;
        let mint_info = next_account_view(account_info_iter)?;

        check_program_account(&new_account_info.owner())?;
        check_program_account(&mint_info.owner())?;

        let owner = if let Some(owner) = owner {
            owner
        } else {
            next_account_view(account_info_iter)?.address()
        };
        let new_account_info_data_len = new_account_info.data_len();
        let rent = if rent_sysvar_account {
            Rent::from_account_view(next_account_view(account_info_iter)?)?
        } else {
            Rent::get()?
        };

        // Note: Storing the lamports value so borrow checker doesn't complain about
        // multiple borrows of `new_account_info`.
        let new_account_lamports = new_account_info.lamports();
        let mut account_data = new_account_info.try_borrow_mut()?;
        // unpack_uninitialized checks account.base.is_initialized() under the hood
        let mut account =
            PodStateWithExtensionsMut::<PodAccount>::unpack_uninitialized(&mut account_data)?;

        if !rent.is_exempt(new_account_lamports, new_account_info_data_len) {
            return Err(TokenError::NotRentExempt.into());
        }

        let mint_data = mint_info.try_borrow()?;
        let mint = PodStateWithExtensions::<PodMint>::unpack(&mint_data)
            .map_err(|_| Into::<ProgramError>::into(TokenError::InvalidMint))?;
        if mint
            .get_extension::<PermanentDelegate>()
            .map(|e| Option::<Address>::from(e.delegate).is_some())
            .unwrap_or(false)
        {
            msg!("Warning: Mint has a permanent delegate, so tokens in this account may be seized at any time");
        }

        check_program_account(&mint_info.owner())?;

        // Sizing walks every TLV entry, which also validates the extension data. It is only a
        // lower bound since it skips extensions already initialized on the account (e.g.
        // `ImmutableOwner`), but each init call below still verifies the space for its own write.
        // Passing the initialized types as `additional_account_extensions` would make it exact.
        let required_account_len = try_calculate_account_len_from_mint_data(&mint_data, &[])?;
        if required_account_len > new_account_info_data_len {
            return Err(ProgramError::InvalidAccountData);
        }
        try_for_each_required_init_account_extension(mint.get_tlv_data(), |extension_type| {
            account.init_account_extension_from_type(extension_type)
        })?;

        let starting_state =
            if let Ok(default_account_state) = mint.get_extension::<DefaultAccountState>() {
                AccountState::try_from(default_account_state.state)
                    .or(Err(ProgramError::InvalidAccountData))?
            } else {
                AccountState::Initialized
            };

        account.base.mint = *mint_info.address();
        account.base.owner = *owner;
        account.base.close_authority = PodCOption::none();
        account.base.delegate = PodCOption::none();
        account.base.delegated_amount = 0.into();
        account.base.state = starting_state.into();
        if mint_info.address() == &native_mint::id() {
            let rent_exempt_reserve = rent.try_minimum_balance(new_account_info_data_len)?;
            account.base.is_native = PodCOption::some(rent_exempt_reserve.into());
            account.base.amount = new_account_lamports
                .checked_sub(rent_exempt_reserve)
                .ok_or(TokenError::Overflow)?
                .into();
        } else {
            account.base.is_native = PodCOption::none();
            account.base.amount = 0.into();
        }

        account.init_account_type()?;

        Ok(())
    }

    /// Processes an [`InitializeAccount`](enum.TokenInstruction.html)
    /// instruction.
    pub fn process_initialize_account(accounts: &mut [AccountView]) -> ProgramResult {
        Self::_process_initialize_account(accounts, None, true)
    }

    /// Processes an [`InitializeAccount2`](enum.TokenInstruction.html)
    /// instruction.
    pub fn process_initialize_account2(
        accounts: &mut [AccountView],
        owner: &Address,
    ) -> ProgramResult {
        Self::_process_initialize_account(accounts, Some(owner), true)
    }

    /// Processes an [`InitializeAccount3`](enum.TokenInstruction.html)
    /// instruction.
    pub fn process_initialize_account3(
        accounts: &mut [AccountView],
        owner: &Address,
    ) -> ProgramResult {
        Self::_process_initialize_account(accounts, Some(owner), false)
    }

    fn _process_initialize_multisig(
        accounts: &mut [AccountView],
        m: u8,
        rent_sysvar_account: bool,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();
        let multisig_info = next_account_view(account_info_iter)?;

        check_program_account(&multisig_info.owner())?;

        let multisig_info_data_len = multisig_info.data_len();
        let rent = if rent_sysvar_account {
            Rent::from_account_view(next_account_view(account_info_iter)?)?
        } else {
            Rent::get()?
        };

        // Note: Storing the lamports value so borrow checker doesn't complain about
        // borrowing `multisig_info` multiple times.
        let multisig_lamports = multisig_info.lamports();

        let mut multisig_data = multisig_info.try_borrow_mut()?;
        let multisig = bytemuck::try_from_bytes_mut::<PodMultisig>(&mut multisig_data)
            .map_err(|_| ProgramError::InvalidArgument)?;
        if bool::from(multisig.is_initialized) {
            return Err(TokenError::AlreadyInUse.into());
        }

        if !rent.is_exempt(multisig_lamports, multisig_info_data_len) {
            return Err(TokenError::NotRentExempt.into());
        }

        let signer_infos = account_info_iter.as_slice();
        multisig.m = m;
        multisig.n = signer_infos.len() as u8;
        if !is_valid_signer_index(multisig.n as usize) {
            return Err(TokenError::InvalidNumberOfProvidedSigners.into());
        }
        if !is_valid_signer_index(multisig.m as usize) {
            return Err(TokenError::InvalidNumberOfRequiredSigners.into());
        }
        for (i, signer_info) in signer_infos.iter().enumerate() {
            multisig.signers[i] = *signer_info.address();
        }
        multisig.is_initialized = true.into();

        Ok(())
    }

    /// Processes a [`InitializeMultisig`](enum.TokenInstruction.html)
    /// instruction.
    pub fn process_initialize_multisig(accounts: &mut [AccountView], m: u8) -> ProgramResult {
        Self::_process_initialize_multisig(accounts, m, true)
    }

    /// Processes a [`InitializeMultisig2`](enum.TokenInstruction.html)
    /// instruction.
    pub fn process_initialize_multisig2(accounts: &mut [AccountView], m: u8) -> ProgramResult {
        Self::_process_initialize_multisig(accounts, m, false)
    }

    /// Processes a [`Transfer`](enum.TokenInstruction.html) instruction.
    pub(crate) fn process_transfer(
        program_id: &Address,
        accounts: &mut [AccountView],
        amount: u64,
        transfer_instruction: TransferInstruction,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();

        let source_account_info = next_account_view(account_info_iter)?;

        let expected_mint_info = match transfer_instruction {
            TransferInstruction::Unchecked => None,
            TransferInstruction::Checked { decimals }
            | TransferInstruction::CheckedWithFee { decimals, .. } => {
                Some((next_account_view(account_info_iter)?, decimals))
            }
        };

        let destination_account_info = next_account_view(account_info_iter)?;
        let authority_info = next_account_view(account_info_iter)?;
        let authority_info_data_len = authority_info.data_len();

        check_program_account(&source_account_info.owner())?;
        check_program_account(&destination_account_info.owner())?;

        // Note: Make a copy of the source account to borrow the data so we can use the account
        // info again without violating the borrow checker. The account data is not re-borrowed on
        // the processor.
        let mut cloned_source_account_info = source_account_info.clone();
        let mut source_account_data = cloned_source_account_info.try_borrow_mut()?;
        let mut source_account =
            PodStateWithExtensionsMut::<PodAccount>::unpack(&mut source_account_data)?;
        if source_account.base.is_frozen() {
            return Err(TokenError::AccountFrozen.into());
        }
        let source_amount = u64::from(source_account.base.amount);
        if source_amount < amount {
            return Err(TokenError::InsufficientFunds.into());
        }
        if source_account.get_extension::<NonTransferableAccount>().is_ok() {
            return Err(TokenError::NonTransferable.into());
        }

        let (calculated_fee, maybe_permanent_delegate, maybe_transfer_hook_program_id) =
            if let Some((ref mint_info, expected_decimals)) = expected_mint_info {
                check_program_account(&mint_info.owner())?;

                if &source_account.base.mint != mint_info.address() {
                    return Err(TokenError::MintMismatch.into());
                }

                let mint_data = mint_info.try_borrow()?;
                let mint = PodStateWithExtensions::<PodMint>::unpack(&mint_data)?;

                if expected_decimals != mint.base.decimals {
                    return Err(TokenError::MintDecimalsMismatch.into());
                }

                let fee = if let Ok(transfer_fee_config) = mint.get_extension::<TransferFeeConfig>()
                {
                    transfer_fee_config
                        .calculate_epoch_fee(Clock::get()?.epoch, amount)
                        .ok_or(TokenError::Overflow)?
                } else {
                    0
                };

                if let Ok(extension) = mint.get_extension::<PausableConfig>() {
                    if extension.paused.into() {
                        return Err(TokenError::MintPaused.into());
                    }
                }

                let maybe_permanent_delegate = get_permanent_delegate(&mint);
                let maybe_transfer_hook_program_id = transfer_hook::get_program_id(&mint);

                (fee, maybe_permanent_delegate, maybe_transfer_hook_program_id)
            } else {
                // Transfer hook extension exists on the account, but no mint
                // was provided to figure out required accounts, abort
                if source_account.get_extension::<TransferHookAccount>().is_ok() {
                    return Err(TokenError::MintRequiredForTransfer.into());
                }

                // Transfer fee amount extension exists on the account, but no mint
                // was provided to calculate the fee, abort
                if source_account.get_extension_mut::<TransferFeeAmount>().is_ok() {
                    return Err(TokenError::MintRequiredForTransfer.into());
                }

                // Pausable extension exists on the account, but no mint
                // was provided to see if it's paused, abort
                if source_account.get_extension::<PausableAccount>().is_ok() {
                    return Err(TokenError::MintRequiredForTransfer.into());
                }

                (0, None, None)
            };
        if let TransferInstruction::CheckedWithFee { fee, .. } = transfer_instruction {
            if calculated_fee != fee {
                msg!("Calculated fee {calculated_fee}, received {fee}");
                return Err(TokenError::FeeMismatch.into());
            }
        }

        let self_transfer = source_account_info.address() == destination_account_info.address();
        if let Ok(cpi_guard) = source_account.get_extension::<CpiGuard>() {
            // Blocks all cases where the authority has signed if CPI Guard is
            // enabled, including:
            // * the account is delegated to the owner
            // * the account owner is the permanent delegate
            if *authority_info.address() == source_account.base.owner
                && cpi_guard.lock_cpi.into()
                && in_cpi()
            {
                return Err(TokenError::CpiGuardTransferBlocked.into());
            }
        }
        match (source_account.base.delegate, maybe_permanent_delegate) {
            (_, Some(ref delegate)) if authority_info.address() == delegate => {
                Self::validate_owner(
                    program_id,
                    delegate,
                    authority_info,
                    authority_info_data_len,
                    account_info_iter.as_slice(),
                )?
            }
            (PodCOption { option: PodCOption::<Address>::SOME, value: delegate }, _)
                if authority_info.address() == &delegate =>
            {
                Self::validate_owner(
                    program_id,
                    &delegate,
                    authority_info,
                    authority_info_data_len,
                    account_info_iter.as_slice(),
                )?;
                let delegated_amount = u64::from(source_account.base.delegated_amount);
                if delegated_amount < amount {
                    return Err(TokenError::InsufficientFunds.into());
                }
                if !self_transfer {
                    source_account.base.delegated_amount =
                        delegated_amount.checked_sub(amount).ok_or(TokenError::Overflow)?.into();
                    if u64::from(source_account.base.delegated_amount) == 0 {
                        source_account.base.delegate = PodCOption::none();
                    }
                }
            }
            _ => {
                Self::validate_owner(
                    program_id,
                    &source_account.base.owner,
                    authority_info,
                    authority_info_data_len,
                    account_info_iter.as_slice(),
                )?;
            }
        }

        // This check MUST occur just before the amounts are manipulated
        // to ensure self-transfers are fully validated
        if self_transfer {
            if memo_required(&source_account) {
                check_previous_sibling_instruction_is_memo()?;
            }
            return Ok(());
        }

        // Note: Make a copy of the destination account to borrow the data so we can use the account
        // info again without violating the borrow checker. The account data is not re-borrowed on
        // the processor.
        let mut cloned_destination_account_info = destination_account_info.clone();
        // self-transfer was dealt with earlier, so this *should* be safe
        let mut destination_account_data = cloned_destination_account_info.try_borrow_mut()?;
        let mut destination_account =
            PodStateWithExtensionsMut::<PodAccount>::unpack(&mut destination_account_data)?;

        if destination_account.base.is_frozen() {
            return Err(TokenError::AccountFrozen.into());
        }
        if source_account.base.mint != destination_account.base.mint {
            return Err(TokenError::MintMismatch.into());
        }

        if memo_required(&destination_account) {
            check_previous_sibling_instruction_is_memo()?;
        }

        if let Ok(confidential_transfer_state) =
            destination_account.get_extension::<ConfidentialTransferAccount>()
        {
            confidential_transfer_state.non_confidential_transfer_allowed()?
        }

        source_account.base.amount =
            source_amount.checked_sub(amount).ok_or(TokenError::Overflow)?.into();
        let credited_amount = amount.checked_sub(calculated_fee).ok_or(TokenError::Overflow)?;
        destination_account.base.amount = u64::from(destination_account.base.amount)
            .checked_add(credited_amount)
            .ok_or(TokenError::Overflow)?
            .into();
        if calculated_fee > 0 {
            if let Ok(extension) = destination_account.get_extension_mut::<TransferFeeAmount>() {
                let new_withheld_amount = u64::from(extension.withheld_amount)
                    .checked_add(calculated_fee)
                    .ok_or(TokenError::Overflow)?;
                extension.withheld_amount = new_withheld_amount.into();
            } else {
                // Use the generic error since this should never happen. If there's
                // a fee, then the mint has a fee configured, which means all accounts
                // must have the withholding.
                return Err(TokenError::InvalidState.into());
            }
        }

        if source_account.base.is_native() {
            let source_starting_lamports = source_account_info.lamports();
            source_account_info.set_lamports(
                source_starting_lamports.checked_sub(amount).ok_or(TokenError::Overflow)?,
            );

            let destination_starting_lamports = destination_account_info.lamports();
            destination_account_info.set_lamports(
                destination_starting_lamports.checked_add(amount).ok_or(TokenError::Overflow)?,
            );
        }

        if let Some(program_id) = maybe_transfer_hook_program_id {
            if let Some((mint_info, _)) = expected_mint_info {
                // set transferring flags
                transfer_hook::set_transferring(&mut source_account)?;
                transfer_hook::set_transferring(&mut destination_account)?;

                // must drop these to avoid the double-borrow during CPI
                drop(source_account_data);
                drop(destination_account_data);
                Execute {
                    program_id: &program_id,
                    source: source_account_info,
                    mint: mint_info,
                    destination: destination_account_info,
                    authority: authority_info,
                    additional_accounts: account_info_iter.as_slice(),
                    amount,
                }
                .invoke()?;

                // unset transferring flag
                crate::extension::unset_transferring(source_account_info)?;
                crate::extension::unset_transferring(destination_account_info)?;
            } else {
                return Err(TokenError::MintRequiredForTransfer.into());
            }
        }

        Ok(())
    }

    /// Processes an [`Approve`](enum.TokenInstruction.html) instruction.
    pub(crate) fn process_approve(
        program_id: &Address,
        accounts: &mut [AccountView],
        amount: u64,
        instruction_variant: InstructionVariant,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();

        let source_account_info = next_account_view(account_info_iter)?;

        check_program_account(&source_account_info.owner())?;

        let expected_mint_info =
            if let InstructionVariant::Checked { decimals } = instruction_variant {
                Some((next_account_view(account_info_iter)?, decimals))
            } else {
                None
            };
        let delegate_info = next_account_view(account_info_iter)?;
        let owner_info = next_account_view(account_info_iter)?;
        let owner_info_data_len = owner_info.data_len();

        let mut source_account_data = source_account_info.try_borrow_mut()?;
        let source_account =
            PodStateWithExtensionsMut::<PodAccount>::unpack(&mut source_account_data)?;

        if source_account.base.is_frozen() {
            return Err(TokenError::AccountFrozen.into());
        }

        if let Some((mint_info, expected_decimals)) = expected_mint_info {
            check_program_account(&mint_info.owner())?;

            if &source_account.base.mint != mint_info.address() {
                return Err(TokenError::MintMismatch.into());
            }

            let mint_data = mint_info.try_borrow()?;
            let mint = PodStateWithExtensions::<PodMint>::unpack(&mint_data)?;
            if expected_decimals != mint.base.decimals {
                return Err(TokenError::MintDecimalsMismatch.into());
            }
        }

        Self::validate_owner(
            program_id,
            &source_account.base.owner,
            owner_info,
            owner_info_data_len,
            account_info_iter.as_slice(),
        )?;

        if let Ok(cpi_guard) = source_account.get_extension::<CpiGuard>() {
            if cpi_guard.lock_cpi.into() && in_cpi() {
                return Err(TokenError::CpiGuardApproveBlocked.into());
            }
        }

        source_account.base.delegate = PodCOption::some(*delegate_info.address());
        source_account.base.delegated_amount = amount.into();

        Ok(())
    }

    /// Processes an [`Revoke`](enum.TokenInstruction.html) instruction.
    pub fn process_revoke(program_id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();
        let source_account_info = next_account_view(account_info_iter)?;
        let authority_info = next_account_view(account_info_iter)?;
        let authority_info_data_len = authority_info.data_len();

        check_program_account(&source_account_info.owner())?;

        let mut source_account_data = source_account_info.try_borrow_mut()?;
        let source_account =
            PodStateWithExtensionsMut::<PodAccount>::unpack(&mut source_account_data)?;
        if source_account.base.is_frozen() {
            return Err(TokenError::AccountFrozen.into());
        }

        Self::validate_owner(
            program_id,
            match &source_account.base.delegate {
                PodCOption { option: PodCOption::<Address>::SOME, value: delegate }
                    if authority_info.address() == delegate =>
                {
                    delegate
                }
                _ => &source_account.base.owner,
            },
            authority_info,
            authority_info_data_len,
            account_info_iter.as_slice(),
        )?;

        source_account.base.delegate = PodCOption::none();
        source_account.base.delegated_amount = 0.into();

        Ok(())
    }

    /// Processes a [`SetAuthority`](enum.TokenInstruction.html) instruction.
    pub fn process_set_authority(
        program_id: &Address,
        accounts: &mut [AccountView],
        authority_type: AuthorityType,
        new_authority: PodCOption<Address>,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();
        let account_info = next_account_view(account_info_iter)?;
        let authority_info = next_account_view(account_info_iter)?;
        let authority_info_data_len = authority_info.data_len();

        check_program_account(&account_info.owner())?;

        let mut account_data = account_info.try_borrow_mut()?;
        if let Ok(mut account) = PodStateWithExtensionsMut::<PodAccount>::unpack(&mut account_data)
        {
            if account.base.is_frozen() {
                return Err(TokenError::AccountFrozen.into());
            }

            match authority_type {
                AuthorityType::AccountOwner => {
                    Self::validate_owner(
                        program_id,
                        &account.base.owner,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;

                    if account.get_extension_mut::<ImmutableOwner>().is_ok() {
                        return Err(TokenError::ImmutableOwner.into());
                    }

                    if let Ok(cpi_guard) = account.get_extension::<CpiGuard>() {
                        if cpi_guard.lock_cpi.into() && in_cpi() {
                            return Err(TokenError::CpiGuardSetAuthorityBlocked.into());
                        } else if cpi_guard.lock_cpi.into() {
                            return Err(TokenError::CpiGuardOwnerChangeBlocked.into());
                        }
                    }

                    if let PodCOption { option: PodCOption::<Address>::SOME, value: authority } =
                        new_authority
                    {
                        account.base.owner = authority;
                    } else {
                        return Err(TokenError::InvalidInstruction.into());
                    }

                    account.base.delegate = PodCOption::none();
                    account.base.delegated_amount = 0.into();

                    if account.base.is_native() {
                        account.base.close_authority = PodCOption::none();
                    }
                }
                AuthorityType::CloseAccount => {
                    let authority = account.base.close_authority.unwrap_or(account.base.owner);
                    Self::validate_owner(
                        program_id,
                        &authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;

                    if let Ok(cpi_guard) = account.get_extension::<CpiGuard>() {
                        if cpi_guard.lock_cpi.into() && in_cpi() && new_authority.is_some() {
                            return Err(TokenError::CpiGuardSetAuthorityBlocked.into());
                        }
                    }

                    account.base.close_authority = new_authority;
                }
                _ => {
                    return Err(TokenError::AuthorityTypeNotSupported.into());
                }
            }
        } else if let Ok(mut mint) = PodStateWithExtensionsMut::<PodMint>::unpack(&mut account_data)
        {
            match authority_type {
                AuthorityType::MintTokens => {
                    // Once a mint's supply is fixed, it cannot be undone by setting a new
                    // mint_authority
                    let mint_authority = mint
                        .base
                        .mint_authority
                        .ok_or(Into::<ProgramError>::into(TokenError::FixedSupply))?;
                    Self::validate_owner(
                        program_id,
                        &mint_authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    mint.base.mint_authority = new_authority;
                }
                AuthorityType::FreezeAccount => {
                    // Once a mint's freeze authority is disabled, it cannot be re-enabled by
                    // setting a new freeze_authority
                    let freeze_authority = mint
                        .base
                        .freeze_authority
                        .ok_or(Into::<ProgramError>::into(TokenError::MintCannotFreeze))?;
                    Self::validate_owner(
                        program_id,
                        &freeze_authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    mint.base.freeze_authority = new_authority;
                }
                AuthorityType::CloseMint => {
                    let extension = mint.get_extension_mut::<MintCloseAuthority>()?;
                    let maybe_close_authority: Option<Address> = extension.close_authority.into();
                    let close_authority =
                        maybe_close_authority.ok_or(TokenError::AuthorityTypeNotSupported)?;
                    Self::validate_owner(
                        program_id,
                        &close_authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    extension.close_authority = new_authority.try_into()?;
                }
                AuthorityType::TransferFeeConfig => {
                    let extension = mint.get_extension_mut::<TransferFeeConfig>()?;
                    let maybe_transfer_fee_config_authority: Option<Address> =
                        extension.transfer_fee_config_authority.into();
                    let transfer_fee_config_authority = maybe_transfer_fee_config_authority
                        .ok_or(TokenError::AuthorityTypeNotSupported)?;
                    Self::validate_owner(
                        program_id,
                        &transfer_fee_config_authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    extension.transfer_fee_config_authority = new_authority.try_into()?;
                }
                AuthorityType::WithheldWithdraw => {
                    let extension = mint.get_extension_mut::<TransferFeeConfig>()?;
                    let maybe_withdraw_withheld_authority: Option<Address> =
                        extension.withdraw_withheld_authority.into();
                    let withdraw_withheld_authority = maybe_withdraw_withheld_authority
                        .ok_or(TokenError::AuthorityTypeNotSupported)?;
                    Self::validate_owner(
                        program_id,
                        &withdraw_withheld_authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    extension.withdraw_withheld_authority = new_authority.try_into()?;
                }
                AuthorityType::InterestRate => {
                    let extension = mint.get_extension_mut::<InterestBearingConfig>()?;
                    let maybe_rate_authority: Option<Address> = extension.rate_authority.into();
                    let rate_authority =
                        maybe_rate_authority.ok_or(TokenError::AuthorityTypeNotSupported)?;
                    Self::validate_owner(
                        program_id,
                        &rate_authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    extension.rate_authority = new_authority.try_into()?;
                }
                AuthorityType::PermanentDelegate => {
                    let extension = mint.get_extension_mut::<PermanentDelegate>()?;
                    let maybe_delegate: Option<Address> = extension.delegate.into();
                    let delegate = maybe_delegate.ok_or(TokenError::AuthorityTypeNotSupported)?;
                    Self::validate_owner(
                        program_id,
                        &delegate,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    extension.delegate = new_authority.try_into()?;
                }
                AuthorityType::ConfidentialTransferMint => {
                    let extension = mint.get_extension_mut::<ConfidentialTransferMint>()?;
                    let maybe_confidential_transfer_mint_authority: Option<Address> =
                        extension.authority.into();
                    let confidential_transfer_mint_authority =
                        maybe_confidential_transfer_mint_authority
                            .ok_or(TokenError::AuthorityTypeNotSupported)?;
                    Self::validate_owner(
                        program_id,
                        &confidential_transfer_mint_authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    extension.authority = new_authority.try_into()?;
                }
                AuthorityType::TransferHookProgramId => {
                    let extension = mint.get_extension_mut::<TransferHook>()?;
                    let maybe_authority: Option<Address> = extension.authority.into();
                    let authority = maybe_authority.ok_or(TokenError::AuthorityTypeNotSupported)?;
                    Self::validate_owner(
                        program_id,
                        &authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    extension.authority = new_authority.try_into()?;
                }
                AuthorityType::ConfidentialTransferFeeConfig => {
                    let extension = mint.get_extension_mut::<ConfidentialTransferFeeConfig>()?;
                    let maybe_authority: Option<Address> = extension.authority.into();
                    let authority = maybe_authority.ok_or(TokenError::AuthorityTypeNotSupported)?;
                    Self::validate_owner(
                        program_id,
                        &authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    extension.authority = new_authority.try_into()?;
                }
                AuthorityType::MetadataPointer => {
                    let extension = mint.get_extension_mut::<MetadataPointer>()?;
                    let maybe_authority: Option<Address> = extension.authority.into();
                    let authority = maybe_authority.ok_or(TokenError::AuthorityTypeNotSupported)?;
                    Self::validate_owner(
                        program_id,
                        &authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    extension.authority = new_authority.try_into()?;
                }
                AuthorityType::GroupPointer => {
                    let extension = mint.get_extension_mut::<GroupPointer>()?;
                    let maybe_authority: Option<Address> = extension.authority.into();
                    let authority = maybe_authority.ok_or(TokenError::AuthorityTypeNotSupported)?;
                    Self::validate_owner(
                        program_id,
                        &authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    extension.authority = new_authority.try_into()?;
                }
                AuthorityType::GroupMemberPointer => {
                    let extension = mint.get_extension_mut::<GroupMemberPointer>()?;
                    let maybe_authority: Option<Address> = extension.authority.into();
                    let authority = maybe_authority.ok_or(TokenError::AuthorityTypeNotSupported)?;
                    Self::validate_owner(
                        program_id,
                        &authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    extension.authority = new_authority.try_into()?;
                }
                AuthorityType::ScaledUiAmount => {
                    let extension = mint.get_extension_mut::<ScaledUiAmountConfig>()?;
                    let maybe_authority: Option<Address> = extension.authority.into();
                    let authority = maybe_authority.ok_or(TokenError::AuthorityTypeNotSupported)?;
                    Self::validate_owner(
                        program_id,
                        &authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    extension.authority = new_authority.try_into()?;
                }
                AuthorityType::Pause => {
                    let extension = mint.get_extension_mut::<PausableConfig>()?;
                    let maybe_authority: Option<Address> = extension.authority.into();
                    let authority = maybe_authority.ok_or(TokenError::AuthorityTypeNotSupported)?;
                    Self::validate_owner(
                        program_id,
                        &authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    extension.authority = new_authority.try_into()?;
                }
                AuthorityType::PermissionedBurn => {
                    let extension = mint.get_extension_mut::<PermissionedBurnConfig>()?;
                    let maybe_authority: Option<Address> = extension.authority.into();
                    let authority = maybe_authority.ok_or(TokenError::AuthorityTypeNotSupported)?;
                    Self::validate_owner(
                        program_id,
                        &authority,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                    extension.authority = new_authority.try_into()?;
                }
                _ => {
                    return Err(TokenError::AuthorityTypeNotSupported.into());
                }
            }
        } else {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(())
    }

    /// Processes a [`MintTo`](enum.TokenInstruction.html) instruction.
    pub(crate) fn process_mint_to(
        program_id: &Address,
        accounts: &mut [AccountView],
        amount: u64,
        instruction_variant: InstructionVariant,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();
        let mint_info = next_account_view(account_info_iter)?;
        let destination_account_info = next_account_view(account_info_iter)?;
        let owner_info = next_account_view(account_info_iter)?;
        let owner_info_data_len = owner_info.data_len();

        check_program_account(&mint_info.owner())?;
        check_program_account(&destination_account_info.owner())?;

        let mut destination_account_data = destination_account_info.try_borrow_mut()?;
        let destination_account =
            PodStateWithExtensionsMut::<PodAccount>::unpack(&mut destination_account_data)?;
        if destination_account.base.is_frozen() {
            return Err(TokenError::AccountFrozen.into());
        }

        if destination_account.base.is_native() {
            return Err(TokenError::NativeNotSupported.into());
        }
        if mint_info.address() != &destination_account.base.mint {
            return Err(TokenError::MintMismatch.into());
        }

        let mut mint_data = mint_info.try_borrow_mut()?;
        let mint = PodStateWithExtensionsMut::<PodMint>::unpack(&mut mint_data)?;

        // If the mint if non-transferable, only allow minting to accounts
        // with immutable ownership and the non-transferable extension.
        if mint.get_extension::<NonTransferable>().is_ok()
            && (destination_account.get_extension::<ImmutableOwner>().is_err()
                || destination_account.get_extension::<NonTransferableAccount>().is_err())
        {
            // This error name isn't totally correct since we also require the
            // non-transferable account extension, but that case only happens in
            // the case of a mint recreated with the extension, which should be
            // very rare. We leave the error name as it was.
            return Err(TokenError::NonTransferableNeedsImmutableOwnership.into());
        }

        if let Ok(extension) = mint.get_extension::<PausableConfig>() {
            if extension.paused.into() {
                return Err(TokenError::MintPaused.into());
            }
        }

        if mint.get_extension::<ConfidentialMintBurn>().is_ok() {
            return Err(TokenError::IllegalMintBurnConversion.into());
        }

        if let InstructionVariant::Checked { decimals } = instruction_variant {
            if decimals != mint.base.decimals {
                return Err(TokenError::MintDecimalsMismatch.into());
            }
        }

        match &mint.base.mint_authority {
            PodCOption { option: PodCOption::<Address>::SOME, value: mint_authority } => {
                Self::validate_owner(
                    program_id,
                    mint_authority,
                    owner_info,
                    owner_info_data_len,
                    account_info_iter.as_slice(),
                )?
            }
            _ => return Err(TokenError::FixedSupply.into()),
        }

        destination_account.base.amount = u64::from(destination_account.base.amount)
            .checked_add(amount)
            .ok_or(TokenError::Overflow)?
            .into();

        mint.base.supply =
            u64::from(mint.base.supply).checked_add(amount).ok_or(TokenError::Overflow)?.into();

        Ok(())
    }

    /// Processes a [`Burn`](enum.TokenInstruction.html) instruction.
    pub(crate) fn process_burn(
        program_id: &Address,
        accounts: &mut [AccountView],
        amount: u64,
        burn_variant: BurnInstructionVariant,
        instruction_variant: InstructionVariant,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();

        let source_account_info = next_account_view(account_info_iter)?;
        let mint_info = next_account_view(account_info_iter)?;

        check_program_account(&source_account_info.owner())?;
        check_program_account(&mint_info.owner())?;

        let (permissioned_burn_authority_info, authority_info) = match burn_variant {
            BurnInstructionVariant::Permissioned => {
                let permissioned_burn_authority_info = next_account_view(account_info_iter)?;
                let authority_info = next_account_view(account_info_iter)?;
                (Some(permissioned_burn_authority_info), authority_info)
            }
            BurnInstructionVariant::Standard => (None, next_account_view(account_info_iter)?),
        };

        let authority_info_data_len = authority_info.data_len();

        // Note: Storing the address of the mint so borrow checker doesn't complain about
        // multiple borrows of `mint_info`.
        let mint_address = *mint_info.address();

        let mut mint_data = mint_info.try_borrow_mut()?;
        let mint = PodStateWithExtensionsMut::<PodMint>::unpack(&mut mint_data)?;

        let permissioned_ext = mint.get_extension::<PermissionedBurnConfig>();
        let maybe_permissioned_burn_authority =
            permissioned_ext.as_ref().ok().and_then(|ext| Option::<Address>::from(ext.authority));

        match burn_variant {
            BurnInstructionVariant::Standard => {
                // Standard burns cannot be used when the permissioned burn
                // extension is present.
                if maybe_permissioned_burn_authority.is_some() {
                    return Err(TokenError::InvalidInstruction.into());
                }
            }
            BurnInstructionVariant::Permissioned => {
                permissioned_ext.map_err(|_| TokenError::InvalidInstruction)?;

                let expected_burn_authority =
                    maybe_permissioned_burn_authority.ok_or_else(|| {
                        msg!("Permissioned burn authority is None; use the standard burn");
                        TokenError::InvalidInstruction
                    })?;

                // Pull the required extra signer from the accounts
                let approver_ai =
                    permissioned_burn_authority_info.ok_or(ProgramError::NotEnoughAccountKeys)?;

                if !approver_ai.is_signer() {
                    return Err(ProgramError::MissingRequiredSignature);
                }

                if *approver_ai.address() != expected_burn_authority {
                    return Err(ProgramError::InvalidAccountData);
                }
            }
        }

        let mut source_account_data = source_account_info.try_borrow_mut()?;
        let source_account =
            PodStateWithExtensionsMut::<PodAccount>::unpack(&mut source_account_data)?;

        if source_account.base.is_frozen() {
            return Err(TokenError::AccountFrozen.into());
        }
        if source_account.base.is_native() {
            return Err(TokenError::NativeNotSupported.into());
        }
        if u64::from(source_account.base.amount) < amount {
            return Err(TokenError::InsufficientFunds.into());
        }
        if mint_address != source_account.base.mint {
            return Err(TokenError::MintMismatch.into());
        }

        if let InstructionVariant::Checked { decimals } = instruction_variant {
            if decimals != mint.base.decimals {
                return Err(TokenError::MintDecimalsMismatch.into());
            }
        }
        if let Ok(extension) = mint.get_extension::<PausableConfig>() {
            if extension.paused.into() {
                return Err(TokenError::MintPaused.into());
            }
        }

        if mint.get_extension::<ConfidentialMintBurn>().is_ok() {
            return Err(TokenError::IllegalMintBurnConversion.into());
        }

        let maybe_permanent_delegate = get_permanent_delegate(&mint);

        if let Ok(cpi_guard) = source_account.get_extension::<CpiGuard>() {
            // Blocks all cases where the authority has signed if CPI Guard is
            // enabled, including:
            // * the account is delegated to the owner
            // * the account owner is the permanent delegate
            if *authority_info.address() == source_account.base.owner
                && cpi_guard.lock_cpi.into()
                && in_cpi()
            {
                return Err(TokenError::CpiGuardBurnBlocked.into());
            }
        }

        if !source_account.base.is_owned_by_system_program_or_incinerator() {
            match (&source_account.base.delegate, maybe_permanent_delegate) {
                (_, Some(ref delegate)) if authority_info.address() == delegate => {
                    Self::validate_owner(
                        program_id,
                        delegate,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?
                }
                (PodCOption { option: PodCOption::<Address>::SOME, value: delegate }, _)
                    if authority_info.address() == delegate =>
                {
                    Self::validate_owner(
                        program_id,
                        delegate,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;

                    if u64::from(source_account.base.delegated_amount) < amount {
                        return Err(TokenError::InsufficientFunds.into());
                    }
                    source_account.base.delegated_amount =
                        u64::from(source_account.base.delegated_amount)
                            .checked_sub(amount)
                            .ok_or(TokenError::Overflow)?
                            .into();
                    if u64::from(source_account.base.delegated_amount) == 0 {
                        source_account.base.delegate = PodCOption::none();
                    }
                }
                _ => {
                    Self::validate_owner(
                        program_id,
                        &source_account.base.owner,
                        authority_info,
                        authority_info_data_len,
                        account_info_iter.as_slice(),
                    )?;
                }
            }
        }

        source_account.base.amount = u64::from(source_account.base.amount)
            .checked_sub(amount)
            .ok_or(TokenError::Overflow)?
            .into();
        mint.base.supply =
            u64::from(mint.base.supply).checked_sub(amount).ok_or(TokenError::Overflow)?.into();

        Ok(())
    }

    /// Processes a [`CloseAccount`](enum.TokenInstruction.html) instruction.
    pub fn process_close_account(
        program_id: &Address,
        accounts: &mut [AccountView],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();
        let source_account_info = next_account_view(account_info_iter)?;
        let destination_account_info = next_account_view(account_info_iter)?;
        let authority_info = next_account_view(account_info_iter)?;
        let authority_info_data_len = authority_info.data_len();

        check_program_account(&source_account_info.owner())?;

        if source_account_info.address() == destination_account_info.address() {
            return Err(ProgramError::InvalidAccountData);
        }

        let source_account_data = source_account_info.try_borrow()?;
        if let Ok(source_account) =
            PodStateWithExtensions::<PodAccount>::unpack(&source_account_data)
        {
            if !source_account.base.is_native() && u64::from(source_account.base.amount) != 0 {
                return Err(TokenError::NonNativeHasBalance.into());
            }

            let authority =
                source_account.base.close_authority.unwrap_or(source_account.base.owner);

            if !source_account.base.is_owned_by_system_program_or_incinerator() {
                if let Ok(cpi_guard) = source_account.get_extension::<CpiGuard>() {
                    if cpi_guard.lock_cpi.into()
                        && in_cpi()
                        && destination_account_info.address() != &source_account.base.owner
                    {
                        return Err(TokenError::CpiGuardCloseAccountBlocked.into());
                    }
                }

                Self::validate_owner(
                    program_id,
                    &authority,
                    authority_info,
                    authority_info_data_len,
                    account_info_iter.as_slice(),
                )?;
            } else if !solana_sdk_ids::incinerator::check_id(destination_account_info.address()) {
                return Err(ProgramError::InvalidAccountData);
            }

            if let Ok(confidential_transfer_state) =
                source_account.get_extension::<ConfidentialTransferAccount>()
            {
                confidential_transfer_state.closable()?
            }

            if let Ok(confidential_transfer_fee_state) =
                source_account.get_extension::<ConfidentialTransferFeeAmount>()
            {
                confidential_transfer_fee_state.closable()?
            }

            if let Ok(transfer_fee_state) = source_account.get_extension::<TransferFeeAmount>() {
                transfer_fee_state.closable()?
            }
        } else if let Ok(mint) = PodStateWithExtensions::<PodMint>::unpack(&source_account_data) {
            let extension = mint.get_extension::<MintCloseAuthority>()?;
            let maybe_authority: Option<Address> = extension.close_authority.into();
            let authority = maybe_authority.ok_or(TokenError::AuthorityTypeNotSupported)?;
            Self::validate_owner(
                program_id,
                &authority,
                authority_info,
                authority_info_data_len,
                account_info_iter.as_slice(),
            )?;

            if u64::from(mint.base.supply) != 0 {
                return Err(TokenError::MintHasSupply.into());
            }

            if let Ok(confidential_mint_burn) = mint.get_extension::<ConfidentialMintBurn>() {
                confidential_mint_burn.closable()?;
            }
        } else {
            return Err(ProgramError::UninitializedAccount);
        }

        let destination_starting_lamports = destination_account_info.lamports();
        destination_account_info.set_lamports(
            destination_starting_lamports
                .checked_add(source_account_info.lamports())
                .ok_or(TokenError::Overflow)?,
        );

        drop(source_account_data);

        // SAFETY: There are no active borrows of `source_account_info` at this point.
        source_account_info.close()?;

        Ok(())
    }

    /// Processes a [`FreezeAccount`](enum.TokenInstruction.html) or a
    /// [`ThawAccount`](enum.TokenInstruction.html) instruction.
    pub fn process_toggle_freeze_account(
        program_id: &Address,
        accounts: &mut [AccountView],
        freeze: bool,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();
        let source_account_info = next_account_view(account_info_iter)?;
        let mint_info = next_account_view(account_info_iter)?;
        let authority_info = next_account_view(account_info_iter)?;
        let authority_info_data_len = authority_info.data_len();

        check_program_account(&source_account_info.owner())?;
        check_program_account(&mint_info.owner())?;

        let mut source_account_data = source_account_info.try_borrow_mut()?;
        let source_account =
            PodStateWithExtensionsMut::<PodAccount>::unpack(&mut source_account_data)?;
        if freeze && source_account.base.is_frozen() || !freeze && !source_account.base.is_frozen()
        {
            return Err(TokenError::InvalidState.into());
        }
        if source_account.base.is_native() {
            return Err(TokenError::NativeNotSupported.into());
        }
        if mint_info.address() != &source_account.base.mint {
            return Err(TokenError::MintMismatch.into());
        }

        let mint_data = mint_info.try_borrow()?;
        let mint = PodStateWithExtensions::<PodMint>::unpack(&mint_data)?;
        match &mint.base.freeze_authority {
            PodCOption { option: PodCOption::<Address>::SOME, value: authority } => {
                Self::validate_owner(
                    program_id,
                    authority,
                    authority_info,
                    authority_info_data_len,
                    account_info_iter.as_slice(),
                )
            }
            _ => Err(TokenError::MintCannotFreeze.into()),
        }?;

        source_account.base.state =
            if freeze { AccountState::Frozen.into() } else { AccountState::Initialized.into() };

        Ok(())
    }

    /// Processes a [`SyncNative`](enum.TokenInstruction.html) instruction
    pub fn process_sync_native(accounts: &mut [AccountView]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();
        let native_account_info = next_account_view(account_info_iter)?;

        check_program_account(&native_account_info.owner())?;

        let rent_exempt_reserve = if let Ok(rent_sysvar_info) = next_account_view(account_info_iter)
        {
            let rent = Rent::from_account_view(rent_sysvar_info)?;
            rent.try_minimum_balance(native_account_info.data_len())?
        } else {
            Rent::get()?.try_minimum_balance(native_account_info.data_len())?
        };

        // Note: Storing the lamports of the native account so borrow checker doesn't complain about
        // multiple borrows of `native_account_info`.
        let native_account_lamports = native_account_info.lamports();

        let mut native_account_data = native_account_info.try_borrow_mut()?;
        let native_account =
            PodStateWithExtensionsMut::<PodAccount>::unpack(&mut native_account_data)?;

        match native_account.base.is_native {
            PodCOption { option: PodCOption::<U64>::SOME, .. } => {
                let new_amount = native_account_lamports
                    .checked_sub(rent_exempt_reserve)
                    .ok_or(TokenError::Overflow)?;
                native_account.base.is_native = PodCOption::some(rent_exempt_reserve.into());
                native_account.base.amount = new_amount.into();
            }
            _ => return Err(TokenError::NonNativeNotSupported.into()),
        }

        Ok(())
    }

    /// Processes an
    /// [`InitializeMintCloseAuthority`](enum.TokenInstruction.html)
    /// instruction
    pub fn process_initialize_mint_close_authority(
        accounts: &mut [AccountView],
        close_authority: PodCOption<Address>,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();
        let mint_account_info = next_account_view(account_info_iter)?;

        check_program_account(&mint_account_info.owner())?;

        let mut mint_data = mint_account_info.try_borrow_mut()?;
        let mut mint = PodStateWithExtensionsMut::<PodMint>::unpack_uninitialized(&mut mint_data)?;
        let extension = mint.init_extension::<MintCloseAuthority>(true)?;
        extension.close_authority = close_authority.try_into()?;

        Ok(())
    }

    /// Processes a [`GetAccountDataSize`](enum.TokenInstruction.html)
    /// instruction
    pub fn process_get_account_data_size(
        accounts: &[AccountView],
        new_extension_types: &[ExtensionType],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let mint_account_info = next_account_view(account_info_iter)?;

        check_program_account(&mint_account_info.owner())?;

        let mint_data = mint_account_info.try_borrow()?;
        let account_len =
            try_calculate_account_len_from_mint_data(&mint_data, new_extension_types)?;
        set_return_data(&account_len.to_le_bytes());

        Ok(())
    }

    /// Processes an [`InitializeImmutableOwner`](enum.TokenInstruction.html)
    /// instruction
    pub fn process_initialize_immutable_owner(accounts: &mut [AccountView]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();
        let token_account_info = next_account_view(account_info_iter)?;

        check_program_account(&token_account_info.owner())?;

        let token_account_data = &mut token_account_info.try_borrow_mut()?;
        let mut token_account =
            PodStateWithExtensionsMut::<PodAccount>::unpack_uninitialized(token_account_data)?;
        token_account.init_extension::<ImmutableOwner>(true).map(|_| ())
    }

    /// Processes an [`AmountToUiAmount`](enum.TokenInstruction.html)
    /// instruction
    pub fn process_amount_to_ui_amount(accounts: &[AccountView], amount: u64) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let mint_info = next_account_view(account_info_iter)?;
        check_program_account(&mint_info.owner())?;

        let mint_data = mint_info.try_borrow()?;
        let mint = PodStateWithExtensions::<PodMint>::unpack(&mint_data)
            .map_err(|_| Into::<ProgramError>::into(TokenError::InvalidMint))?;
        let ui_amount = if let Ok(extension) = mint.get_extension::<InterestBearingConfig>() {
            let unix_timestamp = Clock::get()?.unix_timestamp;
            extension
                .amount_to_ui_amount(amount, mint.base.decimals, unix_timestamp)
                .ok_or(ProgramError::InvalidArgument)?
        } else if let Ok(extension) = mint.get_extension::<ScaledUiAmountConfig>() {
            let unix_timestamp = Clock::get()?.unix_timestamp;
            extension
                .amount_to_ui_amount(amount, mint.base.decimals, unix_timestamp)
                .ok_or(ProgramError::InvalidArgument)?
        } else {
            crate::amount_to_ui_amount_string_trimmed(amount, mint.base.decimals)
        };

        set_return_data(&ui_amount.into_bytes());
        Ok(())
    }

    /// Processes an [`UiAmountToAmount`](enum.TokenInstruction.html)
    /// instruction
    pub fn process_ui_amount_to_amount(accounts: &[AccountView], ui_amount: &str) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let mint_info = next_account_view(account_info_iter)?;
        check_program_account(&mint_info.owner())?;

        let mint_data = mint_info.try_borrow()?;
        let mint = PodStateWithExtensions::<PodMint>::unpack(&mint_data)
            .map_err(|_| Into::<ProgramError>::into(TokenError::InvalidMint))?;
        let amount = if let Ok(extension) = mint.get_extension::<InterestBearingConfig>() {
            let unix_timestamp = Clock::get()?.unix_timestamp;
            extension.try_ui_amount_into_amount(ui_amount, mint.base.decimals, unix_timestamp)?
        } else if let Ok(extension) = mint.get_extension::<ScaledUiAmountConfig>() {
            let unix_timestamp = Clock::get()?.unix_timestamp;
            extension.try_ui_amount_into_amount(ui_amount, mint.base.decimals, unix_timestamp)?
        } else {
            crate::try_ui_amount_into_amount(ui_amount.to_string(), mint.base.decimals)?
        };

        set_return_data(&amount.to_le_bytes());
        Ok(())
    }

    /// Processes a [`CreateNativeMint`](enum.TokenInstruction.html) instruction
    pub fn process_create_native_mint(accounts: &mut [AccountView]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();
        let payer_info = next_account_view(account_info_iter)?;
        let native_mint_info = next_account_view(account_info_iter)?;
        let _system_program_info = next_account_view(account_info_iter)?;

        if *native_mint_info.address() != native_mint::id() {
            return Err(TokenError::InvalidMint.into());
        }

        let rent = Rent::get()?;
        let new_minimum_balance = rent.try_minimum_balance(Mint::get_packed_len())?;
        let lamports_diff = new_minimum_balance.saturating_sub(native_mint_info.lamports());

        let signer_seeds = &[PROGRAM_ADDRESS_SEEDS[0], PROGRAM_ADDRESS_SEEDS[1]];

        // Note: Use `CreateAccountAllowPrefund` to create the native mint account with the
        // correct lamports.
        CreateAccountAllowPrefund {
            funding: Some(Funding { from: payer_info, lamports: lamports_diff }),
            to: native_mint_info,
            space: Mint::get_packed_len() as u64,
            owner: &crate::ID,
        }
        .invoke_signed(&[signer_seeds])?;

        Mint::pack(
            Mint { decimals: native_mint::DECIMALS, is_initialized: true, ..Mint::default() },
            &mut native_mint_info.try_borrow_mut()?,
        )
    }

    /// Processes an
    /// [`InitializeNonTransferableMint`](enum.TokenInstruction.html)
    /// instruction
    pub fn process_initialize_non_transferable_mint(accounts: &mut [AccountView]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();
        let mint_account_info = next_account_view(account_info_iter)?;
        check_program_account(&mint_account_info.owner())?;

        let mut mint_data = mint_account_info.try_borrow_mut()?;
        let mut mint = PodStateWithExtensionsMut::<PodMint>::unpack_uninitialized(&mut mint_data)?;
        mint.init_extension::<NonTransferable>(true)?;

        Ok(())
    }

    /// Processes an [`InitializePermanentDelegate`](enum.TokenInstruction.html)
    /// instruction
    pub fn process_initialize_permanent_delegate(
        accounts: &mut [AccountView],
        delegate: &Address,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();
        let mint_account_info = next_account_view(account_info_iter)?;
        check_program_account(&mint_account_info.owner())?;

        let mut mint_data = mint_account_info.try_borrow_mut()?;
        let mut mint = PodStateWithExtensionsMut::<PodMint>::unpack_uninitialized(&mut mint_data)?;
        let extension = mint.init_extension::<PermanentDelegate>(true)?;
        extension.delegate =
            Some(*delegate).try_into().map_err(|_| ProgramError::InvalidArgument)?;

        Ok(())
    }

    /// Withdraw Excess Lamports is used to recover Lamports transferred to any
    /// `TokenProgram` owned account by moving them to another account
    /// of the source account.
    pub fn process_withdraw_excess_lamports(
        program_id: &Address,
        accounts: &mut [AccountView],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();

        let source_info = next_account_view(account_info_iter)?;
        let destination_info = next_account_view(account_info_iter)?;
        let authority_info = next_account_view(account_info_iter)?;

        check_program_account(&source_info.owner())?;

        let source_data = source_info.try_borrow()?;

        if let Ok(account) = PodStateWithExtensions::<PodAccount>::unpack(&source_data) {
            if account.base.is_native() {
                return Err(TokenError::NativeNotSupported.into());
            }
            Self::validate_owner(
                program_id,
                &account.base.owner,
                authority_info,
                authority_info.data_len(),
                account_info_iter.as_slice(),
            )?;

            if let Ok(cpi_guard) = account.get_extension::<CpiGuard>() {
                if cpi_guard.lock_cpi.into() && in_cpi() {
                    return Err(TokenError::CpiGuardTransferBlocked.into());
                }
            }
        } else if let Ok(mint) = PodStateWithExtensions::<PodMint>::unpack(&source_data) {
            match &mint.base.mint_authority {
                PodCOption { option: PodCOption::<Address>::SOME, value: mint_authority } => {
                    Self::validate_owner(
                        program_id,
                        mint_authority,
                        authority_info,
                        authority_info.data_len(),
                        account_info_iter.as_slice(),
                    )?;
                }
                PodCOption { option: PodCOption::<Address>::NONE, value: _ }
                    if source_info.address() == authority_info.address() =>
                {
                    // This is a special case where there is no mint authority set but the mint
                    // account is the same as the authority account and, therefore, needs to be
                    // a signer.
                    if !authority_info.is_signer() {
                        return Err(ProgramError::MissingRequiredSignature);
                    }
                }
                _ => return Err(TokenError::AuthorityTypeNotSupported.into()),
            }
        } else if source_data.len() == PodMultisig::SIZE_OF {
            Self::validate_owner(
                program_id,
                source_info.address(),
                authority_info,
                authority_info.data_len(),
                account_info_iter.as_slice(),
            )?;
        } else {
            return Err(TokenError::InvalidState.into());
        }

        // Note: Drop the borrow of `source_data` to avoid borrow checker issues.
        drop(source_data);

        let source_rent_exempt_reserve =
            Rent::get()?.try_minimum_balance(source_info.data_len())?;

        let transfer_amount = source_info
            .lamports()
            .checked_sub(source_rent_exempt_reserve)
            .ok_or(TokenError::NotRentExempt)?;

        let source_starting_lamports = source_info.lamports();
        source_info.set_lamports(
            source_starting_lamports.checked_sub(transfer_amount).ok_or(TokenError::Overflow)?,
        );

        let destination_starting_lamports = destination_info.lamports();
        destination_info.set_lamports(
            destination_starting_lamports
                .checked_add(transfer_amount)
                .ok_or(TokenError::Overflow)?,
        );

        Ok(())
    }

    /// Processes a [`UnwrapLamports`](enum.TokenInstruction.html)
    /// instruction
    pub fn process_unwrap_lamports(
        program_id: &Address,
        accounts: &mut [AccountView],
        amount: PodCOption<u64>,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter_mut();
        let source_account_info = next_account_view(account_info_iter)?;
        let destination_account_info = next_account_view(account_info_iter)?;
        let authority_info = next_account_view(account_info_iter)?;
        let authority_info_data_len = authority_info.data_len();

        check_program_account(&source_account_info.owner())?;

        let mut source_account_data = source_account_info.try_borrow_mut()?;
        let source_account =
            PodStateWithExtensionsMut::<PodAccount>::unpack(&mut source_account_data)?;

        let (amount, remaining_amount) = match amount {
            PodCOption { option: PodCOption::<u64>::SOME, value: amount } => (
                amount,
                Into::<u64>::into(source_account.base.amount)
                    .checked_sub(amount)
                    .ok_or(TokenError::InsufficientFunds)?,
            ),
            PodCOption { option: PodCOption::<u64>::NONE, value: _ } => {
                (source_account.base.amount.into(), 0)
            }
            _ => return Err(ProgramError::InvalidInstructionData),
        };

        if !source_account.base.is_native() {
            return Err(TokenError::NonNativeNotSupported.into());
        }

        match source_account.base.delegate {
            PodCOption { option: PodCOption::<Address>::SOME, value: delegate }
                if authority_info.address() == &delegate =>
            {
                Self::validate_owner(
                    program_id,
                    &delegate,
                    authority_info,
                    authority_info_data_len,
                    account_info_iter.as_slice(),
                )?;

                let delegated_amount = u64::from(source_account.base.delegated_amount);

                source_account.base.delegated_amount = delegated_amount
                    .checked_sub(amount)
                    .ok_or(TokenError::InsufficientFunds)?
                    .into();

                if u64::from(source_account.base.delegated_amount) == 0 {
                    source_account.base.delegate = PodCOption::none();
                }
            }
            _ => {
                Self::validate_owner(
                    program_id,
                    &source_account.base.owner,
                    authority_info,
                    authority_info_data_len,
                    account_info_iter.as_slice(),
                )?;
            }
        }

        if let Ok(cpi_guard) = source_account.get_extension::<CpiGuard>() {
            if cpi_guard.lock_cpi.into() && in_cpi() {
                return Err(TokenError::CpiGuardTransferBlocked.into());
            }
        }

        if amount != 0 {
            source_account.base.amount = remaining_amount.into();

            // Note: Drop the borrow of `source_account_data` to avoid borrow checker issues.
            drop(source_account_data);

            if source_account_info.address() != destination_account_info.address() {
                let source_starting_lamports = source_account_info.lamports();
                source_account_info.set_lamports(
                    source_starting_lamports.checked_sub(amount).ok_or(TokenError::Overflow)?,
                );

                let destination_starting_lamports = destination_account_info.lamports();
                destination_account_info.set_lamports(
                    destination_starting_lamports
                        .checked_add(amount)
                        .ok_or(TokenError::Overflow)?,
                );
            }
        }

        Ok(())
    }

    /// The size of the batch instruction header.
    ///
    /// The header of each instruction consists of two `u8` values:
    /// * number of the accounts
    /// * length of the instruction data
    const IX_HEADER_SIZE: usize = 2;

    /// Processes an [`Batch`](enum.TokenInstruction.html)
    /// instruction
    pub fn process_batch(
        program_id: &Address,
        mut accounts: &mut [AccountView],
        mut data: &[u8],
    ) -> ProgramResult {
        loop {
            let header = data.get(..Self::IX_HEADER_SIZE).ok_or(TokenError::InvalidInstruction)?;

            let expected_accounts = header[0] as usize;
            let data_offset = Self::IX_HEADER_SIZE + header[1] as usize;

            let ix_accounts =
                accounts.get_mut(..expected_accounts).ok_or(ProgramError::NotEnoughAccountKeys)?;
            let ix_data = data
                .get(Self::IX_HEADER_SIZE..data_offset)
                .ok_or(TokenError::InvalidInstruction)?;

            Self::_process_inner(program_id, ix_accounts, ix_data)?;

            if data_offset == data.len() {
                break;
            }

            accounts = &mut accounts[expected_accounts..];
            data = &data[data_offset..];
        }

        Ok(())
    }

    /// Processes an [`Instruction`](enum.Instruction.html).
    pub fn process(
        program_id: &Address,
        accounts: &mut [AccountView],
        input: &[u8],
    ) -> ProgramResult {
        if let Ok(PodTokenInstruction::Batch) = decode_instruction_type(input) {
            msg!("Instruction: Batch");
            Self::process_batch(program_id, accounts, &input[1..])
        } else {
            Self::_process_inner(program_id, accounts, input)
        }
    }

    fn _process_inner(
        program_id: &Address,
        accounts: &mut [AccountView],
        input: &[u8],
    ) -> ProgramResult {
        if let Ok(instruction_type) = decode_instruction_type(input) {
            match instruction_type {
                PodTokenInstruction::InitializeMint => {
                    msg!("Instruction: InitializeMint");
                    let (data, freeze_authority) =
                        decode_instruction_data_with_coption_pubkey::<InitializeMintData>(input)?;
                    Self::process_initialize_mint(
                        accounts,
                        data.decimals,
                        &data.mint_authority,
                        freeze_authority,
                    )
                }
                PodTokenInstruction::InitializeMint2 => {
                    msg!("Instruction: InitializeMint2");
                    let (data, freeze_authority) =
                        decode_instruction_data_with_coption_pubkey::<InitializeMintData>(input)?;
                    Self::process_initialize_mint2(
                        accounts,
                        data.decimals,
                        &data.mint_authority,
                        freeze_authority,
                    )
                }
                PodTokenInstruction::InitializeAccount => {
                    msg!("Instruction: InitializeAccount");
                    Self::process_initialize_account(accounts)
                }
                PodTokenInstruction::InitializeAccount2 => {
                    msg!("Instruction: InitializeAccount2");
                    let owner = decode_instruction_data::<Address>(input)?;
                    Self::process_initialize_account2(accounts, owner)
                }
                PodTokenInstruction::InitializeAccount3 => {
                    msg!("Instruction: InitializeAccount3");
                    let owner = decode_instruction_data::<Address>(input)?;
                    Self::process_initialize_account3(accounts, owner)
                }
                PodTokenInstruction::InitializeMultisig => {
                    msg!("Instruction: InitializeMultisig");
                    let data = decode_instruction_data::<InitializeMultisigData>(input)?;
                    Self::process_initialize_multisig(accounts, data.m)
                }
                PodTokenInstruction::InitializeMultisig2 => {
                    msg!("Instruction: InitializeMultisig2");
                    let data = decode_instruction_data::<InitializeMultisigData>(input)?;
                    Self::process_initialize_multisig2(accounts, data.m)
                }
                #[allow(deprecated)]
                PodTokenInstruction::Transfer => {
                    msg!("Instruction: Transfer");
                    let data = decode_instruction_data::<AmountData>(input)?;
                    Self::process_transfer(
                        program_id,
                        accounts,
                        data.amount.into(),
                        TransferInstruction::Unchecked,
                    )
                }
                PodTokenInstruction::Approve => {
                    msg!("Instruction: Approve");
                    let data = decode_instruction_data::<AmountData>(input)?;
                    Self::process_approve(
                        program_id,
                        accounts,
                        data.amount.into(),
                        InstructionVariant::Unchecked,
                    )
                }
                PodTokenInstruction::Revoke => {
                    msg!("Instruction: Revoke");
                    Self::process_revoke(program_id, accounts)
                }
                PodTokenInstruction::SetAuthority => {
                    msg!("Instruction: SetAuthority");
                    let (data, new_authority) =
                        decode_instruction_data_with_coption_pubkey::<SetAuthorityData>(input)?;
                    Self::process_set_authority(
                        program_id,
                        accounts,
                        AuthorityType::from(data.authority_type)?,
                        new_authority,
                    )
                }
                PodTokenInstruction::MintTo => {
                    msg!("Instruction: MintTo");
                    let data = decode_instruction_data::<AmountData>(input)?;
                    Self::process_mint_to(
                        program_id,
                        accounts,
                        data.amount.into(),
                        InstructionVariant::Unchecked,
                    )
                }
                PodTokenInstruction::Burn => {
                    msg!("Instruction: Burn");
                    let data = decode_instruction_data::<AmountData>(input)?;
                    Self::process_burn(
                        program_id,
                        accounts,
                        data.amount.into(),
                        BurnInstructionVariant::Standard,
                        InstructionVariant::Unchecked,
                    )
                }
                PodTokenInstruction::CloseAccount => {
                    msg!("Instruction: CloseAccount");
                    Self::process_close_account(program_id, accounts)
                }
                PodTokenInstruction::FreezeAccount => {
                    msg!("Instruction: FreezeAccount");
                    Self::process_toggle_freeze_account(program_id, accounts, true)
                }
                PodTokenInstruction::ThawAccount => {
                    msg!("Instruction: ThawAccount");
                    Self::process_toggle_freeze_account(program_id, accounts, false)
                }
                PodTokenInstruction::TransferChecked => {
                    msg!("Instruction: TransferChecked");
                    let data = decode_instruction_data::<AmountCheckedData>(input)?;
                    Self::process_transfer(
                        program_id,
                        accounts,
                        data.amount.into(),
                        TransferInstruction::Checked { decimals: data.decimals },
                    )
                }
                PodTokenInstruction::ApproveChecked => {
                    msg!("Instruction: ApproveChecked");
                    let data = decode_instruction_data::<AmountCheckedData>(input)?;
                    Self::process_approve(
                        program_id,
                        accounts,
                        data.amount.into(),
                        InstructionVariant::Checked { decimals: data.decimals },
                    )
                }
                PodTokenInstruction::MintToChecked => {
                    msg!("Instruction: MintToChecked");
                    let data = decode_instruction_data::<AmountCheckedData>(input)?;
                    Self::process_mint_to(
                        program_id,
                        accounts,
                        data.amount.into(),
                        InstructionVariant::Checked { decimals: data.decimals },
                    )
                }
                PodTokenInstruction::BurnChecked => {
                    msg!("Instruction: BurnChecked");
                    let data = decode_instruction_data::<AmountCheckedData>(input)?;
                    Self::process_burn(
                        program_id,
                        accounts,
                        data.amount.into(),
                        BurnInstructionVariant::Standard,
                        InstructionVariant::Checked { decimals: data.decimals },
                    )
                }
                PodTokenInstruction::SyncNative => {
                    msg!("Instruction: SyncNative");
                    Self::process_sync_native(accounts)
                }
                PodTokenInstruction::GetAccountDataSize => {
                    msg!("Instruction: GetAccountDataSize");
                    let extension_types = input[1..]
                        .chunks(std::mem::size_of::<ExtensionType>())
                        .map(ExtensionType::try_from)
                        .collect::<Result<Vec<_>, _>>()?;
                    Self::process_get_account_data_size(accounts, &extension_types)
                }
                PodTokenInstruction::InitializeMintCloseAuthority => {
                    msg!("Instruction: InitializeMintCloseAuthority");
                    let (_, close_authority) =
                        decode_instruction_data_with_coption_pubkey::<()>(input)?;
                    Self::process_initialize_mint_close_authority(accounts, close_authority)
                }
                PodTokenInstruction::TransferFeeExtension => {
                    transfer_fee::processor::process_instruction(program_id, accounts, &input[1..])
                }
                PodTokenInstruction::ConfidentialTransferExtension => {
                    confidential_transfer::processor::process_instruction(
                        program_id,
                        accounts,
                        &input[1..],
                    )
                }
                PodTokenInstruction::DefaultAccountStateExtension => {
                    default_account_state::processor::process_instruction(
                        program_id,
                        accounts,
                        &input[1..],
                    )
                }
                PodTokenInstruction::InitializeImmutableOwner => {
                    msg!("Instruction: InitializeImmutableOwner");
                    Self::process_initialize_immutable_owner(accounts)
                }
                PodTokenInstruction::AmountToUiAmount => {
                    msg!("Instruction: AmountToUiAmount");
                    let data = decode_instruction_data::<AmountData>(input)?;
                    Self::process_amount_to_ui_amount(accounts, data.amount.into())
                }
                PodTokenInstruction::UiAmountToAmount => {
                    msg!("Instruction: UiAmountToAmount");
                    let ui_amount = std::str::from_utf8(&input[1..])
                        .map_err(|_| TokenError::InvalidInstruction)?;
                    Self::process_ui_amount_to_amount(accounts, ui_amount)
                }
                PodTokenInstruction::Reallocate => {
                    msg!("Instruction: Reallocate");
                    let extension_types = input[1..]
                        .chunks(std::mem::size_of::<ExtensionType>())
                        .map(ExtensionType::try_from)
                        .collect::<Result<Vec<_>, _>>()?;
                    reallocate::process_reallocate(program_id, accounts, extension_types)
                }
                PodTokenInstruction::MemoTransferExtension => {
                    memo_transfer::processor::process_instruction(program_id, accounts, &input[1..])
                }
                PodTokenInstruction::CreateNativeMint => {
                    msg!("Instruction: CreateNativeMint");
                    Self::process_create_native_mint(accounts)
                }
                PodTokenInstruction::InitializeNonTransferableMint => {
                    msg!("Instruction: InitializeNonTransferableMint");
                    Self::process_initialize_non_transferable_mint(accounts)
                }
                PodTokenInstruction::InterestBearingMintExtension => {
                    interest_bearing_mint::processor::process_instruction(
                        program_id,
                        accounts,
                        &input[1..],
                    )
                }
                PodTokenInstruction::CpiGuardExtension => {
                    cpi_guard::processor::process_instruction(program_id, accounts, &input[1..])
                }
                PodTokenInstruction::InitializePermanentDelegate => {
                    msg!("Instruction: InitializePermanentDelegate");
                    let delegate = decode_instruction_data::<Address>(input)?;
                    Self::process_initialize_permanent_delegate(accounts, delegate)
                }
                PodTokenInstruction::TransferHookExtension => {
                    transfer_hook::processor::process_instruction(program_id, accounts, &input[1..])
                }
                PodTokenInstruction::ConfidentialTransferFeeExtension => {
                    confidential_transfer_fee::processor::process_instruction(
                        program_id,
                        accounts,
                        &input[1..],
                    )
                }
                PodTokenInstruction::WithdrawExcessLamports => {
                    msg!("Instruction: WithdrawExcessLamports");
                    Self::process_withdraw_excess_lamports(program_id, accounts)
                }
                PodTokenInstruction::MetadataPointerExtension => {
                    metadata_pointer::processor::process_instruction(
                        program_id,
                        accounts,
                        &input[1..],
                    )
                }
                PodTokenInstruction::GroupPointerExtension => {
                    group_pointer::processor::process_instruction(program_id, accounts, &input[1..])
                }
                PodTokenInstruction::GroupMemberPointerExtension => {
                    group_member_pointer::processor::process_instruction(
                        program_id,
                        accounts,
                        &input[1..],
                    )
                }
                PodTokenInstruction::ConfidentialMintBurnExtension => {
                    msg!("Instruction: ConfidentialMintBurnExtension");
                    confidential_mint_burn::processor::process_instruction(
                        program_id,
                        accounts,
                        &input[1..],
                    )
                }
                PodTokenInstruction::ScaledUiAmountExtension => {
                    msg!("Instruction: ScaledUiAmountExtension");
                    scaled_ui_amount::processor::process_instruction(
                        program_id,
                        accounts,
                        &input[1..],
                    )
                }
                PodTokenInstruction::PausableExtension => {
                    msg!("Instruction: PausableExtension");
                    pausable::processor::process_instruction(program_id, accounts, &input[1..])
                }
                PodTokenInstruction::PermissionedBurnExtension => {
                    msg!("Instruction: PermissionedBurnExtension");
                    permissioned_burn::processor::process_instruction(
                        program_id,
                        accounts,
                        &input[1..],
                    )
                }
                PodTokenInstruction::UnwrapLamports => {
                    msg!("Instruction: UnwrapLamports");
                    let (_, amount) = decode_instruction_data_with_coption_u64::<()>(input)?;
                    Self::process_unwrap_lamports(program_id, accounts, amount)
                }
                PodTokenInstruction::Batch => Err(TokenError::InvalidInstruction.into()),
            }
        } else if let Ok(instruction) = TokenMetadataInstruction::unpack(input) {
            token_metadata::processor::process_instruction(program_id, accounts, instruction)
        } else if let Ok(instruction) = TokenGroupInstruction::unpack(input) {
            token_group::processor::process_instruction(program_id, accounts, instruction)
        } else {
            Err(TokenError::InvalidInstruction.into())
        }
    }

    /// Validates owner(s) are present. Used for Mints and Accounts only.
    pub fn validate_owner(
        program_id: &Address,
        expected_owner: &Address,
        owner_account_info: &AccountView,
        owner_account_data_len: usize,
        signers: &[AccountView],
    ) -> ProgramResult {
        if expected_owner != owner_account_info.address() {
            return Err(TokenError::OwnerMismatch.into());
        }

        let owned_by_token_program = program_id == &owner_account_info.owner()
            || owner_account_info.owner() == inline_spl_token::id()
            || owner_account_info.owner() == spl_token_2022_interface::id();
        if owned_by_token_program && owner_account_data_len == PodMultisig::SIZE_OF {
            let multisig_data = &owner_account_info.try_borrow()?;
            let multisig = bytemuck::try_from_bytes::<PodMultisig>(multisig_data)
                .map_err(|_| ProgramError::InvalidArgument)?;
            if !bool::from(multisig.is_initialized) {
                return Err(ProgramError::UninitializedAccount);
            }
            let mut num_signers = 0;
            let mut matched = [false; MAX_SIGNERS];
            for signer in signers.iter() {
                for (position, key) in multisig.signers[0..multisig.n as usize].iter().enumerate() {
                    if key == signer.address() && !matched[position] {
                        if !signer.is_signer() {
                            return Err(ProgramError::MissingRequiredSignature);
                        }
                        matched[position] = true;
                        num_signers += 1;
                    }
                }
            }
            if num_signers < multisig.m {
                return Err(ProgramError::MissingRequiredSignature);
            }
            return Ok(());
        } else if !owner_account_info.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "upstream_tests.rs"]
mod tests;
