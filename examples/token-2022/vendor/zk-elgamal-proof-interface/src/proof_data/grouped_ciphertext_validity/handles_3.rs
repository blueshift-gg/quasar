//! The grouped-ciphertext with 3 decryption handles validity proof instruction.
//!
//! A grouped-ciphertext validity proof certifies that a grouped ElGamal ciphertext is
//! well-defined, i.e. the ciphertext can be decrypted by private keys associated with its
//! decryption handles. To generate the proof, a prover must provide the Pedersen opening
//! associated with the grouped ciphertext's commitment.

use {
    crate::proof_data::{ProofType, ZkProofData},
    bytemuck_derive::{Pod, Zeroable},
    solana_zk_sdk_pod::{
        encryption::{
            elgamal::PodElGamalPubkey, grouped_elgamal::PodGroupedElGamalCiphertext3Handles,
        },
        sigma_proofs::PodGroupedCiphertext3HandlesValidityProof,
    },
};

/// The instruction data that is needed for the
/// `ProofInstruction::VerifyGroupedCiphertext3HandlesValidity` instruction.
///
/// It includes the cryptographic proof as well as the context data information needed to verify
/// the proof.
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct GroupedCiphertext3HandlesValidityProofData {
    pub context: GroupedCiphertext3HandlesValidityProofContext,

    pub proof: PodGroupedCiphertext3HandlesValidityProof,
}

#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct GroupedCiphertext3HandlesValidityProofContext {
    pub first_pubkey: PodElGamalPubkey, // 32 bytes

    pub second_pubkey: PodElGamalPubkey, // 32 bytes

    pub third_pubkey: PodElGamalPubkey, // 32 bytes

    pub grouped_ciphertext: PodGroupedElGamalCiphertext3Handles, // 128 bytes
}

impl ZkProofData<GroupedCiphertext3HandlesValidityProofContext>
    for GroupedCiphertext3HandlesValidityProofData
{
    const PROOF_TYPE: ProofType = ProofType::GroupedCiphertext3HandlesValidity;

    fn context_data(&self) -> &GroupedCiphertext3HandlesValidityProofContext {
        &self.context
    }
}
