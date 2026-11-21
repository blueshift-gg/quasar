//! The ciphertext-ciphertext equality proof instruction.
//!
//! A ciphertext-ciphertext equality proof is defined with respect to two twisted ElGamal
//! ciphertexts. The proof certifies that the two ciphertexts encrypt the same message. To generate
//! the proof, a prover must provide the decryption key for the first ciphertext and the randomness
//! used to generate the second ciphertext.

use {
    crate::proof_data::{ProofType, ZkProofData},
    bytemuck_derive::{Pod, Zeroable},
    solana_zk_sdk_pod::{
        encryption::elgamal::{PodElGamalCiphertext, PodElGamalPubkey},
        sigma_proofs::PodCiphertextCiphertextEqualityProof,
    },
};

/// The instruction data that is needed for the
/// `ProofInstruction::VerifyCiphertextCiphertextEquality` instruction.
///
/// It includes the cryptographic proof as well as the context data information needed to verify
/// the proof.
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct CiphertextCiphertextEqualityProofData {
    pub context: CiphertextCiphertextEqualityProofContext,

    pub proof: PodCiphertextCiphertextEqualityProof,
}

/// The context data needed to verify a ciphertext-ciphertext equality proof.
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct CiphertextCiphertextEqualityProofContext {
    pub first_pubkey: PodElGamalPubkey, // 32 bytes

    pub second_pubkey: PodElGamalPubkey, // 32 bytes

    pub first_ciphertext: PodElGamalCiphertext, // 64 bytes

    pub second_ciphertext: PodElGamalCiphertext, // 64 bytes
}

impl ZkProofData<CiphertextCiphertextEqualityProofContext>
    for CiphertextCiphertextEqualityProofData
{
    const PROOF_TYPE: ProofType = ProofType::CiphertextCiphertextEquality;

    fn context_data(&self) -> &CiphertextCiphertextEqualityProofContext {
        &self.context
    }
}
