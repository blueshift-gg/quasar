mod approve_collection;
mod burn;
mod create_master_edition;
mod create_metadata;
mod freeze_thaw;
mod mint_edition;
mod remove_creator;
mod revoke_collection;
mod set_and_verify_collection;
mod set_collection_size;
mod set_token_standard;
mod sign_metadata;
mod unverify_collection;
mod update_metadata;
mod update_primary_sale;
mod utilize;
mod verify_collection;

use {
    crate::codec::BorshCpiEncode,
    quasar_lang::{
        cpi::{CpiCall, DynCpiCall},
        prelude::*,
    },
};

// Metaplex-enforced maximum field lengths.
const MAX_NAME_LEN: usize = 32;
const MAX_SYMBOL_LEN: usize = 10;
const MAX_URI_LEN: usize = 200;

/// Trait for types that can execute Metaplex Token Metadata CPI calls.
///
/// Implemented by [`crate::MetadataProgram`].
pub trait MetadataCpi: AsAccountView {
    // -----------------------------------------------------------------------
    // Variable-length instructions (DynCpiCall)
    // -----------------------------------------------------------------------

    /// Create a metadata account for an SPL Token mint.
    ///
    /// Accounts (7): metadata, mint, mint_authority, payer, update_authority,
    /// system_program, rent.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn create_metadata_accounts_v3<'a>(
        &'a self,
        metadata: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
        mint_authority: &'a impl AsAccountView,
        payer: &'a impl AsAccountView,
        update_authority: &'a impl AsAccountView,
        system_program: &'a impl AsAccountView,
        rent: &'a impl AsAccountView,
        name: impl BorshCpiEncode,
        symbol: impl BorshCpiEncode,
        uri: impl BorshCpiEncode,
        seller_fee_basis_points: u16,
        is_mutable: bool,
        update_authority_is_signer: bool,
    ) -> Result<DynCpiCall<'a, 7, 512>, ProgramError> {
        create_metadata::create_metadata_accounts_v3(
            self.to_account_view(),
            metadata.to_account_view(),
            mint.to_account_view(),
            mint_authority.to_account_view(),
            payer.to_account_view(),
            update_authority.to_account_view(),
            system_program.to_account_view(),
            rent.to_account_view(),
            name,
            symbol,
            uri,
            seller_fee_basis_points,
            is_mutable,
            update_authority_is_signer,
        )
    }

    /// Update a metadata account.
    ///
    /// Accounts (2): metadata, update_authority.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn update_metadata_accounts_v2<'a>(
        &'a self,
        metadata: &'a impl AsAccountView,
        update_authority: &'a impl AsAccountView,
        new_update_authority: Option<&Address>,
        name: Option<&[u8]>,
        symbol: Option<&[u8]>,
        uri: Option<&[u8]>,
        seller_fee_basis_points: Option<u16>,
        primary_sale_happened: Option<bool>,
        is_mutable: Option<bool>,
    ) -> Result<DynCpiCall<'a, 2, 512>, ProgramError> {
        update_metadata::update_metadata_accounts_v2(
            self.to_account_view(),
            metadata.to_account_view(),
            update_authority.to_account_view(),
            new_update_authority,
            name,
            symbol,
            uri,
            seller_fee_basis_points,
            primary_sale_happened,
            is_mutable,
        )
    }

    // -----------------------------------------------------------------------
    // Fixed-length instructions (CpiCall)
    // -----------------------------------------------------------------------

    /// Create a master edition account, making the mint a verified NFT.
    ///
    /// Accounts (9): edition, mint, update_authority, mint_authority, payer,
    /// metadata, token_program, system_program, rent.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn create_master_edition_v3<'a>(
        &'a self,
        edition: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
        update_authority: &'a impl AsAccountView,
        mint_authority: &'a impl AsAccountView,
        payer: &'a impl AsAccountView,
        metadata: &'a impl AsAccountView,
        token_program: &'a impl AsAccountView,
        system_program: &'a impl AsAccountView,
        rent: &'a impl AsAccountView,
        max_supply: Option<u64>,
    ) -> CpiCall<'a, 9, 10> {
        create_master_edition::create_master_edition_v3(
            self.to_account_view(),
            edition.to_account_view(),
            mint.to_account_view(),
            update_authority.to_account_view(),
            mint_authority.to_account_view(),
            payer.to_account_view(),
            metadata.to_account_view(),
            token_program.to_account_view(),
            system_program.to_account_view(),
            rent.to_account_view(),
            max_supply,
        )
    }

    /// Mint a new edition from a master edition via a token holder.
    ///
    /// Accounts (14): new_metadata, new_edition, master_edition, new_mint,
    /// edition_mark_pda, new_mint_authority, payer, token_account_owner,
    /// token_account, new_metadata_update_authority, metadata, token_program,
    /// system_program, rent.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn mint_new_edition_from_master_edition_via_token<'a>(
        &'a self,
        new_metadata: &'a impl AsAccountView,
        new_edition: &'a impl AsAccountView,
        master_edition: &'a impl AsAccountView,
        new_mint: &'a impl AsAccountView,
        edition_mark_pda: &'a impl AsAccountView,
        new_mint_authority: &'a impl AsAccountView,
        payer: &'a impl AsAccountView,
        token_account_owner: &'a impl AsAccountView,
        token_account: &'a impl AsAccountView,
        new_metadata_update_authority: &'a impl AsAccountView,
        metadata: &'a impl AsAccountView,
        token_program: &'a impl AsAccountView,
        system_program: &'a impl AsAccountView,
        rent: &'a impl AsAccountView,
        edition: u64,
    ) -> CpiCall<'a, 14, 9> {
        mint_edition::mint_new_edition_from_master_edition_via_token(
            self.to_account_view(),
            new_metadata.to_account_view(),
            new_edition.to_account_view(),
            master_edition.to_account_view(),
            new_mint.to_account_view(),
            edition_mark_pda.to_account_view(),
            new_mint_authority.to_account_view(),
            payer.to_account_view(),
            token_account_owner.to_account_view(),
            token_account.to_account_view(),
            new_metadata_update_authority.to_account_view(),
            metadata.to_account_view(),
            token_program.to_account_view(),
            system_program.to_account_view(),
            rent.to_account_view(),
            edition,
        )
    }

    /// Sign metadata as a creator.
    ///
    /// Accounts (2): creator, metadata.
    #[inline(always)]
    fn sign_metadata<'a>(
        &'a self,
        creator: &'a impl AsAccountView,
        metadata: &'a impl AsAccountView,
    ) -> CpiCall<'a, 2, 1> {
        sign_metadata::sign_metadata(
            self.to_account_view(),
            creator.to_account_view(),
            metadata.to_account_view(),
        )
    }

    /// Remove creator verification from metadata.
    ///
    /// Accounts (2): creator, metadata.
    #[inline(always)]
    fn remove_creator_verification<'a>(
        &'a self,
        creator: &'a impl AsAccountView,
        metadata: &'a impl AsAccountView,
    ) -> CpiCall<'a, 2, 1> {
        remove_creator::remove_creator_verification(
            self.to_account_view(),
            creator.to_account_view(),
            metadata.to_account_view(),
        )
    }

    /// Update primary sale happened flag via token holder.
    ///
    /// Accounts (3): metadata, owner, token.
    #[inline(always)]
    fn update_primary_sale_happened_via_token<'a>(
        &'a self,
        metadata: &'a impl AsAccountView,
        owner: &'a impl AsAccountView,
        token: &'a impl AsAccountView,
    ) -> CpiCall<'a, 3, 1> {
        update_primary_sale::update_primary_sale_happened_via_token(
            self.to_account_view(),
            metadata.to_account_view(),
            owner.to_account_view(),
            token.to_account_view(),
        )
    }

    /// Verify a collection item.
    ///
    /// Accounts (6): metadata, collection_authority, payer, collection_mint,
    /// collection_metadata, collection_master_edition.
    #[inline(always)]
    fn verify_collection<'a>(
        &'a self,
        metadata: &'a impl AsAccountView,
        collection_authority: &'a impl AsAccountView,
        payer: &'a impl AsAccountView,
        collection_mint: &'a impl AsAccountView,
        collection_metadata: &'a impl AsAccountView,
        collection_master_edition: &'a impl AsAccountView,
    ) -> CpiCall<'a, 6, 1> {
        verify_collection::verify_collection(
            self.to_account_view(),
            metadata.to_account_view(),
            collection_authority.to_account_view(),
            payer.to_account_view(),
            collection_mint.to_account_view(),
            collection_metadata.to_account_view(),
            collection_master_edition.to_account_view(),
        )
    }

    /// Verify a sized collection item.
    ///
    /// Accounts (6): metadata, collection_authority, payer, collection_mint,
    /// collection_metadata, collection_master_edition.
    #[inline(always)]
    fn verify_sized_collection_item<'a>(
        &'a self,
        metadata: &'a impl AsAccountView,
        collection_authority: &'a impl AsAccountView,
        payer: &'a impl AsAccountView,
        collection_mint: &'a impl AsAccountView,
        collection_metadata: &'a impl AsAccountView,
        collection_master_edition: &'a impl AsAccountView,
    ) -> CpiCall<'a, 6, 1> {
        verify_collection::verify_sized_collection_item(
            self.to_account_view(),
            metadata.to_account_view(),
            collection_authority.to_account_view(),
            payer.to_account_view(),
            collection_mint.to_account_view(),
            collection_metadata.to_account_view(),
            collection_master_edition.to_account_view(),
        )
    }

    /// Unverify a collection item.
    ///
    /// Accounts (5): metadata, collection_authority, collection_mint,
    /// collection_metadata, collection_master_edition.
    #[inline(always)]
    fn unverify_collection<'a>(
        &'a self,
        metadata: &'a impl AsAccountView,
        collection_authority: &'a impl AsAccountView,
        collection_mint: &'a impl AsAccountView,
        collection_metadata: &'a impl AsAccountView,
        collection_master_edition: &'a impl AsAccountView,
    ) -> CpiCall<'a, 5, 1> {
        unverify_collection::unverify_collection(
            self.to_account_view(),
            metadata.to_account_view(),
            collection_authority.to_account_view(),
            collection_mint.to_account_view(),
            collection_metadata.to_account_view(),
            collection_master_edition.to_account_view(),
        )
    }

    /// Unverify a sized collection item.
    ///
    /// Accounts (6): metadata, collection_authority, payer, collection_mint,
    /// collection_metadata, collection_master_edition.
    #[inline(always)]
    fn unverify_sized_collection_item<'a>(
        &'a self,
        metadata: &'a impl AsAccountView,
        collection_authority: &'a impl AsAccountView,
        payer: &'a impl AsAccountView,
        collection_mint: &'a impl AsAccountView,
        collection_metadata: &'a impl AsAccountView,
        collection_master_edition: &'a impl AsAccountView,
    ) -> CpiCall<'a, 6, 1> {
        unverify_collection::unverify_sized_collection_item(
            self.to_account_view(),
            metadata.to_account_view(),
            collection_authority.to_account_view(),
            payer.to_account_view(),
            collection_mint.to_account_view(),
            collection_metadata.to_account_view(),
            collection_master_edition.to_account_view(),
        )
    }

    /// Set and verify a collection item.
    ///
    /// Accounts (7): metadata, collection_authority, payer, update_authority,
    /// collection_mint, collection_metadata, collection_master_edition.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn set_and_verify_collection<'a>(
        &'a self,
        metadata: &'a impl AsAccountView,
        collection_authority: &'a impl AsAccountView,
        payer: &'a impl AsAccountView,
        update_authority: &'a impl AsAccountView,
        collection_mint: &'a impl AsAccountView,
        collection_metadata: &'a impl AsAccountView,
        collection_master_edition: &'a impl AsAccountView,
    ) -> CpiCall<'a, 7, 1> {
        set_and_verify_collection::set_and_verify_collection(
            self.to_account_view(),
            metadata.to_account_view(),
            collection_authority.to_account_view(),
            payer.to_account_view(),
            update_authority.to_account_view(),
            collection_mint.to_account_view(),
            collection_metadata.to_account_view(),
            collection_master_edition.to_account_view(),
        )
    }

    /// Set and verify a sized collection item.
    ///
    /// Accounts (7): metadata, collection_authority, payer, update_authority,
    /// collection_mint, collection_metadata, collection_master_edition.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn set_and_verify_sized_collection_item<'a>(
        &'a self,
        metadata: &'a impl AsAccountView,
        collection_authority: &'a impl AsAccountView,
        payer: &'a impl AsAccountView,
        update_authority: &'a impl AsAccountView,
        collection_mint: &'a impl AsAccountView,
        collection_metadata: &'a impl AsAccountView,
        collection_master_edition: &'a impl AsAccountView,
    ) -> CpiCall<'a, 7, 1> {
        set_and_verify_collection::set_and_verify_sized_collection_item(
            self.to_account_view(),
            metadata.to_account_view(),
            collection_authority.to_account_view(),
            payer.to_account_view(),
            update_authority.to_account_view(),
            collection_mint.to_account_view(),
            collection_metadata.to_account_view(),
            collection_master_edition.to_account_view(),
        )
    }

    /// Approve a collection authority.
    ///
    /// Accounts (6): collection_authority_record, new_collection_authority,
    /// update_authority, payer, metadata, mint.
    #[inline(always)]
    fn approve_collection_authority<'a>(
        &'a self,
        collection_authority_record: &'a impl AsAccountView,
        new_collection_authority: &'a impl AsAccountView,
        update_authority: &'a impl AsAccountView,
        payer: &'a impl AsAccountView,
        metadata: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
    ) -> CpiCall<'a, 6, 1> {
        approve_collection::approve_collection_authority(
            self.to_account_view(),
            collection_authority_record.to_account_view(),
            new_collection_authority.to_account_view(),
            update_authority.to_account_view(),
            payer.to_account_view(),
            metadata.to_account_view(),
            mint.to_account_view(),
        )
    }

    /// Revoke a collection authority.
    ///
    /// Accounts (5): collection_authority_record, delegate_authority,
    /// revoke_authority, metadata, mint.
    #[inline(always)]
    fn revoke_collection_authority<'a>(
        &'a self,
        collection_authority_record: &'a impl AsAccountView,
        delegate_authority: &'a impl AsAccountView,
        revoke_authority: &'a impl AsAccountView,
        metadata: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
    ) -> CpiCall<'a, 5, 1> {
        revoke_collection::revoke_collection_authority(
            self.to_account_view(),
            collection_authority_record.to_account_view(),
            delegate_authority.to_account_view(),
            revoke_authority.to_account_view(),
            metadata.to_account_view(),
            mint.to_account_view(),
        )
    }

    /// Freeze a delegated token account.
    ///
    /// Accounts (5): delegate, token_account, edition, mint, token_program.
    #[inline(always)]
    fn freeze_delegated_account<'a>(
        &'a self,
        delegate: &'a impl AsAccountView,
        token_account: &'a impl AsAccountView,
        edition: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
        token_program: &'a impl AsAccountView,
    ) -> CpiCall<'a, 5, 1> {
        freeze_thaw::freeze_delegated_account(
            self.to_account_view(),
            delegate.to_account_view(),
            token_account.to_account_view(),
            edition.to_account_view(),
            mint.to_account_view(),
            token_program.to_account_view(),
        )
    }

    /// Thaw a delegated token account.
    ///
    /// Accounts (5): delegate, token_account, edition, mint, token_program.
    #[inline(always)]
    fn thaw_delegated_account<'a>(
        &'a self,
        delegate: &'a impl AsAccountView,
        token_account: &'a impl AsAccountView,
        edition: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
        token_program: &'a impl AsAccountView,
    ) -> CpiCall<'a, 5, 1> {
        freeze_thaw::thaw_delegated_account(
            self.to_account_view(),
            delegate.to_account_view(),
            token_account.to_account_view(),
            edition.to_account_view(),
            mint.to_account_view(),
            token_program.to_account_view(),
        )
    }

    /// Burn an NFT (metadata, edition, token, mint).
    ///
    /// Accounts (6): metadata, owner, mint, token, edition, spl_token.
    #[inline(always)]
    fn burn_nft<'a>(
        &'a self,
        metadata: &'a impl AsAccountView,
        owner: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
        token: &'a impl AsAccountView,
        edition: &'a impl AsAccountView,
        spl_token: &'a impl AsAccountView,
    ) -> CpiCall<'a, 6, 1> {
        burn::burn_nft(
            self.to_account_view(),
            metadata.to_account_view(),
            owner.to_account_view(),
            mint.to_account_view(),
            token.to_account_view(),
            edition.to_account_view(),
            spl_token.to_account_view(),
        )
    }

    /// Burn an edition NFT.
    ///
    /// Accounts (10): metadata, owner, print_edition_mint, master_edition_mint,
    /// print_edition_token, master_edition_token, master_edition,
    /// print_edition, edition_marker, spl_token.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn burn_edition_nft<'a>(
        &'a self,
        metadata: &'a impl AsAccountView,
        owner: &'a impl AsAccountView,
        print_edition_mint: &'a impl AsAccountView,
        master_edition_mint: &'a impl AsAccountView,
        print_edition_token: &'a impl AsAccountView,
        master_edition_token: &'a impl AsAccountView,
        master_edition: &'a impl AsAccountView,
        print_edition: &'a impl AsAccountView,
        edition_marker: &'a impl AsAccountView,
        spl_token: &'a impl AsAccountView,
    ) -> CpiCall<'a, 10, 1> {
        burn::burn_edition_nft(
            self.to_account_view(),
            metadata.to_account_view(),
            owner.to_account_view(),
            print_edition_mint.to_account_view(),
            master_edition_mint.to_account_view(),
            print_edition_token.to_account_view(),
            master_edition_token.to_account_view(),
            master_edition.to_account_view(),
            print_edition.to_account_view(),
            edition_marker.to_account_view(),
            spl_token.to_account_view(),
        )
    }

    /// Set the collection size on a collection metadata.
    ///
    /// Accounts (3): metadata, update_authority, mint.
    #[inline(always)]
    fn set_collection_size<'a>(
        &'a self,
        metadata: &'a impl AsAccountView,
        update_authority: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
        size: u64,
    ) -> CpiCall<'a, 3, 9> {
        set_collection_size::set_collection_size(
            self.to_account_view(),
            metadata.to_account_view(),
            update_authority.to_account_view(),
            mint.to_account_view(),
            size,
        )
    }

    /// Set collection size via Bubblegum program.
    ///
    /// Accounts (4): metadata, update_authority, mint, bubblegum_signer.
    #[inline(always)]
    fn bubblegum_set_collection_size<'a>(
        &'a self,
        metadata: &'a impl AsAccountView,
        update_authority: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
        bubblegum_signer: &'a impl AsAccountView,
        size: u64,
    ) -> CpiCall<'a, 4, 9> {
        set_collection_size::bubblegum_set_collection_size(
            self.to_account_view(),
            metadata.to_account_view(),
            update_authority.to_account_view(),
            mint.to_account_view(),
            bubblegum_signer.to_account_view(),
            size,
        )
    }

    /// Set the token standard on a metadata account.
    ///
    /// Accounts (3): metadata, update_authority, mint.
    #[inline(always)]
    fn set_token_standard<'a>(
        &'a self,
        metadata: &'a impl AsAccountView,
        update_authority: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
    ) -> CpiCall<'a, 3, 1> {
        set_token_standard::set_token_standard(
            self.to_account_view(),
            metadata.to_account_view(),
            update_authority.to_account_view(),
            mint.to_account_view(),
        )
    }

    /// Use/utilize an NFT.
    ///
    /// Accounts (5): metadata, token_account, mint, use_authority, owner.
    #[inline(always)]
    fn utilize<'a>(
        &'a self,
        metadata: &'a impl AsAccountView,
        token_account: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
        use_authority: &'a impl AsAccountView,
        owner: &'a impl AsAccountView,
        number_of_uses: u64,
    ) -> CpiCall<'a, 5, 9> {
        utilize::utilize(
            self.to_account_view(),
            metadata.to_account_view(),
            token_account.to_account_view(),
            mint.to_account_view(),
            use_authority.to_account_view(),
            owner.to_account_view(),
            number_of_uses,
        )
    }
}

impl MetadataCpi for crate::MetadataProgram {}

impl MetadataCpi for AccountView {}
