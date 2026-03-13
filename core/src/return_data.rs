//! Instruction return data via `sol_set_return_data`.

/// Set the return data for the current instruction. No-op off-chain.
#[inline(always)]
pub fn set_return_data(_data: &[u8]) {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    // SAFETY: `sol_set_return_data` reads `_data.len()` bytes from `_data.as_ptr()`.
    // Both are valid for the lifetime of this call.
    unsafe {
        solana_define_syscall::definitions::sol_set_return_data(_data.as_ptr(), _data.len() as u64);
    }
}
