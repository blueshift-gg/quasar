//! CPI helpers for the legacy Token Metadata instructions. Writable accounts
//! are borrowed until the call completes; typed accounts must be reloaded after
//! a call that may change them. Optional trailing authority-record accounts are
//! omitted, as in the original integration; rent is supplied explicitly.
pub use crate::codec::Data;
use crate::{MetadataProgram, codec};
use quasar_lang::{
    Address, Result,
    cpi::{Cpi, CpiAccount, CpiWithDataLen},
    wrapper::{Program, Wrapper},
};

/// Metaplex `ApproveCollectionAuthority`.
#[allow(clippy::too_many_arguments)]
pub fn approve_collection_authority<'a>(
    program: &'a Program<MetadataProgram>,
    collection_authority_record: &'a mut impl Wrapper,
    new_collection_authority: &'a impl Wrapper,
    update_authority: &'a mut impl Wrapper,
    payer: &'a impl Wrapper,
    metadata: &'a impl Wrapper,
    mint: &'a impl Wrapper,
    system_program: &'a impl Wrapper,
    rent: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 8, 1> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(collection_authority_record),
            CpiAccount::readonly(new_collection_authority),
            CpiAccount::writable(update_authority).signer(),
            // SAFETY: this Metaplex instruction only debits the payer's lamports.
            unsafe { CpiAccount::paying(payer) },
            CpiAccount::readonly(metadata),
            CpiAccount::readonly(mint),
            CpiAccount::readonly(system_program),
            CpiAccount::readonly(rent),
        ],
        [23],
    )
}

/// Metaplex `BurnNft`.
#[allow(clippy::too_many_arguments)]
pub fn burn_nft<'a>(
    program: &'a Program<MetadataProgram>,
    metadata: &'a mut impl Wrapper,
    owner: &'a mut impl Wrapper,
    mint: &'a mut impl Wrapper,
    token_account: &'a mut impl Wrapper,
    master_edition_account: &'a mut impl Wrapper,
    spl_token_program: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 6, 1> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(metadata),
            CpiAccount::writable(owner).signer(),
            CpiAccount::writable(mint),
            CpiAccount::writable(token_account),
            CpiAccount::writable(master_edition_account),
            CpiAccount::readonly(spl_token_program),
        ],
        [29],
    )
}

/// Metaplex `BurnEditionNft`.
#[allow(clippy::too_many_arguments)]
pub fn burn_edition_nft<'a>(
    program: &'a Program<MetadataProgram>,
    metadata: &'a mut impl Wrapper,
    owner: &'a mut impl Wrapper,
    print_edition_mint: &'a mut impl Wrapper,
    master_edition_mint: &'a impl Wrapper,
    print_edition_token_account: &'a mut impl Wrapper,
    master_edition_token_account: &'a impl Wrapper,
    master_edition_account: &'a mut impl Wrapper,
    print_edition_account: &'a mut impl Wrapper,
    edition_marker_account: &'a mut impl Wrapper,
    spl_token_program: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 10, 1> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(metadata),
            CpiAccount::writable(owner).signer(),
            CpiAccount::writable(print_edition_mint),
            CpiAccount::readonly(master_edition_mint),
            CpiAccount::writable(print_edition_token_account),
            CpiAccount::readonly(master_edition_token_account),
            CpiAccount::writable(master_edition_account),
            CpiAccount::writable(print_edition_account),
            CpiAccount::writable(edition_marker_account),
            CpiAccount::readonly(spl_token_program),
        ],
        [37],
    )
}

/// Metaplex `CreateMasterEditionV3`.
#[allow(clippy::too_many_arguments)]
pub fn create_master_edition_v3<'a>(
    program: &'a Program<MetadataProgram>,
    edition: &'a mut impl Wrapper,
    mint: &'a mut impl Wrapper,
    update_authority: &'a impl Wrapper,
    mint_authority: &'a impl Wrapper,
    payer: &'a impl Wrapper,
    metadata: &'a mut impl Wrapper,
    token_program: &'a impl Wrapper,
    system_program: &'a impl Wrapper,
    rent: &'a impl Wrapper,
    max_supply: Option<u64>,
) -> Result<CpiWithDataLen<'a, MetadataProgram, 9, 10>> {
    let (data, len) = codec::option_u64_data(max_supply);
    Cpi::new(
        program,
        [
            CpiAccount::writable(edition),
            CpiAccount::writable(mint),
            CpiAccount::readonly(update_authority).signer(),
            CpiAccount::readonly(mint_authority).signer(),
            // SAFETY: this Metaplex instruction only debits the payer's lamports.
            unsafe { CpiAccount::paying(payer) },
            CpiAccount::writable(metadata),
            CpiAccount::readonly(token_program),
            CpiAccount::readonly(system_program),
            CpiAccount::readonly(rent),
        ],
        data,
    )
    .with_data_len(len)
}

/// Metaplex `CreateMetadataAccountV3`.
#[allow(clippy::too_many_arguments)]
pub fn create_metadata_accounts_v3<'a>(
    program: &'a Program<MetadataProgram>,
    metadata: &'a mut impl Wrapper,
    mint: &'a impl Wrapper,
    mint_authority: &'a impl Wrapper,
    payer: &'a impl Wrapper,
    update_authority: &'a impl Wrapper,
    system_program: &'a impl Wrapper,
    rent: &'a impl Wrapper,
    data: Data<'_>,
    is_mutable: bool,
    update_authority_is_signer: bool,
) -> Result<CpiWithDataLen<'a, MetadataProgram, 7, 512>> {
    let encoded = codec::create(data, is_mutable)?;
    Cpi::new(
        program,
        [
            CpiAccount::writable(metadata),
            CpiAccount::readonly(mint),
            CpiAccount::readonly(mint_authority).signer(),
            // SAFETY: this Metaplex instruction only debits the payer's lamports.
            unsafe { CpiAccount::paying(payer) },
            if update_authority_is_signer {
                CpiAccount::readonly(update_authority).signer()
            } else {
                CpiAccount::readonly(update_authority)
            },
            CpiAccount::readonly(system_program),
            CpiAccount::readonly(rent),
        ],
        encoded.bytes,
    )
    .with_data_len(encoded.len)
}

/// Metaplex `FreezeDelegatedAccount`.
#[allow(clippy::too_many_arguments)]
pub fn freeze_delegated_account<'a>(
    program: &'a Program<MetadataProgram>,
    delegate: &'a mut impl Wrapper,
    token_account: &'a mut impl Wrapper,
    edition: &'a impl Wrapper,
    mint: &'a impl Wrapper,
    token_program: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 5, 1> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(delegate).signer(),
            CpiAccount::writable(token_account),
            CpiAccount::readonly(edition),
            CpiAccount::readonly(mint),
            CpiAccount::readonly(token_program),
        ],
        [26],
    )
}

/// Metaplex `ThawDelegatedAccount`.
#[allow(clippy::too_many_arguments)]
pub fn thaw_delegated_account<'a>(
    program: &'a Program<MetadataProgram>,
    delegate: &'a mut impl Wrapper,
    token_account: &'a mut impl Wrapper,
    edition: &'a impl Wrapper,
    mint: &'a impl Wrapper,
    token_program: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 5, 1> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(delegate).signer(),
            CpiAccount::writable(token_account),
            CpiAccount::readonly(edition),
            CpiAccount::readonly(mint),
            CpiAccount::readonly(token_program),
        ],
        [27],
    )
}

/// Metaplex `MintNewEditionFromMasterEditionViaToken`.
#[allow(clippy::too_many_arguments)]
pub fn mint_new_edition_from_master_edition_via_token<'a>(
    program: &'a Program<MetadataProgram>,
    new_metadata: &'a mut impl Wrapper,
    new_edition: &'a mut impl Wrapper,
    master_edition: &'a mut impl Wrapper,
    new_mint: &'a mut impl Wrapper,
    edition_mark_pda: &'a mut impl Wrapper,
    new_mint_authority: &'a impl Wrapper,
    payer: &'a impl Wrapper,
    token_account_owner: &'a impl Wrapper,
    token_account: &'a impl Wrapper,
    new_metadata_update_authority: &'a impl Wrapper,
    metadata: &'a impl Wrapper,
    token_program: &'a impl Wrapper,
    system_program: &'a impl Wrapper,
    rent: &'a impl Wrapper,
    edition: u64,
) -> Cpi<'a, MetadataProgram, 14, 9> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(new_metadata),
            CpiAccount::writable(new_edition),
            CpiAccount::writable(master_edition),
            CpiAccount::writable(new_mint),
            CpiAccount::writable(edition_mark_pda),
            CpiAccount::readonly(new_mint_authority).signer(),
            // SAFETY: this Metaplex instruction only debits the payer's lamports.
            unsafe { CpiAccount::paying(payer) },
            CpiAccount::readonly(token_account_owner).signer(),
            CpiAccount::readonly(token_account),
            CpiAccount::readonly(new_metadata_update_authority),
            CpiAccount::readonly(metadata),
            CpiAccount::readonly(token_program),
            CpiAccount::readonly(system_program),
            CpiAccount::readonly(rent),
        ],
        codec::u64_data::<11>(edition),
    )
}

/// Metaplex `RemoveCreatorVerification`.
#[allow(clippy::too_many_arguments)]
pub fn remove_creator_verification<'a>(
    program: &'a Program<MetadataProgram>,
    metadata: &'a mut impl Wrapper,
    creator: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 2, 1> {
    Cpi::new(
        program,
        [CpiAccount::writable(metadata), CpiAccount::readonly(creator).signer()],
        [28],
    )
}

/// Metaplex `RevokeCollectionAuthority`.
#[allow(clippy::too_many_arguments)]
pub fn revoke_collection_authority<'a>(
    program: &'a Program<MetadataProgram>,
    collection_authority_record: &'a mut impl Wrapper,
    delegate_authority: &'a mut impl Wrapper,
    revoke_authority: &'a mut impl Wrapper,
    metadata: &'a impl Wrapper,
    mint: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 5, 1> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(collection_authority_record),
            CpiAccount::writable(delegate_authority),
            CpiAccount::writable(revoke_authority).signer(),
            CpiAccount::readonly(metadata),
            CpiAccount::readonly(mint),
        ],
        [24],
    )
}

/// Metaplex `SetAndVerifyCollection`.
#[allow(clippy::too_many_arguments)]
pub fn set_and_verify_collection<'a>(
    program: &'a Program<MetadataProgram>,
    metadata: &'a mut impl Wrapper,
    collection_authority: &'a mut impl Wrapper,
    payer: &'a impl Wrapper,
    update_authority: &'a impl Wrapper,
    collection_mint: &'a impl Wrapper,
    collection: &'a impl Wrapper,
    collection_master_edition_account: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 7, 1> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(metadata),
            CpiAccount::writable(collection_authority).signer(),
            // SAFETY: this Metaplex instruction only debits the payer's lamports.
            unsafe { CpiAccount::paying(payer) },
            CpiAccount::readonly(update_authority),
            CpiAccount::readonly(collection_mint),
            CpiAccount::readonly(collection),
            CpiAccount::readonly(collection_master_edition_account),
        ],
        [25],
    )
}

/// Metaplex `SetAndVerifySizedCollectionItem`.
#[allow(clippy::too_many_arguments)]
pub fn set_and_verify_sized_collection_item<'a>(
    program: &'a Program<MetadataProgram>,
    metadata: &'a mut impl Wrapper,
    collection_authority: &'a impl Wrapper,
    payer: &'a impl Wrapper,
    update_authority: &'a impl Wrapper,
    collection_mint: &'a impl Wrapper,
    collection: &'a mut impl Wrapper,
    collection_master_edition_account: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 7, 1> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(metadata),
            CpiAccount::readonly(collection_authority).signer(),
            // SAFETY: this Metaplex instruction only debits the payer's lamports.
            unsafe { CpiAccount::paying(payer) },
            CpiAccount::readonly(update_authority),
            CpiAccount::readonly(collection_mint),
            CpiAccount::writable(collection),
            CpiAccount::readonly(collection_master_edition_account),
        ],
        [32],
    )
}

/// Metaplex `SetCollectionSize`.
#[allow(clippy::too_many_arguments)]
pub fn set_collection_size<'a>(
    program: &'a Program<MetadataProgram>,
    collection_metadata: &'a mut impl Wrapper,
    collection_authority: &'a mut impl Wrapper,
    collection_mint: &'a impl Wrapper,
    size: u64,
) -> Cpi<'a, MetadataProgram, 3, 9> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(collection_metadata),
            CpiAccount::writable(collection_authority).signer(),
            CpiAccount::readonly(collection_mint),
        ],
        codec::u64_data::<34>(size),
    )
}

/// Metaplex `BubblegumSetCollectionSize`.
#[allow(clippy::too_many_arguments)]
pub fn bubblegum_set_collection_size<'a>(
    program: &'a Program<MetadataProgram>,
    collection_metadata: &'a mut impl Wrapper,
    collection_authority: &'a impl Wrapper,
    collection_mint: &'a impl Wrapper,
    bubblegum_signer: &'a impl Wrapper,
    size: u64,
) -> Cpi<'a, MetadataProgram, 4, 9> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(collection_metadata),
            CpiAccount::readonly(collection_authority).signer(),
            CpiAccount::readonly(collection_mint),
            CpiAccount::readonly(bubblegum_signer).signer(),
        ],
        codec::u64_data::<36>(size),
    )
}

/// Metaplex `SetTokenStandard`.
#[allow(clippy::too_many_arguments)]
pub fn set_token_standard<'a>(
    program: &'a Program<MetadataProgram>,
    metadata: &'a mut impl Wrapper,
    update_authority: &'a impl Wrapper,
    mint: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 3, 1> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(metadata),
            CpiAccount::readonly(update_authority).signer(),
            CpiAccount::readonly(mint),
        ],
        [35],
    )
}

/// Metaplex `SignMetadata`.
#[allow(clippy::too_many_arguments)]
pub fn sign_metadata<'a>(
    program: &'a Program<MetadataProgram>,
    metadata: &'a mut impl Wrapper,
    creator: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 2, 1> {
    Cpi::new(program, [CpiAccount::writable(metadata), CpiAccount::readonly(creator).signer()], [7])
}

/// Metaplex `UnverifyCollection`.
#[allow(clippy::too_many_arguments)]
pub fn unverify_collection<'a>(
    program: &'a Program<MetadataProgram>,
    metadata: &'a mut impl Wrapper,
    collection_authority: &'a mut impl Wrapper,
    collection_mint: &'a impl Wrapper,
    collection: &'a impl Wrapper,
    collection_master_edition_account: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 5, 1> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(metadata),
            CpiAccount::writable(collection_authority).signer(),
            CpiAccount::readonly(collection_mint),
            CpiAccount::readonly(collection),
            CpiAccount::readonly(collection_master_edition_account),
        ],
        [22],
    )
}

/// Metaplex `UnverifySizedCollectionItem`.
#[allow(clippy::too_many_arguments)]
pub fn unverify_sized_collection_item<'a>(
    program: &'a Program<MetadataProgram>,
    metadata: &'a mut impl Wrapper,
    collection_authority: &'a impl Wrapper,
    payer: &'a impl Wrapper,
    collection_mint: &'a impl Wrapper,
    collection: &'a mut impl Wrapper,
    collection_master_edition_account: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 6, 1> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(metadata),
            CpiAccount::readonly(collection_authority).signer(),
            // SAFETY: this Metaplex instruction only debits the payer's lamports.
            unsafe { CpiAccount::paying(payer) },
            CpiAccount::readonly(collection_mint),
            CpiAccount::writable(collection),
            CpiAccount::readonly(collection_master_edition_account),
        ],
        [31],
    )
}

/// Metaplex `UpdateMetadataAccountV2`.
#[allow(clippy::too_many_arguments)]
pub fn update_metadata_accounts_v2<'a>(
    program: &'a Program<MetadataProgram>,
    metadata: &'a mut impl Wrapper,
    update_authority: &'a impl Wrapper,
    data: Option<Data<'_>>,
    new_update_authority: Option<&Address>,
    primary_sale_happened: Option<bool>,
    is_mutable: Option<bool>,
) -> Result<CpiWithDataLen<'a, MetadataProgram, 2, 512>> {
    let encoded = codec::update(data, new_update_authority, primary_sale_happened, is_mutable)?;
    Cpi::new(
        program,
        [CpiAccount::writable(metadata), CpiAccount::readonly(update_authority).signer()],
        encoded.bytes,
    )
    .with_data_len(encoded.len)
}

/// Metaplex `UpdatePrimarySaleHappenedViaToken`.
#[allow(clippy::too_many_arguments)]
pub fn update_primary_sale_happened_via_token<'a>(
    program: &'a Program<MetadataProgram>,
    metadata: &'a mut impl Wrapper,
    owner: &'a impl Wrapper,
    token: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 3, 1> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(metadata),
            CpiAccount::readonly(owner).signer(),
            CpiAccount::readonly(token),
        ],
        [4],
    )
}

/// Metaplex `Utilize`.
#[allow(clippy::too_many_arguments)]
pub fn utilize<'a>(
    program: &'a Program<MetadataProgram>,
    metadata: &'a mut impl Wrapper,
    token_account: &'a mut impl Wrapper,
    mint: &'a mut impl Wrapper,
    use_authority: &'a mut impl Wrapper,
    owner: &'a impl Wrapper,
    token_program: &'a impl Wrapper,
    ata_program: &'a impl Wrapper,
    system_program: &'a impl Wrapper,
    rent: &'a impl Wrapper,
    number_of_uses: u64,
) -> Cpi<'a, MetadataProgram, 9, 9> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(metadata),
            CpiAccount::writable(token_account),
            CpiAccount::writable(mint),
            CpiAccount::writable(use_authority).signer(),
            CpiAccount::readonly(owner),
            CpiAccount::readonly(token_program),
            CpiAccount::readonly(ata_program),
            CpiAccount::readonly(system_program),
            CpiAccount::readonly(rent),
        ],
        codec::u64_data::<19>(number_of_uses),
    )
}

/// Metaplex `VerifyCollection`.
#[allow(clippy::too_many_arguments)]
pub fn verify_collection<'a>(
    program: &'a Program<MetadataProgram>,
    metadata: &'a mut impl Wrapper,
    collection_authority: &'a mut impl Wrapper,
    payer: &'a impl Wrapper,
    collection_mint: &'a impl Wrapper,
    collection: &'a impl Wrapper,
    collection_master_edition_account: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 6, 1> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(metadata),
            CpiAccount::writable(collection_authority).signer(),
            // SAFETY: this Metaplex instruction only debits the payer's lamports.
            unsafe { CpiAccount::paying(payer) },
            CpiAccount::readonly(collection_mint),
            CpiAccount::readonly(collection),
            CpiAccount::readonly(collection_master_edition_account),
        ],
        [18],
    )
}

/// Metaplex `VerifySizedCollectionItem`.
#[allow(clippy::too_many_arguments)]
pub fn verify_sized_collection_item<'a>(
    program: &'a Program<MetadataProgram>,
    metadata: &'a mut impl Wrapper,
    collection_authority: &'a impl Wrapper,
    payer: &'a impl Wrapper,
    collection_mint: &'a impl Wrapper,
    collection: &'a mut impl Wrapper,
    collection_master_edition_account: &'a impl Wrapper,
) -> Cpi<'a, MetadataProgram, 6, 1> {
    Cpi::new(
        program,
        [
            CpiAccount::writable(metadata),
            CpiAccount::readonly(collection_authority).signer(),
            // SAFETY: this Metaplex instruction only debits the payer's lamports.
            unsafe { CpiAccount::paying(payer) },
            CpiAccount::readonly(collection_mint),
            CpiAccount::writable(collection),
            CpiAccount::readonly(collection_master_edition_account),
        ],
        [30],
    )
}
