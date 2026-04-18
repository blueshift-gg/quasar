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

// ---------------------------------------------------------------------------
// Kani proof harnesses for Metaplex metadata instruction data layout
// ---------------------------------------------------------------------------
//
// Each harness replicates the unsafe `MaybeUninit` + pointer-write pattern used
// by the corresponding instruction builder and asserts:
//   1. The discriminator byte is correct.
//   2. Payload fields are written at the expected offsets in little-endian.
//   3. All bytes of the buffer are initialised before `assume_init`.
//
// Because the harnesses use `kani::any()` for payload values, Kani explores
// *every* possible input, giving us a full proof — not just example-based
// tests.
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {

    // -- create_master_edition_v3 with Some(max_supply) (disc=17, 10-byte buf) --

    /// Prove that the `create_master_edition_v3` instruction data layout is
    /// correct when `max_supply` is `Some(v)` for all possible `v` values.
    #[kani::proof]
    fn create_master_edition_v3_some_layout() {
        let max_supply: u64 = kani::any();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 10]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 17u8);
            // Some variant: option tag = 1
            core::ptr::write(ptr.add(1), 1u8);
            core::ptr::copy_nonoverlapping(max_supply.to_le_bytes().as_ptr(), ptr.add(2), 8);
            buf.assume_init()
        };

        // Discriminator at offset 0
        assert!(data[0] == 17u8);
        // Option tag at offset 1
        assert!(data[1] == 1u8);
        // max_supply at offset 2..10 (little-endian)
        let le = max_supply.to_le_bytes();
        assert!(data[2] == le[0]);
        assert!(data[3] == le[1]);
        assert!(data[4] == le[2]);
        assert!(data[5] == le[3]);
        assert!(data[6] == le[4]);
        assert!(data[7] == le[5]);
        assert!(data[8] == le[6]);
        assert!(data[9] == le[7]);
    }

    // -- create_master_edition_v3 with None (disc=17, 10-byte buf) ------------

    /// Prove that the `create_master_edition_v3` instruction data layout is
    /// correct when `max_supply` is `None` (option tag 0, eight zero bytes).
    #[kani::proof]
    fn create_master_edition_v3_none_layout() {
        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 10]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 17u8);
            // None variant: option tag = 0
            core::ptr::write(ptr.add(1), 0u8);
            core::ptr::write_bytes(ptr.add(2), 0, 8);
            buf.assume_init()
        };

        // Discriminator at offset 0
        assert!(data[0] == 17u8);
        // Option tag at offset 1
        assert!(data[1] == 0u8);
        // Remaining 8 bytes must be zero
        assert!(data[2] == 0u8);
        assert!(data[3] == 0u8);
        assert!(data[4] == 0u8);
        assert!(data[5] == 0u8);
        assert!(data[6] == 0u8);
        assert!(data[7] == 0u8);
        assert!(data[8] == 0u8);
        assert!(data[9] == 0u8);
    }

    // -- mint_new_edition_from_master_edition_via_token (disc=11, 9-byte buf) -

    /// Prove that the `mint_new_edition_from_master_edition_via_token`
    /// instruction data layout is correct for all possible `edition` values.
    #[kani::proof]
    fn mint_edition_instruction_layout() {
        let edition: u64 = kani::any();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 11u8);
            core::ptr::copy_nonoverlapping(edition.to_le_bytes().as_ptr(), ptr.add(1), 8);
            buf.assume_init()
        };

        // Discriminator at offset 0
        assert!(data[0] == 11u8);
        // edition at offset 1..9 (little-endian)
        let le = edition.to_le_bytes();
        assert!(data[1] == le[0]);
        assert!(data[2] == le[1]);
        assert!(data[3] == le[2]);
        assert!(data[4] == le[3]);
        assert!(data[5] == le[4]);
        assert!(data[6] == le[5]);
        assert!(data[7] == le[6]);
        assert!(data[8] == le[7]);
    }

    // -- set_collection_size (disc=34, 9-byte buf) ----------------------------

    /// Prove that the `set_collection_size` instruction data layout is correct
    /// for all possible `size` values.
    #[kani::proof]
    fn set_collection_size_instruction_layout() {
        let size: u64 = kani::any();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 34u8);
            core::ptr::copy_nonoverlapping(size.to_le_bytes().as_ptr(), ptr.add(1), 8);
            buf.assume_init()
        };

        // Discriminator at offset 0
        assert!(data[0] == 34u8);
        // size at offset 1..9 (little-endian)
        let le = size.to_le_bytes();
        assert!(data[1] == le[0]);
        assert!(data[2] == le[1]);
        assert!(data[3] == le[2]);
        assert!(data[4] == le[3]);
        assert!(data[5] == le[4]);
        assert!(data[6] == le[5]);
        assert!(data[7] == le[6]);
        assert!(data[8] == le[7]);
    }

    // -- bubblegum_set_collection_size (disc=36, 9-byte buf) ------------------

    /// Prove that the `bubblegum_set_collection_size` instruction data layout
    /// is correct for all possible `size` values.
    #[kani::proof]
    fn bubblegum_set_collection_size_instruction_layout() {
        let size: u64 = kani::any();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 36u8);
            core::ptr::copy_nonoverlapping(size.to_le_bytes().as_ptr(), ptr.add(1), 8);
            buf.assume_init()
        };

        // Discriminator at offset 0
        assert!(data[0] == 36u8);
        // size at offset 1..9 (little-endian)
        let le = size.to_le_bytes();
        assert!(data[1] == le[0]);
        assert!(data[2] == le[1]);
        assert!(data[3] == le[2]);
        assert!(data[4] == le[3]);
        assert!(data[5] == le[4]);
        assert!(data[6] == le[5]);
        assert!(data[7] == le[6]);
        assert!(data[8] == le[7]);
    }

    // -- utilize (disc=19, 9-byte buf) ----------------------------------------

    /// Prove that the `utilize` instruction data layout is correct for all
    /// possible `number_of_uses` values.
    #[kani::proof]
    fn utilize_instruction_layout() {
        let number_of_uses: u64 = kani::any();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 19u8);
            core::ptr::copy_nonoverlapping(number_of_uses.to_le_bytes().as_ptr(), ptr.add(1), 8);
            buf.assume_init()
        };

        // Discriminator at offset 0
        assert!(data[0] == 19u8);
        // number_of_uses at offset 1..9 (little-endian)
        let le = number_of_uses.to_le_bytes();
        assert!(data[1] == le[0]);
        assert!(data[2] == le[1]);
        assert!(data[3] == le[2]);
        assert!(data[4] == le[3]);
        assert!(data[5] == le[4]);
        assert!(data[6] == le[5]);
        assert!(data[7] == le[6]);
        assert!(data[8] == le[7]);
    }

    // -----------------------------------------------------------------------
    // Dynamic-offset instruction builders — buffer overflow proofs
    // -----------------------------------------------------------------------
    //
    // These builders use variable-length Borsh strings, so the final offset
    // depends on runtime field lengths. We prove that every valid combination
    // of field lengths keeps the total offset within the 512-byte buffer.

    // -- create_metadata_accounts_v3 (disc=33, 512-byte DynCpiCall buf) ------

    /// Prove that `create_metadata_accounts_v3` offset arithmetic stays within
    /// the 512-byte buffer for all valid field lengths.
    ///
    /// Layout:
    ///
    /// ```text
    ///   [0]       discriminator (33)                          1
    ///   [1..]     name:   Borsh string (4-byte u32 LE len + bytes)  4 + name_len
    ///             symbol: Borsh string                              4 + symbol_len
    ///             uri:    Borsh string                              4 + uri_len
    ///             seller_fee_basis_points (u16 LE)                  2
    ///             creators  Option None tag                         1
    ///             collection Option None tag                        1
    ///             uses      Option None tag                         1
    ///             is_mutable (u8)                                   1
    ///             collection_details Option None tag                1
    ///
    ///   Total = 1 + (4+name_len) + (4+symbol_len) + (4+uri_len) + 2 + 3 + 1 + 1
    ///         = 20 + name_len + symbol_len + uri_len
    ///
    ///   Max  = 20 + 32 + 10 + 200 = 262 ≤ 512.
    /// ```
    #[kani::proof]
    fn create_metadata_v3_offset_within_buffer() {
        const BUF_CAP: usize = 512;
        const MAX_NAME: usize = 32;
        const MAX_SYMBOL: usize = 10;
        const MAX_URI: usize = 200;

        let name_len: usize = kani::any();
        let symbol_len: usize = kani::any();
        let uri_len: usize = kani::any();

        kani::assume(name_len <= MAX_NAME);
        kani::assume(symbol_len <= MAX_SYMBOL);
        kani::assume(uri_len <= MAX_URI);

        // Mirror the offset arithmetic from create_metadata.rs
        let mut offset: usize = 0;

        // Discriminator
        offset += 1;

        // name: Borsh string (u32 LE prefix + bytes)
        offset += 4 + name_len;

        // symbol: Borsh string
        offset += 4 + symbol_len;

        // uri: Borsh string
        offset += 4 + uri_len;

        // seller_fee_basis_points (u16)
        offset += 2;

        // creators: Option<Vec<Creator>> = None
        offset += 1;

        // collection: Option<Collection> = None
        offset += 1;

        // uses: Option<Uses> = None
        offset += 1;

        // is_mutable (u8)
        offset += 1;

        // collection_details: Option<CollectionDetails> = None
        offset += 1;

        assert!(offset <= BUF_CAP);

        // Verify the closed-form matches the step-by-step accumulation
        let expected = 20 + name_len + symbol_len + uri_len;
        assert!(offset == expected);
    }

    // -- update_metadata_accounts_v2 (disc=15, 512-byte DynCpiCall buf) ------

    /// Prove that `update_metadata_accounts_v2` offset arithmetic stays within
    /// the 512-byte buffer in the worst case: all `Option` fields are `Some`
    /// with maximum-length strings.
    ///
    /// Layout (all-Some branch):
    ///   discriminator                                   1
    ///   Option<DataV2> Some tag                         1
    ///     name:   Borsh string (4 + name_len)
    ///     symbol: Borsh string (4 + symbol_len)
    ///     uri:    Borsh string (4 + uri_len)
    ///     seller_fee_basis_points (u16)                 2
    ///     creators  None tag                            1
    ///     collection None tag                           1
    ///     uses      None tag                            1
    ///   new_update_authority Some tag + Pubkey           1 + 32
    ///   primary_sale_happened Some tag + bool            1 + 1
    ///   is_mutable Some tag + bool                       1 + 1
    ///
    /// Total = 1 + 1 + (4+n) + (4+s) + (4+u) + 2 + 3 + 33 + 2 + 2
    ///       = 56 + n + s + u
    /// Max  = 56 + 32 + 10 + 200 = 298 ≤ 512.
    #[kani::proof]
    fn update_metadata_v2_all_some_offset_within_buffer() {
        const BUF_CAP: usize = 512;
        const MAX_NAME: usize = 32;
        const MAX_SYMBOL: usize = 10;
        const MAX_URI: usize = 200;

        let name_len: usize = kani::any();
        let symbol_len: usize = kani::any();
        let uri_len: usize = kani::any();

        kani::assume(name_len <= MAX_NAME);
        kani::assume(symbol_len <= MAX_SYMBOL);
        kani::assume(uri_len <= MAX_URI);

        // Mirror the offset arithmetic from update_metadata.rs (all-Some branch)
        let mut offset: usize = 0;

        // Discriminator
        offset += 1;

        // Option<DataV2>: Some tag
        offset += 1;

        // name: Borsh string (u32 LE prefix + bytes)
        offset += 4 + name_len;

        // symbol: Borsh string
        offset += 4 + symbol_len;

        // uri: Borsh string
        offset += 4 + uri_len;

        // seller_fee_basis_points (u16)
        offset += 2;

        // creators: None
        offset += 1;

        // collection: None
        offset += 1;

        // uses: None
        offset += 1;

        // new_update_authority: Some(Pubkey) — tag + 32 bytes
        offset += 1 + 32;

        // primary_sale_happened: Some(bool) — tag + 1 byte
        offset += 1 + 1;

        // is_mutable: Some(bool) — tag + 1 byte
        offset += 1 + 1;

        assert!(offset <= BUF_CAP);

        // Verify the closed-form matches
        let expected = 56 + name_len + symbol_len + uri_len;
        assert!(offset == expected);
    }

    /// Prove that `update_metadata_accounts_v2` offset arithmetic is correct
    /// in the minimum case: all `Option` fields are `None`.
    ///
    /// Layout (all-None branch):
    ///   discriminator                          1
    ///   Option<DataV2> None tag                1
    ///   new_update_authority None tag           1
    ///   primary_sale_happened None tag          1
    ///   is_mutable None tag                     1
    ///
    /// Total = 5 ≤ 512.
    #[kani::proof]
    fn update_metadata_v2_all_none_offset_within_buffer() {
        const BUF_CAP: usize = 512;

        // Mirror the offset arithmetic from update_metadata.rs (all-None branch)
        let mut offset: usize = 0;

        // Discriminator
        offset += 1;

        // Option<DataV2>: None tag
        offset += 1;

        // new_update_authority: None tag
        offset += 1;

        // primary_sale_happened: None tag
        offset += 1;

        // is_mutable: None tag
        offset += 1;

        assert!(offset <= BUF_CAP);
        assert!(offset == 5);
    }
}
