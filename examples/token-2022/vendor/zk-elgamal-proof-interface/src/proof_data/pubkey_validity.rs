//! The public-key validity proof instruction.
//!
//! A public-key validity proof system is defined with respect to an ElGamal public key. The proof
//! certifies that a given public key is a valid ElGamal public key (i.e. the prover knows a
//! corresponding secret key). To generate the proof, a prover must provide the secret key for the
//! public key.

use {
    crate::proof_data::{ProofType, ZkProofData},
    bytemuck_derive::{Pod, Zeroable},
    solana_zk_sdk_pod::{
        encryption::elgamal::PodElGamalPubkey, sigma_proofs::PodPubkeyValidityProof,
    },
};

/// The instruction data that is needed for the `ProofInstruction::VerifyPubkeyValidity`
/// instruction.
///
/// It includes the cryptographic proof as well as the context data information needed to verify
/// the proof.
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct PubkeyValidityProofData {
    /// The context data for the public key validity proof
    pub context: PubkeyValidityProofContext, // 32 bytes

    /// Proof that the public key is well-formed
    pub proof: PodPubkeyValidityProof, // 64 bytes
}

/// The context data needed to verify a pubkey validity proof.
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct PubkeyValidityProofContext {
    /// The public key to be proved
    pub pubkey: PodElGamalPubkey, // 32 bytes
}

impl ZkProofData<PubkeyValidityProofContext> for PubkeyValidityProofData {
    const PROOF_TYPE: ProofType = ProofType::PubkeyValidity;

    fn context_data(&self) -> &PubkeyValidityProofContext {
        &self.context
    }
}
