//! The percentage-with-cap proof instruction.
//!
//! The percentage-with-cap proof is defined with respect to three Pedersen commitments that
//! encodes values referred to as a `percentage`, `delta`, and `claimed` amounts. The proof
//! certifies that either
//! - the `percentage` amount is equal to a constant (referred to as the `max_value`)
//! - the `delta` and `claimed` amounts are equal

use {
    crate::proof_data::{ProofType, ZkProofData},
    bytemuck_derive::{Pod, Zeroable},
    solana_zk_sdk_pod::{
        encryption::pedersen::PodPedersenCommitment, primitive_types::PodU64,
        sigma_proofs::PodPercentageWithCapProof,
    },
};

/// The instruction data that is needed for the `ProofInstruction::VerifyPercentageWithCap`
/// instruction.
///
/// It includes the cryptographic proof as well as the context data information needed to verify
/// the proof.
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct PercentageWithCapProofData {
    pub context: PercentageWithCapProofContext,

    pub proof: PodPercentageWithCapProof,
}

/// The context data needed to verify a percentage-with-cap proof.
///
/// We refer to [`ZK ElGamal proof`] for the formal details on how the percentage-with-cap proof is
/// computed.
///
/// [`ZK ElGamal proof`]: https://docs.solanalabs.com/runtime/zk-token-proof
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct PercentageWithCapProofContext {
    /// The Pedersen commitment to the percentage amount.
    pub percentage_commitment: PodPedersenCommitment,

    /// The Pedersen commitment to the delta amount.
    pub delta_commitment: PodPedersenCommitment,

    /// The Pedersen commitment to the claimed amount.
    pub claimed_commitment: PodPedersenCommitment,

    /// The maximum cap bound.
    pub max_value: PodU64,
}

impl ZkProofData<PercentageWithCapProofContext> for PercentageWithCapProofData {
    const PROOF_TYPE: ProofType = ProofType::PercentageWithCap;

    fn context_data(&self) -> &PercentageWithCapProofContext {
        &self.context
    }
}
