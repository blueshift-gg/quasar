//! Smoke-level fuzz: feed the decoder random byte strings and assert it
//! *never* panics, only returns `Err(DecodeError)` (or, improbably, `Ok`).
//!
//! This is not a replacement for a real cargo-fuzz harness — it runs a fixed
//! number of iterations with a seeded PRNG so the test is deterministic in CI
//! — but it catches the dominant class of decoder bugs (out-of-bounds slice,
//! arithmetic overflow, allocation explosion) without pulling in `libfuzzer`.

use quasar_schema_canonical::{decode, decode_body};

/// Deterministic splitmix64 PRNG — no external dep, reproducible across
/// platforms. Seed choice is arbitrary but fixed so failures are debuggable.
struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    fn fill(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            let bytes = self.next().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
    }
}

#[test]
fn decode_never_panics_on_random_input() {
    let mut rng = SplitMix64::new(0xDEADBEEFCAFEBABE);

    // 2_000 iterations × up to 256 bytes — small enough to run fast, large
    // enough to exercise varied length prefixes, tag bytes, and short reads.
    for _ in 0..2_000 {
        let len = (rng.next() as usize) % 256;
        let mut buf = vec![0u8; len];
        rng.fill(&mut buf);

        // `decode` may or may not error; the only thing we disallow is a
        // panic / abort. `std::panic::catch_unwind` would let us assert this
        // explicitly, but any panic here will just fail the test normally.
        let _ = decode(&buf);
        let _ = decode_body(&buf);
    }
}

#[test]
fn decode_never_panics_on_short_buffers() {
    // Exhaustive sweep of buffer sizes 0..=48 with the deterministic stream
    // above. Catches off-by-one errors at the header boundary (40 bytes).
    let mut rng = SplitMix64::new(0x0123456789ABCDEF);
    for len in 0..=48 {
        for _ in 0..16 {
            let mut buf = vec![0u8; len];
            rng.fill(&mut buf);
            let _ = decode(&buf);
            let _ = decode_body(&buf);
        }
    }
}

#[test]
fn decode_never_panics_with_valid_header_random_body() {
    // Craft a well-formed header (magic + version + body_len + zero digest)
    // pointing at a random body. The digest check will reject, but only
    // *after* the body-length check — so if body_len disagrees with the
    // slice, `BodyLengthMismatch` fires first. Either way: no panic.
    use quasar_schema_canonical::{wire, MAGIC, VERSION};

    let mut rng = SplitMix64::new(0xFEED_FACE_BAAD_F00D);
    for _ in 0..500 {
        let body_len = (rng.next() as usize) % 512;
        let mut body = vec![0u8; body_len];
        rng.fill(&mut body);

        let mut blob = Vec::with_capacity(wire::HEADER_LEN + body_len);
        blob.extend_from_slice(&MAGIC);
        blob.push(VERSION);
        blob.extend_from_slice(&(body_len as u32).to_le_bytes());
        blob.extend_from_slice(&[0u8; 32]); // deliberately-wrong digest
        blob.extend_from_slice(&body);

        let _ = decode(&blob);
    }
}
