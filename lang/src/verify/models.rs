//! What harnesses assume the runtime does in place of each syscall. Each
//! model allows at least every behaviour of the real syscall, so a property
//! proved against the models holds for the real program.

use core::mem::MaybeUninit;

use crate::{Address, Result, syscalls::RawSlice};

pub fn set_return_data(_: &[u8]) {}

pub fn log(_: &str) {}

pub fn end() -> ! {
    panic!("the program ended")
}

pub fn log_data(_: &[&[u8]]) {}

/// Any hash, but a function: hashing the same bytes twice gives the same
/// result, as the real hash does. A fresh value on every call would also
/// allow every real behaviour, but would report failures no real run can
/// have, such as a PDA that passes its check and then fails it on the same
/// seeds.
///
/// # Safety
/// Each part points to `len` readable bytes.
pub unsafe fn sha256<'a>(parts: &[RawSlice], hash: &'a mut MaybeUninit<[u8; 32]>) -> &'a [u8; 32] {
    let mut input = [0u8; HASH_INPUT];
    let mut len = 0;
    for part in parts {
        let part_len = part.len as usize;
        assert!(len + part_len <= HASH_INPUT, "the hash model's input is too long");
        // SAFETY: forwarded from this function's contract.
        let bytes = unsafe { core::slice::from_raw_parts(part.addr, part_len) };
        input[len..len + part_len].copy_from_slice(bytes);
        len += part_len;
    }
    // SAFETY: harnesses run on one thread; nothing else holds a reference.
    let hashed = unsafe { &mut *(&raw mut HASHED) };
    for (seen_input, seen_len, seen_output) in &hashed.calls[..hashed.count] {
        if *seen_len == len && seen_input[..len] == input[..len] {
            return hash.write(*seen_output);
        }
    }
    assert!(hashed.count < HASHES, "more distinct hashes than the model remembers");
    let output = kani::any();
    hashed.calls[hashed.count] = (input, len, output);
    hashed.count += 1;
    hash.write(output)
}

/// The longest hash input the model takes: two addresses of seeds and a short
/// literal, then bump, program and marker. Longer inputs fail verification
/// loudly; raise it for a program that needs more.
const HASH_INPUT: usize = 128;
/// Distinct inputs a harness may hash; a PDA checked before and after an
/// instruction is one.
const HASHES: usize = 2;

struct Hashed {
    calls: [([u8; HASH_INPUT], usize, [u8; 32]); HASHES],
    count: usize,
}

static mut HASHED: Hashed = Hashed { calls: [([0; HASH_INPUT], 0, [0; 32]); HASHES], count: 0 };

/// Either answer: the real one is among them.
pub fn is_on_curve(_: &[u8; 32]) -> bool {
    kani::any()
}

/// Any value of the sysvar.
pub fn sysvar(_: &Address, out: &mut [u8]) -> Result {
    for byte in out {
        *byte = kani::any();
    }
    Ok(())
}

/// # Safety
/// Nothing is read.
pub unsafe fn invoke(_: *const u8, _: *const u8, _: u64, _: *const u8, _: u64) {
    unimplemented!("calls to other programs have no model yet")
}
