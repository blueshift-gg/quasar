use quasar_core::borsh::BorshString;
use quasar_core::cpi::{BufCpiCall, CpiCall, InstructionAccount};
use quasar_core::prelude::*;

// Metaplex-enforced maximum field lengths.
const MAX_NAME_LEN: usize = 32;
const MAX_SYMBOL_LEN: usize = 10;
const MAX_URI_LEN: usize = 200;

const RENT_SYSVAR: Address = Address::new_from_array([
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155, 161,
    253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
]);

// Metaplex Token Metadata instruction discriminators (Borsh enum variant index).
const CREATE_METADATA_ACCOUNTS_V3: u8 = 33;
const UPDATE_METADATA_ACCOUNTS_V2: u8 = 15;
const CREATE_MASTER_EDITION_V3: u8 = 17;
const MINT_NEW_EDITION_FROM_MASTER_EDITION_VIA_TOKEN: u8 = 11;
const UPDATE_PRIMARY_SALE_HAPPENED_VIA_TOKEN: u8 = 4;
const SIGN_METADATA: u8 = 7;
const VERIFY_COLLECTION: u8 = 18;
const UTILIZE: u8 = 19;
const UNVERIFY_COLLECTION: u8 = 22;
const APPROVE_COLLECTION_AUTHORITY: u8 = 23;
const REVOKE_COLLECTION_AUTHORITY: u8 = 24;
const SET_AND_VERIFY_COLLECTION: u8 = 25;
const FREEZE_DELEGATED_ACCOUNT: u8 = 26;
const THAW_DELEGATED_ACCOUNT: u8 = 27;
const REMOVE_CREATOR_VERIFICATION: u8 = 28;
const BURN_NFT: u8 = 29;
const VERIFY_SIZED_COLLECTION_ITEM: u8 = 30;
const UNVERIFY_SIZED_COLLECTION_ITEM: u8 = 31;
const SET_AND_VERIFY_SIZED_COLLECTION_ITEM: u8 = 32;
const SET_COLLECTION_SIZE: u8 = 34;
const SET_TOKEN_STANDARD: u8 = 35;
const BUBBLEGUM_SET_COLLECTION_SIZE: u8 = 36;
const BURN_EDITION_NFT: u8 = 37;

/// Trait for types that can execute Metaplex Token Metadata CPI calls.
///
/// Implemented by [`super::MetadataProgram`].
pub trait MetadataCpi: AsAccountView {
    // -----------------------------------------------------------------------
    // Variable-length instructions (BufCpiCall)
    // -----------------------------------------------------------------------

    /// Create a metadata account for an SPL Token mint.
    ///
    /// Accounts (7): metadata, mint, mint_authority, payer, update_authority,
    /// system_program, rent (sysvar address).
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
        name: BorshString<'_>,
        symbol: BorshString<'_>,
        uri: BorshString<'_>,
        seller_fee_basis_points: u16,
        is_mutable: bool,
        update_authority_is_signer: bool,
    ) -> BufCpiCall<'a, 7, 512> {
        let metadata = metadata.to_account_view();
        let mint = mint.to_account_view();
        let mint_authority = mint_authority.to_account_view();
        let payer = payer.to_account_view();
        let update_authority = update_authority.to_account_view();
        let system_program = system_program.to_account_view();
        let rent = rent.to_account_view();

        assert!(
            name.0.len() <= MAX_NAME_LEN
                && symbol.0.len() <= MAX_SYMBOL_LEN
                && uri.0.len() <= MAX_URI_LEN,
            "metadata field lengths exceed Metaplex limits (name={}, symbol={}, uri={})",
            name.0.len(),
            symbol.0.len(),
            uri.0.len(),
        );

        // Borsh-serialize: discriminator + DataV2 + is_mutable + collection_details
        // DataV2 = name(String) + symbol(String) + uri(String) + seller_fee(u16) + creators(Option<Vec>) + collection(Option) + uses(Option)
        let mut data = [0u8; 512];
        let mut offset = 0;

        // SAFETY: All writes are within the 512-byte buffer. The assert above
        // enforces name<=32, symbol<=10, uri<=200, so max variable data =
        // 12 (len prefixes) + 242 (bytes) + 8 (fixed fields) + 1 (disc) = 263.
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
            self.address(),
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
                InstructionAccount::readonly(&RENT_SYSVAR),
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
        name: Option<BorshString<'_>>,
        symbol: Option<BorshString<'_>>,
        uri: Option<BorshString<'_>>,
        seller_fee_basis_points: Option<u16>,
        primary_sale_happened: Option<bool>,
        is_mutable: Option<bool>,
    ) -> BufCpiCall<'a, 2, 512> {
        let metadata = metadata.to_account_view();
        let update_authority = update_authority.to_account_view();

        if let Some(ref n) = name {
            assert!(
                n.0.len() <= MAX_NAME_LEN,
                "name length {} exceeds max {}",
                n.0.len(),
                MAX_NAME_LEN
            );
        }
        if let Some(ref s) = symbol {
            assert!(
                s.0.len() <= MAX_SYMBOL_LEN,
                "symbol length {} exceeds max {}",
                s.0.len(),
                MAX_SYMBOL_LEN
            );
        }
        if let Some(ref u) = uri {
            assert!(
                u.0.len() <= MAX_URI_LEN,
                "uri length {} exceeds max {}",
                u.0.len(),
                MAX_URI_LEN
            );
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
            self.address(),
            [
                InstructionAccount::writable(metadata.address()),
                InstructionAccount::readonly_signer(update_authority.address()),
            ],
            [metadata, update_authority],
            data,
            offset,
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
        let edition = edition.to_account_view();
        let mint = mint.to_account_view();
        let update_authority = update_authority.to_account_view();
        let mint_authority = mint_authority.to_account_view();
        let payer = payer.to_account_view();
        let metadata = metadata.to_account_view();
        let token_program = token_program.to_account_view();
        let system_program = system_program.to_account_view();
        let rent = rent.to_account_view();

        // SAFETY: All 10 bytes are written before assume_init.
        // Layout: discriminator(1) + Option<u64>(1 tag + 8 value) = 10 bytes
        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 10]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, CREATE_MASTER_EDITION_V3);
            match max_supply {
                Some(v) => {
                    core::ptr::write(ptr.add(1), 1u8);
                    core::ptr::copy_nonoverlapping(v.to_le_bytes().as_ptr(), ptr.add(2), 8);
                }
                None => {
                    core::ptr::write(ptr.add(1), 0u8);
                    core::ptr::write_bytes(ptr.add(2), 0, 8);
                }
            }
            buf.assume_init()
        };

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(edition.address()),
                InstructionAccount::writable(mint.address()),
                InstructionAccount::readonly_signer(update_authority.address()),
                InstructionAccount::readonly_signer(mint_authority.address()),
                InstructionAccount::writable_signer(payer.address()),
                InstructionAccount::writable(metadata.address()),
                InstructionAccount::readonly(token_program.address()),
                InstructionAccount::readonly(system_program.address()),
                InstructionAccount::readonly(&RENT_SYSVAR),
            ],
            [
                edition,
                mint,
                update_authority,
                mint_authority,
                payer,
                metadata,
                token_program,
                system_program,
                rent,
            ],
            data,
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
        let new_metadata = new_metadata.to_account_view();
        let new_edition = new_edition.to_account_view();
        let master_edition = master_edition.to_account_view();
        let new_mint = new_mint.to_account_view();
        let edition_mark_pda = edition_mark_pda.to_account_view();
        let new_mint_authority = new_mint_authority.to_account_view();
        let payer = payer.to_account_view();
        let token_account_owner = token_account_owner.to_account_view();
        let token_account = token_account.to_account_view();
        let new_metadata_update_authority = new_metadata_update_authority.to_account_view();
        let metadata = metadata.to_account_view();
        let token_program = token_program.to_account_view();
        let system_program = system_program.to_account_view();
        let rent = rent.to_account_view();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, MINT_NEW_EDITION_FROM_MASTER_EDITION_VIA_TOKEN);
            core::ptr::copy_nonoverlapping(edition.to_le_bytes().as_ptr(), ptr.add(1), 8);
            buf.assume_init()
        };

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(new_metadata.address()),
                InstructionAccount::writable(new_edition.address()),
                InstructionAccount::writable(master_edition.address()),
                InstructionAccount::writable(new_mint.address()),
                InstructionAccount::writable(edition_mark_pda.address()),
                InstructionAccount::readonly_signer(new_mint_authority.address()),
                InstructionAccount::writable_signer(payer.address()),
                InstructionAccount::readonly_signer(token_account_owner.address()),
                InstructionAccount::readonly(token_account.address()),
                InstructionAccount::readonly(new_metadata_update_authority.address()),
                InstructionAccount::readonly(metadata.address()),
                InstructionAccount::readonly(token_program.address()),
                InstructionAccount::readonly(system_program.address()),
                InstructionAccount::readonly(&RENT_SYSVAR),
            ],
            [
                new_metadata,
                new_edition,
                master_edition,
                new_mint,
                edition_mark_pda,
                new_mint_authority,
                payer,
                token_account_owner,
                token_account,
                new_metadata_update_authority,
                metadata,
                token_program,
                system_program,
                rent,
            ],
            data,
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
        let creator = creator.to_account_view();
        let metadata = metadata.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::readonly_signer(creator.address()),
                InstructionAccount::writable(metadata.address()),
            ],
            [creator, metadata],
            [SIGN_METADATA],
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
        let creator = creator.to_account_view();
        let metadata = metadata.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::readonly_signer(creator.address()),
                InstructionAccount::writable(metadata.address()),
            ],
            [creator, metadata],
            [REMOVE_CREATOR_VERIFICATION],
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
        let metadata = metadata.to_account_view();
        let owner = owner.to_account_view();
        let token = token.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(metadata.address()),
                InstructionAccount::readonly_signer(owner.address()),
                InstructionAccount::readonly(token.address()),
            ],
            [metadata, owner, token],
            [UPDATE_PRIMARY_SALE_HAPPENED_VIA_TOKEN],
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
        let metadata = metadata.to_account_view();
        let collection_authority = collection_authority.to_account_view();
        let payer = payer.to_account_view();
        let collection_mint = collection_mint.to_account_view();
        let collection_metadata = collection_metadata.to_account_view();
        let collection_master_edition = collection_master_edition.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(metadata.address()),
                InstructionAccount::readonly_signer(collection_authority.address()),
                InstructionAccount::writable_signer(payer.address()),
                InstructionAccount::readonly(collection_mint.address()),
                InstructionAccount::readonly(collection_metadata.address()),
                InstructionAccount::readonly(collection_master_edition.address()),
            ],
            [
                metadata,
                collection_authority,
                payer,
                collection_mint,
                collection_metadata,
                collection_master_edition,
            ],
            [VERIFY_COLLECTION],
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
        let metadata = metadata.to_account_view();
        let collection_authority = collection_authority.to_account_view();
        let payer = payer.to_account_view();
        let collection_mint = collection_mint.to_account_view();
        let collection_metadata = collection_metadata.to_account_view();
        let collection_master_edition = collection_master_edition.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(metadata.address()),
                InstructionAccount::readonly_signer(collection_authority.address()),
                InstructionAccount::writable_signer(payer.address()),
                InstructionAccount::readonly(collection_mint.address()),
                InstructionAccount::readonly(collection_metadata.address()),
                InstructionAccount::readonly(collection_master_edition.address()),
            ],
            [
                metadata,
                collection_authority,
                payer,
                collection_mint,
                collection_metadata,
                collection_master_edition,
            ],
            [VERIFY_SIZED_COLLECTION_ITEM],
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
        let metadata = metadata.to_account_view();
        let collection_authority = collection_authority.to_account_view();
        let collection_mint = collection_mint.to_account_view();
        let collection_metadata = collection_metadata.to_account_view();
        let collection_master_edition = collection_master_edition.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(metadata.address()),
                InstructionAccount::readonly_signer(collection_authority.address()),
                InstructionAccount::readonly(collection_mint.address()),
                InstructionAccount::readonly(collection_metadata.address()),
                InstructionAccount::readonly(collection_master_edition.address()),
            ],
            [
                metadata,
                collection_authority,
                collection_mint,
                collection_metadata,
                collection_master_edition,
            ],
            [UNVERIFY_COLLECTION],
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
        let metadata = metadata.to_account_view();
        let collection_authority = collection_authority.to_account_view();
        let payer = payer.to_account_view();
        let collection_mint = collection_mint.to_account_view();
        let collection_metadata = collection_metadata.to_account_view();
        let collection_master_edition = collection_master_edition.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(metadata.address()),
                InstructionAccount::readonly_signer(collection_authority.address()),
                InstructionAccount::writable_signer(payer.address()),
                InstructionAccount::readonly(collection_mint.address()),
                InstructionAccount::readonly(collection_metadata.address()),
                InstructionAccount::readonly(collection_master_edition.address()),
            ],
            [
                metadata,
                collection_authority,
                payer,
                collection_mint,
                collection_metadata,
                collection_master_edition,
            ],
            [UNVERIFY_SIZED_COLLECTION_ITEM],
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
        let metadata = metadata.to_account_view();
        let collection_authority = collection_authority.to_account_view();
        let payer = payer.to_account_view();
        let update_authority = update_authority.to_account_view();
        let collection_mint = collection_mint.to_account_view();
        let collection_metadata = collection_metadata.to_account_view();
        let collection_master_edition = collection_master_edition.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(metadata.address()),
                InstructionAccount::readonly_signer(collection_authority.address()),
                InstructionAccount::writable_signer(payer.address()),
                InstructionAccount::readonly(update_authority.address()),
                InstructionAccount::readonly(collection_mint.address()),
                InstructionAccount::readonly(collection_metadata.address()),
                InstructionAccount::readonly(collection_master_edition.address()),
            ],
            [
                metadata,
                collection_authority,
                payer,
                update_authority,
                collection_mint,
                collection_metadata,
                collection_master_edition,
            ],
            [SET_AND_VERIFY_COLLECTION],
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
        let metadata = metadata.to_account_view();
        let collection_authority = collection_authority.to_account_view();
        let payer = payer.to_account_view();
        let update_authority = update_authority.to_account_view();
        let collection_mint = collection_mint.to_account_view();
        let collection_metadata = collection_metadata.to_account_view();
        let collection_master_edition = collection_master_edition.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(metadata.address()),
                InstructionAccount::readonly_signer(collection_authority.address()),
                InstructionAccount::writable_signer(payer.address()),
                InstructionAccount::readonly(update_authority.address()),
                InstructionAccount::readonly(collection_mint.address()),
                InstructionAccount::readonly(collection_metadata.address()),
                InstructionAccount::readonly(collection_master_edition.address()),
            ],
            [
                metadata,
                collection_authority,
                payer,
                update_authority,
                collection_mint,
                collection_metadata,
                collection_master_edition,
            ],
            [SET_AND_VERIFY_SIZED_COLLECTION_ITEM],
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
        let collection_authority_record = collection_authority_record.to_account_view();
        let new_collection_authority = new_collection_authority.to_account_view();
        let update_authority = update_authority.to_account_view();
        let payer = payer.to_account_view();
        let metadata = metadata.to_account_view();
        let mint = mint.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(collection_authority_record.address()),
                InstructionAccount::readonly(new_collection_authority.address()),
                InstructionAccount::readonly_signer(update_authority.address()),
                InstructionAccount::writable_signer(payer.address()),
                InstructionAccount::readonly(metadata.address()),
                InstructionAccount::readonly(mint.address()),
            ],
            [
                collection_authority_record,
                new_collection_authority,
                update_authority,
                payer,
                metadata,
                mint,
            ],
            [APPROVE_COLLECTION_AUTHORITY],
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
        let collection_authority_record = collection_authority_record.to_account_view();
        let delegate_authority = delegate_authority.to_account_view();
        let revoke_authority = revoke_authority.to_account_view();
        let metadata = metadata.to_account_view();
        let mint = mint.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(collection_authority_record.address()),
                InstructionAccount::readonly(delegate_authority.address()),
                InstructionAccount::readonly_signer(revoke_authority.address()),
                InstructionAccount::readonly(metadata.address()),
                InstructionAccount::readonly(mint.address()),
            ],
            [
                collection_authority_record,
                delegate_authority,
                revoke_authority,
                metadata,
                mint,
            ],
            [REVOKE_COLLECTION_AUTHORITY],
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
        let delegate = delegate.to_account_view();
        let token_account = token_account.to_account_view();
        let edition = edition.to_account_view();
        let mint = mint.to_account_view();
        let token_program = token_program.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::readonly_signer(delegate.address()),
                InstructionAccount::writable(token_account.address()),
                InstructionAccount::readonly(edition.address()),
                InstructionAccount::readonly(mint.address()),
                InstructionAccount::readonly(token_program.address()),
            ],
            [delegate, token_account, edition, mint, token_program],
            [FREEZE_DELEGATED_ACCOUNT],
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
        let delegate = delegate.to_account_view();
        let token_account = token_account.to_account_view();
        let edition = edition.to_account_view();
        let mint = mint.to_account_view();
        let token_program = token_program.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::readonly_signer(delegate.address()),
                InstructionAccount::writable(token_account.address()),
                InstructionAccount::readonly(edition.address()),
                InstructionAccount::readonly(mint.address()),
                InstructionAccount::readonly(token_program.address()),
            ],
            [delegate, token_account, edition, mint, token_program],
            [THAW_DELEGATED_ACCOUNT],
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
        let metadata = metadata.to_account_view();
        let owner = owner.to_account_view();
        let mint = mint.to_account_view();
        let token = token.to_account_view();
        let edition = edition.to_account_view();
        let spl_token = spl_token.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(metadata.address()),
                InstructionAccount::writable_signer(owner.address()),
                InstructionAccount::writable(mint.address()),
                InstructionAccount::writable(token.address()),
                InstructionAccount::writable(edition.address()),
                InstructionAccount::readonly(spl_token.address()),
            ],
            [metadata, owner, mint, token, edition, spl_token],
            [BURN_NFT],
        )
    }

    /// Burn an edition NFT.
    ///
    /// Accounts (10): metadata, owner, print_edition_mint, master_edition_mint,
    /// print_edition_token, master_edition_token, master_edition, print_edition,
    /// edition_marker, spl_token.
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
        let metadata = metadata.to_account_view();
        let owner = owner.to_account_view();
        let print_edition_mint = print_edition_mint.to_account_view();
        let master_edition_mint = master_edition_mint.to_account_view();
        let print_edition_token = print_edition_token.to_account_view();
        let master_edition_token = master_edition_token.to_account_view();
        let master_edition = master_edition.to_account_view();
        let print_edition = print_edition.to_account_view();
        let edition_marker = edition_marker.to_account_view();
        let spl_token = spl_token.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(metadata.address()),
                InstructionAccount::writable_signer(owner.address()),
                InstructionAccount::writable(print_edition_mint.address()),
                InstructionAccount::readonly(master_edition_mint.address()),
                InstructionAccount::writable(print_edition_token.address()),
                InstructionAccount::writable(master_edition_token.address()),
                InstructionAccount::writable(master_edition.address()),
                InstructionAccount::writable(print_edition.address()),
                InstructionAccount::writable(edition_marker.address()),
                InstructionAccount::readonly(spl_token.address()),
            ],
            [
                metadata,
                owner,
                print_edition_mint,
                master_edition_mint,
                print_edition_token,
                master_edition_token,
                master_edition,
                print_edition,
                edition_marker,
                spl_token,
            ],
            [BURN_EDITION_NFT],
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
        let metadata = metadata.to_account_view();
        let update_authority = update_authority.to_account_view();
        let mint = mint.to_account_view();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, SET_COLLECTION_SIZE);
            core::ptr::copy_nonoverlapping(size.to_le_bytes().as_ptr(), ptr.add(1), 8);
            buf.assume_init()
        };

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(metadata.address()),
                InstructionAccount::readonly_signer(update_authority.address()),
                InstructionAccount::readonly(mint.address()),
            ],
            [metadata, update_authority, mint],
            data,
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
        let metadata = metadata.to_account_view();
        let update_authority = update_authority.to_account_view();
        let mint = mint.to_account_view();
        let bubblegum_signer = bubblegum_signer.to_account_view();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, BUBBLEGUM_SET_COLLECTION_SIZE);
            core::ptr::copy_nonoverlapping(size.to_le_bytes().as_ptr(), ptr.add(1), 8);
            buf.assume_init()
        };

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(metadata.address()),
                InstructionAccount::readonly_signer(update_authority.address()),
                InstructionAccount::readonly(mint.address()),
                InstructionAccount::readonly_signer(bubblegum_signer.address()),
            ],
            [metadata, update_authority, mint, bubblegum_signer],
            data,
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
        let metadata = metadata.to_account_view();
        let update_authority = update_authority.to_account_view();
        let mint = mint.to_account_view();
        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(metadata.address()),
                InstructionAccount::readonly_signer(update_authority.address()),
                InstructionAccount::readonly(mint.address()),
            ],
            [metadata, update_authority, mint],
            [SET_TOKEN_STANDARD],
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
        let metadata = metadata.to_account_view();
        let token_account = token_account.to_account_view();
        let mint = mint.to_account_view();
        let use_authority = use_authority.to_account_view();
        let owner = owner.to_account_view();

        let data = unsafe {
            let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, UTILIZE);
            core::ptr::copy_nonoverlapping(number_of_uses.to_le_bytes().as_ptr(), ptr.add(1), 8);
            buf.assume_init()
        };

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(metadata.address()),
                InstructionAccount::writable(token_account.address()),
                InstructionAccount::writable(mint.address()),
                InstructionAccount::readonly_signer(use_authority.address()),
                InstructionAccount::readonly(owner.address()),
            ],
            [metadata, token_account, mint, use_authority, owner],
            data,
        )
    }
}

impl MetadataCpi for super::MetadataProgram {}

/// Blanket impl for raw `AccountView` — used by generated macro code during
/// `#[account(init, metadata::*)]` where typed wrappers aren't constructed yet.
/// The SVM validates the program ID at CPI time, so passing a non-metadata
/// program will fail at runtime.
impl MetadataCpi for AccountView {}
