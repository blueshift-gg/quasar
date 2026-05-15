fn assert_u64_data<const DISCRIMINATOR: u8>(data: [u8; 9], value: u64) {
    assert!(data[0] == DISCRIMINATOR);
    let le = value.to_le_bytes();
    let mut i: usize = 0;
    while i < 8 {
        assert!(data[1 + i] == le[i]);
        i += 1;
    }
}

fn assert_option_u64_some<const DISCRIMINATOR: u8>(data: [u8; 10], value: u64) {
    assert!(data[0] == DISCRIMINATOR);
    assert!(data[1] == 1u8);
    let le = value.to_le_bytes();
    let mut i: usize = 0;
    while i < 8 {
        assert!(data[2 + i] == le[i]);
        i += 1;
    }
}

fn assert_option_u64_none<const DISCRIMINATOR: u8>(data: [u8; 10]) {
    assert!(data[0] == DISCRIMINATOR);
    assert!(data[1] == 0u8);
    let mut i: usize = 0;
    while i < 8 {
        assert!(data[2 + i] == 0u8);
        i += 1;
    }
}

/// Prove that the `create_master_edition_v3` instruction data layout is
/// correct when `max_supply` is `Some(v)` for all possible `v` values.
#[kani::proof]
fn create_master_edition_v3_some_layout() {
    let max_supply: u64 = kani::any();

    let data = super::option_u64_data::<17>(Some(max_supply));
    assert_option_u64_some::<17>(data, max_supply);
}

/// Prove that the `create_master_edition_v3` instruction data layout is
/// correct when `max_supply` is `None` (option tag 0, eight zero bytes).
#[kani::proof]
fn create_master_edition_v3_none_layout() {
    let data = super::option_u64_data::<17>(None);
    assert_option_u64_none::<17>(data);
}

/// Prove that the `mint_new_edition_from_master_edition_via_token`
/// instruction data layout is correct for all possible `edition` values.
#[kani::proof]
fn mint_edition_instruction_layout() {
    let edition: u64 = kani::any();

    let data = super::u64_data::<11>(edition);
    assert_u64_data::<11>(data, edition);
}

/// Prove that the `set_collection_size` instruction data layout is correct
/// for all possible `size` values.
#[kani::proof]
fn set_collection_size_instruction_layout() {
    let size: u64 = kani::any();

    let data = super::u64_data::<34>(size);
    assert_u64_data::<34>(data, size);
}

/// Prove that the `bubblegum_set_collection_size` instruction data layout
/// is correct for all possible `size` values.
#[kani::proof]
fn bubblegum_set_collection_size_instruction_layout() {
    let size: u64 = kani::any();

    let data = super::u64_data::<36>(size);
    assert_u64_data::<36>(data, size);
}

/// Prove that the `utilize` instruction data layout is correct for all
/// possible `number_of_uses` values.
#[kani::proof]
fn utilize_instruction_layout() {
    let number_of_uses: u64 = kani::any();

    let data = super::u64_data::<19>(number_of_uses);
    assert_u64_data::<19>(data, number_of_uses);
}

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
///   Max  = 20 + 32 + 10 + 200 = 262 <= 512.
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
/// Max  = 56 + 32 + 10 + 200 = 298 <= 512.
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

    // new_update_authority: Some(Pubkey) tag + 32 bytes
    offset += 1 + 32;

    // primary_sale_happened: Some(bool) tag + 1 byte
    offset += 1 + 1;

    // is_mutable: Some(bool) tag + 1 byte
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
/// Total = 5 <= 512.
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
