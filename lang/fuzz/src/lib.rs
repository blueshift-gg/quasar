//! Shared decode harnesses + seed-corpus generators for the quasar-lang fuzz
//! targets.
//!
//! The `decode_*` functions mirror EXACTLY what `#[instruction]` codegen emits
//! for untrusted instruction data:
//!   fixed:   length check -> `Zc` pointer cast -> `validate_zc` -> `from_zc`
//!            (see `derive/src/instruction.rs` `emit_fixed_schema_stmts`)
//!   compact: `ZeroPodCompact::validate` -> `Ref::new_unchecked` -> accessors
//!            (see `derive/src/instruction.rs` compact path)
//!
//! Invariants under fuzz:
//!   * `decode_fixed`  : a length-checked slice is decoded without OOB reads;
//!                       `validate_zc` gates every `from_zc` (e.g. `Option` tags).
//!   * `decode_compact`: `validate(data).is_ok()` implies `Ref::new_unchecked`
//!                       plus every accessor is total (no OOB / panic).
//! A libFuzzer + AddressSanitizer crash on any of these is a security finding.

use quasar_lang::{
    __zeropod as zeropod, instruction_arg::InstructionArg, prelude::Address, ZeroPodCompact,
};

/// Exercise the allocating client-side decoders on arbitrary, untrusted bytes.
/// Length prefixes are allocation-bounded and UTF-8 decoding is strict; no
/// input may panic or trigger an attacker-sized preallocation.
pub fn decode_client(data: &[u8]) {
    use quasar_lang::client::{wincode, DynString, DynVec};

    let string = wincode::deserialize::<DynString<u32>>(data);
    let bytes = wincode::deserialize::<DynVec<u8, u32>>(data);
    let words = wincode::deserialize::<DynVec<u64, u32>>(data);
    let _ = core::hint::black_box((string, bytes, words));
}

// ---------------------------------------------------------------------------
// decode_fixed: u64 + bool + Option<u64> + Option<Address>
// ---------------------------------------------------------------------------

/// Alignment-1 zero-copy layout, identical to the `__InstructionDataZc` the
/// derive builds for a fixed `#[instruction]` argument list.
#[repr(C)]
struct FixedZc {
    a: <u64 as InstructionArg>::Zc,
    b: <bool as InstructionArg>::Zc,
    c: <Option<u64> as InstructionArg>::Zc,
    d: <Option<Address> as InstructionArg>::Zc,
}

// The derive emits this exact const-assert; the pointer cast below is only
// sound because the ZC layout is align-1.
const _: () = assert!(core::mem::align_of::<FixedZc>() == 1);

const FIXED_SIZE: usize = core::mem::size_of::<FixedZc>();

/// Mirror of the emitted fixed-argument decode path.
pub fn decode_fixed(data: &[u8]) {
    if data.len() < FIXED_SIZE {
        return;
    }
    // SAFETY: length checked above; `FixedZc` is align-1 so the cast from an
    // arbitrary `&[u8]` is well-defined.
    let zc = unsafe { &*(data.as_ptr() as *const FixedZc) };

    if <u64 as InstructionArg>::validate_zc(&zc.a).is_err() {
        return;
    }
    let a = <u64 as InstructionArg>::from_zc(&zc.a);
    if <bool as InstructionArg>::validate_zc(&zc.b).is_err() {
        return;
    }
    let b = <bool as InstructionArg>::from_zc(&zc.b);
    if <Option<u64> as InstructionArg>::validate_zc(&zc.c).is_err() {
        return;
    }
    let c = <Option<u64> as InstructionArg>::from_zc(&zc.c);
    if <Option<Address> as InstructionArg>::validate_zc(&zc.d).is_err() {
        return;
    }
    let d = <Option<Address> as InstructionArg>::from_zc(&zc.d);

    core::hint::black_box((a, b, c, d));
}

// ---------------------------------------------------------------------------
// decode_compact: one dynamic-tail schema + a two-dynamic-tail schema
// ---------------------------------------------------------------------------

// Fields are read through the generated `*Ref` accessors, not the struct.
#[allow(dead_code)]
#[derive(zeropod::ZeroPod)]
#[zeropod(compact)]
struct FuzzSchema {
    a: u64,
    s: zeropod::pod::PodString<64, 1>,
    v: zeropod::pod::PodVec<u8, 32, 1>,
}

/// Two dynamic tails: doubles as the fuzz guard on the compact wire layout
/// (all inline/prefix headers first, then all tail payloads).
#[allow(dead_code)]
#[derive(zeropod::ZeroPod)]
#[zeropod(compact)]
struct FuzzSchemaTwoDyn {
    a: u64,
    s1: zeropod::pod::PodString<64, 1>,
    s2: zeropod::pod::PodString<64, 1>,
}

/// Mirror of the emitted compact decode path for `FuzzSchema`.
pub fn decode_compact_one(data: &[u8]) {
    if <FuzzSchema as ZeroPodCompact>::validate(data).is_err() {
        return;
    }
    // SAFETY: `validate` succeeded on this exact slice.
    let r = unsafe { FuzzSchemaRef::new_unchecked(data) };
    let a = <u64 as InstructionArg>::from_zc(&r.a);
    let s = r.s();
    let v = r.v();
    core::hint::black_box((a, s, v));
}

/// Mirror of the emitted compact decode path for the two-dynamic schema.
pub fn decode_compact_two(data: &[u8]) {
    if <FuzzSchemaTwoDyn as ZeroPodCompact>::validate(data).is_err() {
        return;
    }
    // SAFETY: `validate` succeeded on this exact slice.
    let r = unsafe { FuzzSchemaTwoDynRef::new_unchecked(data) };
    let a = <u64 as InstructionArg>::from_zc(&r.a);
    let s1 = r.s1();
    let s2 = r.s2();
    core::hint::black_box((a, s1, s2));
}

/// Run both compact schemas over the same input.
pub fn decode_compact(data: &[u8]) {
    decode_compact_one(data);
    decode_compact_two(data);
}

// ---------------------------------------------------------------------------
// remaining_accounts_model: structured SVM account-region state machine
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum ModelEntry {
    Full {
        address: [u8; 32],
        owner: [u8; 32],
        lamports: u64,
        data_len: usize,
    },
    Duplicate {
        original_index: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExpectedAccount {
    address: [u8; 32],
    owner: [u8; 32],
    lamports: u64,
    data_len: usize,
}

struct AccountRegion {
    words: std::vec::Vec<u64>,
    len: usize,
}

impl AccountRegion {
    fn serialize(entries: &[ModelEntry]) -> Self {
        use quasar_lang::__internal::{
            RuntimeAccount, ACCOUNT_HEADER, DUP_ENTRY_SIZE, NOT_BORROWED,
        };

        let len: usize = entries
            .iter()
            .map(|entry| match entry {
                ModelEntry::Full { data_len, .. } => {
                    (ACCOUNT_HEADER + data_len).next_multiple_of(8)
                }
                ModelEntry::Duplicate { .. } => DUP_ENTRY_SIZE,
            })
            .sum();
        let mut region = Self {
            words: std::vec![0; len.div_ceil(8).max(1)],
            len,
        };
        let base = region.words.as_mut_ptr().cast::<u8>();
        let mut offset = 0usize;
        for entry in entries {
            match entry {
                ModelEntry::Full {
                    address,
                    owner,
                    lamports,
                    data_len,
                } => {
                    // SAFETY: `base + offset` is 8-aligned and the allocation
                    // reserves a full account stride for this entry.
                    let raw = unsafe { &mut *base.add(offset).cast::<RuntimeAccount>() };
                    raw.borrow_state = NOT_BORROWED;
                    raw.is_signer = 0;
                    raw.is_writable = 1;
                    raw.executable = 0;
                    raw.padding = [0; 4];
                    raw.address = quasar_lang::prelude::Address::new_from_array(*address);
                    raw.owner = quasar_lang::prelude::Address::new_from_array(*owner);
                    raw.lamports = *lamports;
                    raw.data_len = *data_len as u64;
                    offset += (ACCOUNT_HEADER + data_len).next_multiple_of(8);
                }
                ModelEntry::Duplicate { original_index } => {
                    // SAFETY: every duplicate owns one 8-byte entry at
                    // `base + offset`; only its first byte is interpreted.
                    unsafe { *base.add(offset) = *original_index as u8 };
                    offset += DUP_ENTRY_SIZE;
                }
            }
        }
        region
    }

    fn start(&mut self) -> *mut u8 {
        self.words.as_mut_ptr().cast::<u8>()
    }

    fn boundary(&self) -> *const u8 {
        // SAFETY: `len` is at most the byte capacity reserved in `words`.
        unsafe { self.words.as_ptr().cast::<u8>().add(self.len) }
    }
}

fn model_entries(data: &[u8]) -> std::vec::Vec<ModelEntry> {
    let mut entries = std::vec::Vec::new();
    let mut full_indices = std::vec::Vec::new();
    for (index, chunk) in data.chunks(3).take(24).enumerate() {
        let kind = chunk.first().copied().unwrap_or_default();
        let value = chunk.get(1).copied().unwrap_or_default();
        let extra = chunk.get(2).copied().unwrap_or_default();
        if kind & 1 == 1 && !full_indices.is_empty() {
            entries.push(ModelEntry::Duplicate {
                original_index: full_indices[value as usize % full_indices.len()],
            });
        } else {
            let address_byte = (index + 1) as u8;
            full_indices.push(entries.len());
            entries.push(ModelEntry::Full {
                address: [address_byte; 32],
                owner: [value; 32],
                lamports: u64::from(extra) + 1,
                data_len: usize::from(kind >> 1) % 16,
            });
        }
    }
    entries
}

fn oracle(entries: &[ModelEntry]) -> std::vec::Vec<ExpectedAccount> {
    let mut accounts: std::vec::Vec<ExpectedAccount> = std::vec::Vec::with_capacity(entries.len());
    for entry in entries {
        let account = match entry {
            ModelEntry::Full {
                address,
                owner,
                lamports,
                data_len,
            } => ExpectedAccount {
                address: *address,
                owner: *owner,
                lamports: *lamports,
                data_len: *data_len,
            },
            ModelEntry::Duplicate { original_index } => accounts[*original_index].clone(),
        };
        accounts.push(account);
    }
    accounts
}

fn observed(account: &quasar_lang::remaining::RemainingAccount) -> ExpectedAccount {
    ExpectedAccount {
        address: account.address().to_bytes(),
        owner: account.owner().to_bytes(),
        lamports: account.lamports(),
        data_len: account.data_len(),
    }
}

/// Compare every prefix of a structured account-region sequence with a safe
/// Vec oracle.
pub fn remaining_accounts_model(data: &[u8]) {
    use quasar_lang::{
        accounts::UncheckedAccount, error::QuasarError, prelude::ProgramError,
        remaining::RemainingAccounts,
    };

    let entries = model_entries(data);
    for end in 0..=entries.len() {
        let prefix = &entries[..end];
        let expected = oracle(prefix);
        let mut region = AccountRegion::serialize(prefix);
        let boundary = region.boundary();
        // SAFETY: `AccountRegion::serialize` emits a valid aligned SVM account
        // region, and there are no declared accounts in this model.
        let remaining = unsafe { RemainingAccounts::new(region.start(), boundary, &[]) };

        let iterated = remaining
            .iter()
            .collect::<Result<std::vec::Vec<_>, ProgramError>>()
            .expect("valid structured region must iterate");
        assert_eq!(
            iterated.iter().map(observed).collect::<std::vec::Vec<_>>(),
            expected
        );

        for index in 0..=prefix.len() {
            let actual = remaining
                .get(index)
                .expect("valid structured region must support get")
                .as_ref()
                .map(observed);
            assert_eq!(actual, expected.get(index).cloned());
        }

        let parsed = remaining.parse::<UncheckedAccount, 24>();
        if prefix
            .iter()
            .any(|entry| matches!(entry, ModelEntry::Duplicate { .. }))
        {
            assert_eq!(
                parsed.err(),
                Some(QuasarError::RemainingAccountDuplicate.into())
            );
        } else {
            assert_eq!(
                parsed.expect("unique full accounts must parse").len(),
                expected.len()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Seed corpora, built from the on-chain-matching client serializers.
// ---------------------------------------------------------------------------

/// Valid `decode_fixed` inputs: each field's zero-copy bytes, concatenated in
/// declaration order (exactly the `FixedZc` object representation). Uses the
/// client `SerializeArg` bridge so the seeds share the on-chain wire layout.
pub fn fixed_seeds() -> std::vec::Vec<std::vec::Vec<u8>> {
    use quasar_lang::client::SerializeArg;

    fn seed(a: u64, b: bool, c: Option<u64>, d: Option<Address>) -> std::vec::Vec<u8> {
        let mut buf = std::vec::Vec::new();
        buf.extend(a.serialize_arg());
        buf.extend(b.serialize_arg());
        buf.extend(c.serialize_arg());
        buf.extend(d.serialize_arg());
        buf
    }

    std::vec![
        seed(0, false, None, None),
        seed(1, true, Some(0), None),
        seed(u64::MAX, true, Some(u64::MAX), Some(Address::new_from_array([0x11; 32]))),
        seed(42, false, None, Some(Address::new_from_array([0xAB; 32]))),
    ]
}

/// Build a compact buffer: all field headers (inline fixed + dynamic length
/// prefixes) in order, followed by all dynamic tails in order.
fn compact_one_seed(a: u64, s: &str, v: &[u8]) -> std::vec::Vec<u8> {
    use quasar_lang::client::{CompactSerializeArg, DynString, DynVec};

    let ds = DynString::<u8>::new(s);
    let dv = DynVec::<u8, u8>::new(v.to_vec());

    let mut buf = std::vec::Vec::new();
    // headers
    buf.extend(a.compact_header());
    buf.extend(ds.compact_header());
    buf.extend(dv.compact_header());
    // tails
    buf.extend(a.compact_tail());
    buf.extend(ds.compact_tail());
    buf.extend(dv.compact_tail());
    buf
}

fn compact_two_seed(a: u64, s1: &str, s2: &str) -> std::vec::Vec<u8> {
    use quasar_lang::client::{CompactSerializeArg, DynString};

    let ds1 = DynString::<u8>::new(s1);
    let ds2 = DynString::<u8>::new(s2);

    let mut buf = std::vec::Vec::new();
    buf.extend(a.compact_header());
    buf.extend(ds1.compact_header());
    buf.extend(ds2.compact_header());
    buf.extend(a.compact_tail());
    buf.extend(ds1.compact_tail());
    buf.extend(ds2.compact_tail());
    buf
}

/// Valid `decode_compact` inputs for both schemas.
pub fn compact_seeds() -> std::vec::Vec<std::vec::Vec<u8>> {
    std::vec![
        compact_one_seed(0, "", &[]),
        compact_one_seed(42, "hello", &[1, 2, 3]),
        compact_one_seed(u64::MAX, &"x".repeat(64), &(0u8..32).collect::<std::vec::Vec<u8>>()),
        compact_two_seed(0, "", ""),
        compact_two_seed(7, "aa", "zz"),
        compact_two_seed(u64::MAX, &"a".repeat(64), &"b".repeat(64)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    fn corpus_dir(target: &str) -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("corpus");
        p.push(target);
        p
    }

    fn write_seeds(target: &str, seeds: &[std::vec::Vec<u8>]) {
        let dir = corpus_dir(target);
        fs::create_dir_all(&dir).unwrap();
        for (i, seed) in seeds.iter().enumerate() {
            let path = dir.join(format!("seed_{i:03}"));
            fs::write(&path, seed).unwrap();
        }
    }

    /// Generate + commit the seed corpora. Run with `cargo test` in `lang/fuzz`.
    #[test]
    fn generate_corpora() {
        write_seeds("decode_fixed", &fixed_seeds());
        write_seeds("decode_compact", &compact_seeds());
    }

    /// Every generated fixed seed must be a *valid* (accepted) decode input,
    /// i.e. long enough and passing every `validate_zc`. Guards the generator
    /// against drifting away from the on-chain layout.
    #[test]
    fn fixed_seeds_are_accepted() {
        for seed in fixed_seeds() {
            assert!(seed.len() >= FIXED_SIZE, "fixed seed too short: {}", seed.len());
            // SAFETY: length checked; `FixedZc` is align-1.
            let zc = unsafe { &*(seed.as_ptr() as *const FixedZc) };
            assert!(<u64 as InstructionArg>::validate_zc(&zc.a).is_ok());
            assert!(<bool as InstructionArg>::validate_zc(&zc.b).is_ok());
            assert!(<Option<u64> as InstructionArg>::validate_zc(&zc.c).is_ok());
            assert!(<Option<Address> as InstructionArg>::validate_zc(&zc.d).is_ok());
        }
    }

    /// Every generated compact seed must pass `validate` for its schema so the
    /// corpus exercises the "validated" branch of the decode.
    #[test]
    fn compact_seeds_are_accepted() {
        for seed in
            [compact_one_seed(0, "", &[]), compact_one_seed(42, "hello", &[1, 2, 3])]
        {
            assert!(
                <FuzzSchema as ZeroPodCompact>::validate(&seed).is_ok(),
                "compact one-dyn seed rejected"
            );
        }
        for seed in [compact_two_seed(0, "", ""), compact_two_seed(7, "aa", "zz")] {
            assert!(
                <FuzzSchemaTwoDyn as ZeroPodCompact>::validate(&seed).is_ok(),
                "compact two-dyn seed rejected"
            );
        }
    }
}
