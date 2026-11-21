use quasar_lang::{Address, input::Cursor};

pub const fn key(byte: u8) -> Address {
    Address::new_from_array([byte; 32])
}

/// Host-side ABIv1 buffer. Every account handle stays inside the callback.
pub fn with_accounts(accounts: &[(Address, Address, &[u8])], run: impl FnOnce(Cursor)) {
    let entry_size = |len: usize| (88 + len + 10 * 1024).next_multiple_of(8) + 8;
    const ID: Address = key(99);
    let size = 8
        + accounts.iter().map(|(_, _, data)| entry_size(data.len())).sum::<usize>()
        + size_of::<u64>()
        + size_of::<Address>();
    let mut input = vec![0u64; size / 8];
    input[0] = accounts.len() as u64;
    // SAFETY: the slice spans the initialized, aligned allocation, before any handles exist.
    let bytes = unsafe { core::slice::from_raw_parts_mut(input.as_mut_ptr().cast::<u8>(), size) };
    let mut offset = 8;
    for (key, owner, data) in accounts {
        bytes[offset..offset + 4].copy_from_slice(&[255, 1, 1, 1]);
        bytes[offset + 8..offset + 40].copy_from_slice(key.as_ref());
        bytes[offset + 40..offset + 72].copy_from_slice(owner.as_ref());
        bytes[offset + 72..offset + 80].copy_from_slice(&100u64.to_le_bytes());
        bytes[offset + 80..offset + 88].copy_from_slice(&(data.len() as u64).to_le_bytes());
        bytes[offset + 88..offset + 88 + data.len()].copy_from_slice(data);
        offset += entry_size(data.len());
    }
    // The zero length word is followed by empty instruction data and the executing program ID.
    let data_offset = offset + size_of::<u64>();
    bytes[data_offset..].copy_from_slice(ID.as_ref());
    let input = input.as_mut_ptr().cast::<u8>();
    // SAFETY: the buffer follows the entrypoint ABI and outlives all handles used by `run`.
    let status = unsafe {
        quasar_lang::context::entrypoint::<0>(input, input.add(data_offset), &ID, |_, _, cursor| {
            run(cursor);
            Ok(())
        })
    };
    assert_eq!(status, 0);
}
