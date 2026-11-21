//! The batched grouped-ciphertext validity proof instruction.
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
            elgamal::PodElGamalPubkey, grouped_elgamal::PodGroupedElGamalCiphertext2Handles,
        },
        sigma_proofs::PodBatchedGroupedCiphertext2HandlesValidityProof,
    },
};

/// The instruction data that is needed for the
/// `ProofInstruction::VerifyBatchedGroupedCiphertextValidity` instruction.
///
/// It includes the cryptographic proof as well as the context data information needed to verify
/// the proof.
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct BatchedGroupedCiphertext2HandlesValidityProofData {
    pub context: BatchedGroupedCiphertext2HandlesValidityProofContext,

    pub proof: PodBatchedGroupedCiphertext2HandlesValidityProof,
}

#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct BatchedGroupedCiphertext2HandlesValidityProofContext {
    pub first_pubkey: PodElGamalPubkey, // 32 bytes

    pub second_pubkey: PodElGamalPubkey, // 32 bytes

    pub grouped_ciphertext_lo: PodGroupedElGamalCiphertext2Handles, // 96 bytes

    pub grouped_ciphertext_hi: PodGroupedElGamalCiphertext2Handles, // 96 bytes
}

impl ZkProofData<BatchedGroupedCiphertext2HandlesValidityProofContext>
    for BatchedGroupedCiphertext2HandlesValidityProofData
{
    const PROOF_TYPE: ProofType = ProofType::BatchedGroupedCiphertext2HandlesValidity;

    fn context_data(&self) -> &BatchedGroupedCiphertext2HandlesValidityProofContext {
        &self.context
    }
}
