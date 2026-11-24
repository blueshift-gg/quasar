//! The grouped-ciphertext validity proof instruction.
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
            elgamal::PodElGamalPubkey, grouped_elgamal::PodGroupedElGamalCiphertext2Handles,
        },
        sigma_proofs::PodGroupedCiphertext2HandlesValidityProof,
    },
};

/// The instruction data that is needed for the `ProofInstruction::VerifyGroupedCiphertextValidity`
/// instruction.
///
/// It includes the cryptographic proof as well as the context data information needed to verify
/// the proof.
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct GroupedCiphertext2HandlesValidityProofData {
    pub context: GroupedCiphertext2HandlesValidityProofContext,

    pub proof: PodGroupedCiphertext2HandlesValidityProof,
}

#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct GroupedCiphertext2HandlesValidityProofContext {
    pub first_pubkey: PodElGamalPubkey, // 32 bytes

    pub second_pubkey: PodElGamalPubkey, // 32 bytes

    pub grouped_ciphertext: PodGroupedElGamalCiphertext2Handles, // 96 bytes
}

impl ZkProofData<GroupedCiphertext2HandlesValidityProofContext>
    for GroupedCiphertext2HandlesValidityProofData
{
    const PROOF_TYPE: ProofType = ProofType::GroupedCiphertext2HandlesValidity;

    fn context_data(&self) -> &GroupedCiphertext2HandlesValidityProofContext {
        &self.context
    }
}
