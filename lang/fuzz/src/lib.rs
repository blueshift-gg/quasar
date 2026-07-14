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
