//! The batched range proof instructions.
//!
//! A batched range proof is a cryptographic method that proves a set of committed values
//! fall within specified bit-ranges, without revealing the values themselves. It is more
//! efficient than verifying individual range proofs for each commitment.
//!
//! This module provides three instructions for batched range proofs, each corresponding to a
//! different total bit length:
//! - `VerifyBatchedRangeProof64`: For proofs where the sum of bit lengths is 64.
//! - `VerifyBatchedRangeProof128`: For proofs where the sum of bit lengths is 128.
//! - `VerifyBatchedRangeProof256`: For proofs where the sum of bit lengths is 256.
//!
//! For example, to generate a batched range proof for a sequence of commitments `[C_1, C_2, C_3]`
//! with corresponding bit-lengths `[32, 32, 64]`, one must use `VerifyBatchedRangeProof128`,
//! since the sum of bit-lengths is `32 + 32 + 64 = 128`.
//!
//! The maximum number of commitments that can be batched together is fixed at 8. Each individual
//! bit length `n_i` must be at most 128.

use solana_zk_sdk_pod::encryption::pedersen::PodPedersenCommitment;

pub mod batched_range_proof_u128;
pub mod batched_range_proof_u256;
pub mod batched_range_proof_u64;

pub use {batched_range_proof_u128::*, batched_range_proof_u256::*, batched_range_proof_u64::*};

/// The maximum number of Pedersen commitments that can be processed in a single batched range proof.
pub const MAX_COMMITMENTS: usize = 8;

/// The context data needed to verify a range-proof for a Pedersen committed value.
///
/// This struct holds the public information that a batched range proof certifies. It includes the
/// Pedersen commitments and their corresponding bit lengths. This context is shared by all
/// `VerifyBatchedRangeProof{N}` instructions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck_derive::Pod, bytemuck_derive::Zeroable)]
#[repr(C)]
pub struct BatchedRangeProofContext {
    pub commitments: [PodPedersenCommitment; MAX_COMMITMENTS],
    pub bit_lengths: [u8; MAX_COMMITMENTS],
}
