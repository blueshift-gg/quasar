//! The 64-bit batched range proof instruction.

use {
    crate::proof_data::{batched_range_proof::BatchedRangeProofContext, ProofType, ZkProofData},
    bytemuck_derive::{Pod, Zeroable},
    solana_zk_sdk_pod::range_proof::PodRangeProofU64,
};

/// The instruction data that is needed for the
/// `ProofInstruction::VerifyBatchedRangeProofU64` instruction.
///
/// It includes the cryptographic proof as well as the context data information needed to verify
/// the proof.
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct BatchedRangeProofU64Data {
    /// The context data for a batched range proof
    pub context: BatchedRangeProofContext,

    /// The batched range proof
    pub proof: PodRangeProofU64,
}

impl ZkProofData<BatchedRangeProofContext> for BatchedRangeProofU64Data {
    const PROOF_TYPE: ProofType = ProofType::BatchedRangeProofU64;

    fn context_data(&self) -> &BatchedRangeProofContext {
        &self.context
    }
}
