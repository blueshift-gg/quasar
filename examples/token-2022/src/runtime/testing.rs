//! Runtime-serialized test input, loaded through the actual Quasar cursor.
use super::AccountView;
use solana_address::Address;

/// # Safety
/// The returned account must not outlive its backing vector.
pub unsafe fn account(
    address: &Address,
    owner: &Address,
    signer: bool,
    writable: bool,
    data: &[u8],
) -> (Vec<u64>, AccountView) {
    let size = 8 + ((88 + data.len() + 10240 + 7) & !7) + 8;
    let mut storage = vec![0u64; size / 8];
    storage[0] = 1;
    let pointer = storage.as_mut_ptr().cast::<u8>();
    // SAFETY: the vector is aligned, initialized and large enough for ABIv1.
    let bytes = unsafe { std::slice::from_raw_parts_mut(pointer, size) };
    bytes[8] = 255;
    bytes[9] = u8::from(signer);
    bytes[10] = u8::from(writable);
    bytes[16..48].copy_from_slice(address.as_ref());
    bytes[48..80].copy_from_slice(owner.as_ref());
    bytes[88..96].copy_from_slice(&(data.len() as u64).to_le_bytes());
    bytes[96..96 + data.len()].copy_from_slice(data);
    let instruction = [0u64; 1];
    let mut result = None;
    // SAFETY: pointer encodes one complete account; instruction's preceding word is zero.
    let status = unsafe {
        quasar_lang::context::entrypoint::<0>(
            pointer,
            instruction.as_ptr().add(1).cast(),
            &crate::ID,
            |_, _, cursor| {
                result = cursor.into_shared()?.pop();
                Ok(())
            },
        )
    };
    assert_eq!(status, 0);
    (storage, result.expect("one serialized account"))
}

pub fn mollusk() -> mollusk_svm::Mollusk {
    let mut runtime = mollusk_svm::Mollusk::default();
    runtime.feature_set = agave_feature_set::FeatureSet::all_enabled();
    runtime.program_cache = mollusk_svm::program::ProgramCache::new(
        &runtime.feature_set,
        &runtime.compute_budget,
        false,
    );
    runtime.add_program(&crate::ID, "quasar_token_2022");
    runtime
}
