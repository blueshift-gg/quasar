use quasar_lang::client::{wincode, DynBytes, DynVec};

// ===================================================================
// Wire format oracle tests — u32 prefix (default)
//
// Pin the exact byte representation. The on-chain program
// deserializes from alignment-1 zero-copy structs that expect THIS
// exact layout. A change here breaks every deployed program's client.
// ===================================================================

#[test]
fn dyn_bytes_u32_wire_format() {
    let wire = wincode::serialize(&DynBytes::<u32>::new(vec![1, 2, 3])).unwrap();
    assert_eq!(wire, [3, 0, 0, 0, 1, 2, 3]);
}

#[test]
fn dyn_bytes_u32_wire_empty() {
    let wire = wincode::serialize(&DynBytes::<u32>::new(vec![])).unwrap();
    assert_eq!(wire, [0, 0, 0, 0]);
}

#[test]
fn dyn_vec_u32_u8_items_wire() {
    let wire = wincode::serialize(&DynVec::<u8, u32>::new(vec![0xAA, 0xBB])).unwrap();
    assert_eq!(wire, [2, 0, 0, 0, 0xAA, 0xBB]);
}

#[test]
fn dyn_vec_u32_u64_items_wire() {
    let wire = wincode::serialize(&DynVec::<u64, u32>::new(vec![1, 2])).unwrap();
    let mut expected = vec![2u8, 0, 0, 0];
    expected.extend_from_slice(&1u64.to_le_bytes());
    expected.extend_from_slice(&2u64.to_le_bytes());
    assert_eq!(wire, expected);
}

#[test]
fn dyn_vec_u32_address_wire() {
    let addr = solana_address::Address::from([42u8; 32]);
    let wire = wincode::serialize(&DynVec::<_, u32>::new(vec![addr])).unwrap();
    let mut expected = vec![1u8, 0, 0, 0];
    expected.extend_from_slice(&[42u8; 32]);
    assert_eq!(wire, expected);
}

#[test]
fn dyn_vec_u32_empty_wire() {
    let wire = wincode::serialize(&DynVec::<u64, u32>::new(vec![])).unwrap();
    assert_eq!(wire, [0, 0, 0, 0]);
}

// ===================================================================
// Wire format oracle tests — u8 prefix
// ===================================================================

#[test]
fn dyn_bytes_u8_wire_format() {
    let wire = wincode::serialize(&DynBytes::<u8>::new(vec![1, 2, 3])).unwrap();
    assert_eq!(wire, [3, 1, 2, 3]);
}

#[test]
fn dyn_bytes_u8_wire_empty() {
    let wire = wincode::serialize(&DynBytes::<u8>::new(vec![])).unwrap();
    assert_eq!(wire, [0]);
}

#[test]
fn dyn_vec_u8_prefix_wire_format() {
    let wire = wincode::serialize(&DynVec::<u8, u8>::new(vec![0xAA, 0xBB])).unwrap();
    assert_eq!(wire, [2, 0xAA, 0xBB]);
}

#[test]
fn dyn_vec_u8_prefix_u64_items_wire() {
    let wire = wincode::serialize(&DynVec::<u64, u8>::new(vec![1, 2])).unwrap();
    let mut expected = vec![2u8];
    expected.extend_from_slice(&1u64.to_le_bytes());
    expected.extend_from_slice(&2u64.to_le_bytes());
    assert_eq!(wire, expected);
}

#[test]
fn dyn_vec_u8_prefix_empty_wire() {
    let wire = wincode::serialize(&DynVec::<u64, u8>::new(vec![])).unwrap();
    assert_eq!(wire, [0]);
}

// ===================================================================
// Wire format oracle tests — u16 prefix
// ===================================================================

#[test]
fn dyn_bytes_u16_wire_format() {
    let wire = wincode::serialize(&DynBytes::<u16>::new(vec![1, 2])).unwrap();
    assert_eq!(wire, [2, 0, 1, 2]);
}

#[test]
fn dyn_bytes_u16_wire_empty() {
    let wire = wincode::serialize(&DynBytes::<u16>::new(vec![])).unwrap();
    assert_eq!(wire, [0, 0]);
}

#[test]
fn dyn_vec_u16_prefix_wire_format() {
    let wire = wincode::serialize(&DynVec::<u8, u16>::new(vec![0xAA, 0xBB])).unwrap();
    assert_eq!(wire, [2, 0, 0xAA, 0xBB]);
}

#[test]
fn dyn_vec_u16_prefix_u64_items_wire() {
    let wire = wincode::serialize(&DynVec::<u64, u16>::new(vec![1])).unwrap();
    let mut expected = vec![1u8, 0];
    expected.extend_from_slice(&1u64.to_le_bytes());
    assert_eq!(wire, expected);
}

#[test]
fn dyn_vec_u16_prefix_empty_wire() {
    let wire = wincode::serialize(&DynVec::<u64, u16>::new(vec![])).unwrap();
    assert_eq!(wire, [0, 0]);
}

// ===================================================================
// Round-trip: serialize → deserialize = identity
//
// Exhaustive for every prefix width × element type combination.
// ===================================================================

// --- DynBytes round-trips ---

#[test]
fn dyn_bytes_u8_roundtrip() {
    let original = DynBytes::<u8>::new(vec![10, 20, 30]);
    let wire = wincode::serialize(&original).unwrap();
    let decoded: DynBytes<u8> = wincode::deserialize(&wire).unwrap();
    assert_eq!(decoded.0, original.0);
}

#[test]
fn dyn_bytes_u8_roundtrip_empty() {
    let original = DynBytes::<u8>::new(vec![]);
    let wire = wincode::serialize(&original).unwrap();
    let decoded: DynBytes<u8> = wincode::deserialize(&wire).unwrap();
    assert_eq!(decoded.0, original.0);
}

#[test]
fn dyn_bytes_u16_roundtrip() {
    let original = DynBytes::<u16>::new(vec![1, 2, 3, 4, 5]);
    let wire = wincode::serialize(&original).unwrap();
    let decoded: DynBytes<u16> = wincode::deserialize(&wire).unwrap();
    assert_eq!(decoded.0, original.0);
}

#[test]
fn dyn_bytes_u16_roundtrip_empty() {
    let original = DynBytes::<u16>::new(vec![]);
    let wire = wincode::serialize(&original).unwrap();
    let decoded: DynBytes<u16> = wincode::deserialize(&wire).unwrap();
    assert_eq!(decoded.0, original.0);
}

#[test]
fn dyn_bytes_u32_roundtrip() {
    let original = DynBytes::<u32>::new(vec![1, 2, 3, 4, 5]);
    let wire = wincode::serialize(&original).unwrap();
    let decoded: DynBytes<u32> = wincode::deserialize(&wire).unwrap();
    assert_eq!(decoded.0, original.0);
}

#[test]
fn dyn_bytes_u32_roundtrip_empty() {
    let original = DynBytes::<u32>::new(vec![]);
    let wire = wincode::serialize(&original).unwrap();
    let decoded: DynBytes<u32> = wincode::deserialize(&wire).unwrap();
    assert_eq!(decoded.0, original.0);
}

// --- DynVec round-trips ---

#[test]
fn dyn_vec_u8_prefix_u64_roundtrip() {
    let original = DynVec::<u64, u8>::new(vec![100, 200]);
    let wire = wincode::serialize(&original).unwrap();
    let decoded: DynVec<u64, u8> = wincode::deserialize(&wire).unwrap();
    assert_eq!(decoded.0, original.0);
}

#[test]
fn dyn_vec_u8_prefix_addr_roundtrip() {
    let original =
        DynVec::<solana_address::Address, u8>::new(vec![solana_address::Address::from([1u8; 32])]);
    let wire = wincode::serialize(&original).unwrap();
    let decoded: DynVec<solana_address::Address, u8> = wincode::deserialize(&wire).unwrap();
    assert_eq!(decoded.0, original.0);
}

#[test]
fn dyn_vec_u16_prefix_u64_roundtrip() {
    let original = DynVec::<u64, u16>::new(vec![100, 200, 300]);
    let wire = wincode::serialize(&original).unwrap();
    let decoded: DynVec<u64, u16> = wincode::deserialize(&wire).unwrap();
    assert_eq!(decoded.0, original.0);
}

#[test]
fn dyn_vec_u16_prefix_addr_roundtrip() {
    let original = DynVec::<solana_address::Address, u16>::new(vec![
        solana_address::Address::from([1u8; 32]),
        solana_address::Address::from([2u8; 32]),
    ]);
    let wire = wincode::serialize(&original).unwrap();
    let decoded: DynVec<solana_address::Address, u16> = wincode::deserialize(&wire).unwrap();
    assert_eq!(decoded.0, original.0);
}

#[test]
fn dyn_vec_u32_u64_roundtrip() {
    let original = DynVec::<u64, u32>::new(vec![100, 200, 300]);
    let wire = wincode::serialize(&original).unwrap();
    let decoded: DynVec<u64, u32> = wincode::deserialize(&wire).unwrap();
    assert_eq!(decoded.0, original.0);
}

#[test]
fn dyn_vec_u32_addr_roundtrip() {
    let original = DynVec::<solana_address::Address, u32>::new(vec![
        solana_address::Address::from([1u8; 32]),
        solana_address::Address::from([2u8; 32]),
    ]);
    let wire = wincode::serialize(&original).unwrap();
    let decoded: DynVec<solana_address::Address, u32> = wincode::deserialize(&wire).unwrap();
    assert_eq!(decoded.0, original.0);
}

// ===================================================================
// On-chain wire compatibility
//
// Construct bytes exactly as the on-chain ZC layout expects, then
// deserialize through our types to prove they match. Covers every
// prefix width to catch prefix-size mismatches.
// ===================================================================

// --- u32 prefix (String<100> / Vec<T, 100>) ---

#[test]
fn dyn_bytes_u32_reads_onchain_layout() {
    let mut data = vec![];
    data.extend_from_slice(&5u32.to_le_bytes());
    data.extend_from_slice(b"hello");
    let decoded: DynBytes<u32> = wincode::deserialize(&data).unwrap();
    assert_eq!(decoded.0, b"hello");
}

#[test]
fn dyn_vec_u32_reads_onchain_layout() {
    let mut data = vec![];
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(&42u64.to_le_bytes());
    data.extend_from_slice(&99u64.to_le_bytes());
    let decoded: DynVec<u64, u32> = wincode::deserialize(&data).unwrap();
    assert_eq!(decoded.0, vec![42u64, 99]);
}

#[test]
fn dyn_vec_u32_address_reads_onchain_layout() {
    let addr_bytes = [42u8; 32];
    let mut data = vec![];
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&addr_bytes);
    let decoded: DynVec<solana_address::Address, u32> = wincode::deserialize(&data).unwrap();
    assert_eq!(decoded.0, vec![solana_address::Address::from(addr_bytes)]);
}

// --- u8 prefix (String<u8, 100> / Vec<T, u8, 100>) ---

#[test]
fn dyn_bytes_u8_reads_onchain_layout() {
    // On-chain String<u8, 100>: [u8 byte-length][raw bytes]
    let data = vec![3u8, b'a', b'b', b'c'];
    let decoded: DynBytes<u8> = wincode::deserialize(&data).unwrap();
    assert_eq!(decoded.0, b"abc");
}

#[test]
fn dyn_vec_u8_reads_onchain_layout() {
    // On-chain Vec<u64, u8, 10>: [u8 count][u64 LE values]
    let mut data = vec![2u8];
    data.extend_from_slice(&42u64.to_le_bytes());
    data.extend_from_slice(&99u64.to_le_bytes());
    let decoded: DynVec<u64, u8> = wincode::deserialize(&data).unwrap();
    assert_eq!(decoded.0, vec![42u64, 99]);
}

// --- u16 prefix (String<u16, 1000> / Vec<T, u16, 1000>) ---

#[test]
fn dyn_bytes_u16_reads_onchain_layout() {
    // On-chain String<u16, 1000>: [u16 LE byte-length][raw bytes]
    let mut data = vec![];
    data.extend_from_slice(&5u16.to_le_bytes());
    data.extend_from_slice(b"hello");
    let decoded: DynBytes<u16> = wincode::deserialize(&data).unwrap();
    assert_eq!(decoded.0, b"hello");
}

#[test]
fn dyn_vec_u16_reads_onchain_layout() {
    // On-chain Vec<u64, u16, 500>: [u16 LE count][u64 LE values]
    let mut data = vec![];
    data.extend_from_slice(&2u16.to_le_bytes());
    data.extend_from_slice(&42u64.to_le_bytes());
    data.extend_from_slice(&99u64.to_le_bytes());
    let decoded: DynVec<u64, u16> = wincode::deserialize(&data).unwrap();
    assert_eq!(decoded.0, vec![42u64, 99]);
}

// ===================================================================
// Cross-prefix isolation
//
// Prove that data serialized with one prefix width cannot be
// accidentally deserialized with a different prefix width. This
// catches the exact bug we're fixing: client sending u32-prefixed
// data to an on-chain program expecting u8-prefixed data.
// ===================================================================

#[test]
fn dyn_bytes_u8_data_rejected_by_u32_decode() {
    // u8-prefixed: [3, 1, 2, 3]
    let wire = wincode::serialize(&DynBytes::<u8>::new(vec![1, 2, 3])).unwrap();
    // Attempt to read as u32-prefixed: reads 4 bytes as length = 0x03020103
    // which is way beyond the buffer
    let result = wincode::deserialize::<DynBytes<u32>>(&wire);
    assert!(result.is_err());
}

#[test]
fn dyn_bytes_u32_data_not_u8_compatible() {
    // u32-prefixed: [3, 0, 0, 0, 1, 2, 3]
    let wire = wincode::serialize(&DynBytes::<u32>::new(vec![1, 2, 3])).unwrap();
    // Read as u8-prefixed: length = 3, then reads [0, 0, 0] — wrong data
    let decoded: DynBytes<u8> = wincode::deserialize(&wire).unwrap();
    assert_ne!(
        decoded.0,
        vec![1u8, 2, 3],
        "prefix mismatch must produce different data"
    );
}

// ===================================================================
// Multi-field instruction data
//
// Simulates what build_ix() produces: disc bytes followed by
// concat of wincode::serialize for each arg.
// ===================================================================

#[test]
fn instruction_data_disc_plus_args() {
    let disc: &[u8] = &[1];
    let amount = 42u64;
    let name = DynBytes::<u32>::new(b"test".to_vec());

    let mut data = Vec::from(disc);
    data.extend_from_slice(&wincode::serialize(&amount).unwrap());
    data.extend_from_slice(&wincode::serialize(&name).unwrap());

    let mut expected = vec![1u8]; // disc
    expected.extend_from_slice(&42u64.to_le_bytes()); // amount
    expected.extend_from_slice(&4u32.to_le_bytes()); // name length
    expected.extend_from_slice(b"test"); // name bytes
    assert_eq!(data, expected);
}

#[test]
fn instruction_data_disc_plus_u8_dyn_bytes() {
    // Same instruction but with u8-prefixed string
    let disc: &[u8] = &[1];
    let amount = 42u64;
    let name = DynBytes::<u8>::new(b"test".to_vec());

    let mut data = Vec::from(disc);
    data.extend_from_slice(&wincode::serialize(&amount).unwrap());
    data.extend_from_slice(&wincode::serialize(&name).unwrap());

    let mut expected = vec![1u8]; // disc
    expected.extend_from_slice(&42u64.to_le_bytes()); // amount
    expected.push(4u8); // name length (u8 prefix!)
    expected.extend_from_slice(b"test"); // name bytes
    assert_eq!(data, expected);
}

#[test]
fn instruction_data_disc_plus_dyn_vec_arg() {
    let disc: &[u8] = &[2];
    let tags = DynVec::<_, u32>::new(vec![
        solana_address::Address::from([1u8; 32]),
        solana_address::Address::from([2u8; 32]),
    ]);

    let mut data = Vec::from(disc);
    data.extend_from_slice(&wincode::serialize(&tags).unwrap());

    let mut expected = vec![2u8]; // disc
    expected.extend_from_slice(&2u32.to_le_bytes()); // count
    expected.extend_from_slice(&[1u8; 32]); // addr 1
    expected.extend_from_slice(&[2u8; 32]); // addr 2
    assert_eq!(data, expected);
}

// ===================================================================
// Prefix width byte size verification
//
// Confirm the exact number of prefix bytes for each type to catch
// off-by-one or wrong-width regressions.
// ===================================================================

#[test]
fn dyn_bytes_prefix_sizes() {
    let payload = vec![0xFFu8; 10];
    let wire_u8 = wincode::serialize(&DynBytes::<u8>::new(payload.clone())).unwrap();
    let wire_u16 = wincode::serialize(&DynBytes::<u16>::new(payload.clone())).unwrap();
    let wire_u32 = wincode::serialize(&DynBytes::<u32>::new(payload.clone())).unwrap();
    // prefix_bytes + payload_bytes
    assert_eq!(wire_u8.len(), 1 + 10);
    assert_eq!(wire_u16.len(), 2 + 10);
    assert_eq!(wire_u32.len(), 4 + 10);
}

#[test]
fn dyn_vec_prefix_sizes() {
    let items = vec![1u64, 2, 3];
    let wire_u8 = wincode::serialize(&DynVec::<u64, u8>::new(items.clone())).unwrap();
    let wire_u16 = wincode::serialize(&DynVec::<u64, u16>::new(items.clone())).unwrap();
    let wire_u32 = wincode::serialize(&DynVec::<u64, u32>::new(items.clone())).unwrap();
    // prefix_bytes + 3 * 8 (u64 items)
    assert_eq!(wire_u8.len(), 1 + 24);
    assert_eq!(wire_u16.len(), 2 + 24);
    assert_eq!(wire_u32.len(), 4 + 24);
}
