//! sBPF ABI facts for the Solana VM, centralized so the `unsafe` SAFETY
//! comments across the crate can reference one authoritative description.
//! Where a fact is expressible as a compile-time invariant it is const-asserted
//! here.
//!
//! # Entrypoint
//!
//! The SVM invokes a program with a two-register convention: the input pointer
//! is passed in `r1`. `entrypoint!` reads the serialized input buffer from that
//! pointer (see [`crate::entrypoint`]).
//!
//! # Input buffer layout
//!
//! The serialized input buffer is:
//!
//! ```text
//! [ num_accounts: u64 ]
//! [ account entries ... ]           (RuntimeAccount headers + data + padding)
//! [ instruction_data_len: u64 ]
//! [ instruction_data ... ]
//! [ program_id: [u8; 32] ]
//! ```
//!
//! Two consequences the codegen relies on:
//! - the instruction-data length lives at `instruction_data_ptr - 8`;
//! - the program id immediately follows the instruction data.
//!
//! # Slice / `SolBytes` equivalence
//!
//! On the SVM's 64-bit target a `&[u8]` fat pointer has the same layout as the
//! syscall `SolBytes { addr: *const u8, len: u64 }` — a pointer followed by a
//! `u64` length. Syscalls that take `*const SolBytes` (e.g. `sol_sha256`) are
//! therefore fed Rust slice arrays directly, without any copy. The
//! [`SolBytes`] type below makes that layout explicit for code that needs a
//! *value* with slice layout whose pointee is mutated afterwards — which cannot
//! be expressed soundly with a real `&[u8]`.

/// A `(pointer, length)` pair with the same layout as a `&[u8]` fat pointer on
/// the SVM's 64-bit target, carrying no reference/borrow semantics.
///
/// Used where a slice-shaped value must be built and its pointee mutated later
/// (the PDA bump slot), which would be unsound to express with a real `&[u8]`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SolBytes {
    /// Pointer to the first byte.
    pub addr: *const u8,
    /// Length in bytes.
    pub len: u64,
}

impl SolBytes {
    /// Build a `SolBytes` from a byte slice (pointer + length, no copy).
    #[inline(always)]
    pub fn from_slice(s: &[u8]) -> Self {
        Self {
            addr: s.as_ptr(),
            len: s.len() as u64,
        }
    }

    /// Build a `SolBytes` from a raw pointer and length.
    ///
    /// # Safety
    ///
    /// The syscall consuming this value must only read `len` bytes at `addr`
    /// for the duration of the call.
    #[inline(always)]
    pub unsafe fn from_raw(addr: *const u8, len: usize) -> Self {
        Self {
            addr,
            len: len as u64,
        }
    }
}

// The `&[u8] == SolBytes` layout equivalence the syscall path relies on. The
// crate already requires a 64-bit target (see `__internal`), so `usize == u64`
// and both are a `(pointer, u64)` pair of the same size and alignment.
const _: () = assert!(core::mem::size_of::<&[u8]>() == core::mem::size_of::<SolBytes>());
const _: () = assert!(core::mem::align_of::<&[u8]>() == core::mem::align_of::<SolBytes>());
