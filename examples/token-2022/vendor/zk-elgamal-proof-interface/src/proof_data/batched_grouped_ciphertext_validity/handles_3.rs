//! The batched grouped-ciphertext with 3 handles validity proof instruction.
//!
//! A batched grouped-ciphertext validity proof certifies the validity of two grouped ElGamal
//! ciphertext that are encrypted using the same set of ElGamal public keys. A batched
//! grouped-ciphertext validity proof is shorter and more efficient than two individual
//! grouped-ciphertext validity proofs.

use {
    crate::proof_data::{ProofType, ZkProofData},
    bytemuck_derive::{Pod, Zeroable},
    solana_zk_sdk_pod::{
        encryption::{
            elgamal::PodElGamalPubkey, grouped_elgamal::PodGroupedElGamalCiphertext3Handles,
        },
        sigma_proofs::PodBatchedGroupedCiphertext3HandlesValidityProof,
    },
};

/// The instruction data that is needed for the
/// `ProofInstruction::VerifyBatchedGroupedCiphertext3HandlesValidity` instruction.
///
/// It includes the cryptographic proof as well as the context data information needed to verify
/// the proof.
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct BatchedGroupedCiphertext3HandlesValidityProofData {
    pub context: BatchedGroupedCiphertext3HandlesValidityProofContext,

    pub proof: PodBatchedGroupedCiphertext3HandlesValidityProof,
}

#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct BatchedGroupedCiphertext3HandlesValidityProofContext {
    pub first_pubkey: PodElGamalPubkey, // 32 bytes

    pub second_pubkey: PodElGamalPubkey, // 32 bytes

    pub third_pubkey: PodElGamalPubkey, // 32 bytes

    pub grouped_ciphertext_lo: PodGroupedElGamalCiphertext3Handles, // 128 bytes

    pub grouped_ciphertext_hi: PodGroupedElGamalCiphertext3Handles, // 128 bytes
}

impl ZkProofData<BatchedGroupedCiphertext3HandlesValidityProofContext>
    for BatchedGroupedCiphertext3HandlesValidityProofData
{
    const PROOF_TYPE: ProofType = ProofType::BatchedGroupedCiphertext3HandlesValidity;

    fn context_data(&self) -> &BatchedGroupedCiphertext3HandlesValidityProofContext {
        &self.context
    }
}
