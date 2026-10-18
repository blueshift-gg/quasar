//! The only module that differs between builds: the SBF build calls the
//! runtime, the host build (for tests) has what it needs, and a Kani build
//! uses the models in `verify`.

/// A byte slice as syscalls read one: an address and a length. It is the
/// layout of the runtime's `SolSignerSeedC`, `SolSignerSeedsC` and of each
/// `sol_sha256` input. Unlike `&[u8]`, its bytes may change between uses.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawSlice {
    pub addr: *const u8,
    pub len: u64,
}

impl RawSlice {
    pub fn new(bytes: &[u8]) -> Self {
        Self { addr: bytes.as_ptr(), len: bytes.len() as u64 }
    }
}

#[cfg(target_os = "solana")]
mod sbf {
    use solana_define_syscall::{
        curve_constants::CURVE25519_EDWARDS,
        definitions::{
            abort, sol_curve_validate_point, sol_get_sysvar, sol_invoke_signed_c, sol_log_,
            sol_log_data, sol_set_return_data, sol_sha256,
        },
    };

    use core::{mem::MaybeUninit, ptr};

    use super::RawSlice;
    use crate::{Address, ProgramError, Result};

    #[panic_handler]
    fn panic(_: &core::panic::PanicInfo) -> ! {
        // SAFETY: `abort` ends the program and never returns.
        unsafe { abort() }
    }

    pub fn log(message: &str) {
        // SAFETY: the syscall reads `message.len()` bytes from `message`.
        unsafe { sol_log_(message.as_ptr(), message.len() as u64) }
    }

    /// Ends the program.
    #[cold]
    pub fn end() -> ! {
        // SAFETY: `abort` ends the program and never returns.
        unsafe { abort() }
    }

    pub fn set_return_data(data: &[u8]) {
        // SAFETY: the syscall reads `data.len()` bytes from `data`.
        unsafe { sol_set_return_data(data.as_ptr(), data.len() as u64) }
    }

    pub fn log_data(parts: &[&[u8]]) {
        // SAFETY: the syscall reads `parts.len()` slices, each laid out as a
        // pointer and a length, which is how `&[u8]` is represented.
        unsafe { sol_log_data(parts.as_ptr().cast(), parts.len() as u64) }
    }

    /// # Safety
    /// The arguments are laid out as `sol_invoke_signed_c` requires.
    #[inline(always)]
    pub unsafe fn invoke(
        instruction: *const u8,
        infos: *const u8,
        infos_len: u64,
        signers: *const u8,
        signers_len: u64,
    ) {
        // SAFETY: forwarded from this function's contract. A failing callee
        // aborts the caller; the syscall itself only returns success.
        unsafe { sol_invoke_signed_c(instruction, infos, infos_len, signers, signers_len) };
    }

    /// # Safety
    /// Each part points to `len` readable bytes.
    #[inline(always)]
    pub unsafe fn sha256<'a>(
        parts: &[RawSlice],
        hash: &'a mut MaybeUninit<[u8; 32]>,
    ) -> &'a [u8; 32] {
        // SAFETY: forwarded from this function's contract; the output is 32 bytes.
        unsafe { sol_sha256(parts.as_ptr().cast(), parts.len() as u64, hash.as_mut_ptr().cast()) };
        // SAFETY: the syscall wrote all 32 bytes.
        unsafe { hash.assume_init_ref() }
    }

    #[inline(always)]
    pub fn is_on_curve(bytes: &[u8; 32]) -> bool {
        // SAFETY: the syscall reads 32 bytes and ignores its third argument,
        // so no result buffer: one would share a stack slot with the hash
        // and force a copy of it per bump (measured).
        let point = bytes.as_ptr();
        unsafe { sol_curve_validate_point(CURVE25519_EDWARDS, point, ptr::null_mut()) == 0 }
    }

    /// Reads `out.len()` bytes of a sysvar, from its start.
    pub fn sysvar(id: &Address, out: &mut [u8]) -> Result {
        // SAFETY: the syscall writes `out.len()` bytes into `out`.
        let status =
            unsafe { sol_get_sysvar(id.as_ref().as_ptr(), out.as_mut_ptr(), 0, out.len() as u64) };
        if status == 0 { Ok(()) } else { Err(ProgramError::UnsupportedSysvar) }
    }
}

#[cfg(all(not(target_os = "solana"), not(kani)))]
mod host {
    use crate::{Address, Result};

    pub fn log(message: &str) {
        std::eprintln!("{message}");
    }

    pub fn end() -> ! {
        panic!("the program ended")
    }

    pub fn set_return_data(_: &[u8]) {
        unreachable!("syscalls run only on the SBF build")
    }

    pub fn log_data(_: &[&[u8]]) {
        unreachable!("syscalls run only on the SBF build")
    }

    /// # Safety
    /// The arguments are laid out as `sol_invoke_signed_c` requires.
    pub unsafe fn invoke(
        _instruction: *const u8,
        _infos: *const u8,
        _infos_len: u64,
        _signers: *const u8,
        _signers_len: u64,
    ) {
        #[cfg(test)]
        if let Some(invoke) = crate::cpi::miri::INVOKE.get() {
            // SAFETY: forwarded from this function's contract; the test hook reads the ABI.
            unsafe { invoke(_instruction, _infos, _infos_len, _signers, _signers_len) };
            return;
        }
        unreachable!("syscalls run only on the SBF build")
    }

    /// # Safety
    /// Each part points to `len` readable bytes.
    pub unsafe fn sha256<'a>(
        parts: &[super::RawSlice],
        hash: &'a mut core::mem::MaybeUninit<[u8; 32]>,
    ) -> &'a [u8; 32] {
        let mut hasher = solana_sha256_hasher::Hasher::default();
        for part in parts {
            // SAFETY: forwarded from this function's contract.
            hasher.hash(unsafe { core::slice::from_raw_parts(part.addr, part.len as usize) });
        }
        hash.write(hasher.result().to_bytes())
    }

    pub fn is_on_curve(bytes: &[u8; 32]) -> bool {
        crate::pda::is_on_curve(bytes)
    }

    pub fn sysvar(_: &Address, _: &mut [u8]) -> Result {
        unreachable!("syscalls run only on the SBF build")
    }
}

#[cfg(target_os = "solana")]
pub use sbf::*;

#[cfg(all(not(target_os = "solana"), not(kani)))]
pub use host::*;

#[cfg(kani)]
pub use crate::verify::models::*;
