//! The zero-ciphertext proof instruction.
//!
//! A zero-ciphertext proof is defined with respect to a twisted ElGamal ciphertext. The proof
//! certifies that a given ciphertext encrypts the message 0 in the field (`Scalar::zero()`). To
//! generate the proof, a prover must provide the decryption key for the ciphertext.

use {
    crate::proof_data::{ProofType, ZkProofData},
    bytemuck_derive::{Pod, Zeroable},
    solana_zk_sdk_pod::{
        encryption::elgamal::{PodElGamalCiphertext, PodElGamalPubkey},
        sigma_proofs::PodZeroCiphertextProof,
    },
};

/// The instruction data that is needed for the `ProofInstruction::VerifyZeroCiphertext` instruction.
///
/// It includes the cryptographic proof as well as the context data information needed to verify
/// the proof.
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct ZeroCiphertextProofData {
    /// The context data for the zero-ciphertext proof
    pub context: ZeroCiphertextProofContext, // 96 bytes

    /// Proof that the ciphertext is zero
    pub proof: PodZeroCiphertextProof, // 96 bytes
}

/// The context data needed to verify a zero-ciphertext proof.
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct ZeroCiphertextProofContext {
    /// The ElGamal pubkey associated with the ElGamal ciphertext
    pub pubkey: PodElGamalPubkey, // 32 bytes

    /// The ElGamal ciphertext that encrypts zero
    pub ciphertext: PodElGamalCiphertext, // 64 bytes
}

impl ZkProofData<ZeroCiphertextProofContext> for ZeroCiphertextProofData {
    const PROOF_TYPE: ProofType = ProofType::ZeroCiphertext;

    fn context_data(&self) -> &ZeroCiphertextProofContext {
        &self.context
    }
}
