//! Structured transaction logging via `sol_log_data`.

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
use solana_define_syscall::definitions::sol_log_data;

/// Write structured data to the transaction log.
///
/// Each slice becomes a separate base64-encoded field. No-op off-chain.
#[inline(always)]
pub fn log_data(data: &[&[u8]]) {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    // SAFETY: `sol_log_data` expects `(*const SolBytes, u64)` where `SolBytes`
    // has the same layout as `&[u8]` on SBF (`*const u8, u64`). The cast from
    // `&[&[u8]]` is layout-compatible.
    unsafe {
        sol_log_data(data.as_ptr() as *const u8, data.len() as u64);
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        core::hint::black_box(data);
    }
}
