use super::*;

/// Prove MetadataPrefixZc is exactly 65 bytes and matches ZeroPodFixed::SIZE.
#[kani::proof]
fn metadata_prefix_zc_size_65() {
    assert!(core::mem::size_of::<MetadataPrefixZc>() == 65);
    assert!(<MetadataPrefix as quasar_lang::__zeropod::ZeroPodFixed>::SIZE == 65);
}

/// Prove MetadataPrefixZc has alignment 1, so account data pointer casts do
/// not require a stronger alignment precondition.
#[kani::proof]
fn metadata_prefix_zc_align_one() {
    assert!(core::mem::align_of::<MetadataPrefixZc>() == 1);
}

/// Prove the DataLen guard covers the full metadata prefix layout.
#[kani::proof]
fn metadata_prefix_data_len_guard_sufficient() {
    let data_len: usize = kani::any();
    kani::assume(data_len >= <MetadataPrefix as quasar_lang::__zeropod::ZeroPodFixed>::SIZE);
    assert!(data_len >= core::mem::size_of::<MetadataPrefixZc>());
}

/// Prove MasterEditionPrefixZc is exactly 18 bytes and matches
/// ZeroPodFixed::SIZE.
#[kani::proof]
fn master_edition_prefix_zc_size_18() {
    assert!(core::mem::size_of::<MasterEditionPrefixZc>() == 18);
    assert!(<MasterEditionPrefix as quasar_lang::__zeropod::ZeroPodFixed>::SIZE == 18);
}

/// Prove MasterEditionPrefixZc has alignment 1, so account data pointer casts
/// do not require a stronger alignment precondition.
#[kani::proof]
fn master_edition_prefix_zc_align_one() {
    assert!(core::mem::align_of::<MasterEditionPrefixZc>() == 1);
}

/// Prove the DataLen guard covers the full master edition prefix layout.
#[kani::proof]
fn master_edition_prefix_data_len_guard_sufficient() {
    let data_len: usize = kani::any();
    kani::assume(data_len >= <MasterEditionPrefix as quasar_lang::__zeropod::ZeroPodFixed>::SIZE);
    assert!(data_len >= core::mem::size_of::<MasterEditionPrefixZc>());
}
