//! The ciphertext-commitment equality proof instruction.
//!
//! A ciphertext-commitment equality proof is defined with respect to a twisted ElGamal ciphertext
//! and a Pedersen commitment. The proof certifies that a given ciphertext and a commitment pair
//! encrypts/encodes the same message. To generate the proof, a prover must provide the decryption
//! key for the first ciphertext and the Pedersen opening for the commitment.

use {
    crate::proof_data::{ProofType, ZkProofData},
    bytemuck_derive::{Pod, Zeroable},
    solana_zk_sdk_pod::{
        encryption::{
            elgamal::{PodElGamalCiphertext, PodElGamalPubkey},
            pedersen::PodPedersenCommitment,
        },
        sigma_proofs::PodCiphertextCommitmentEqualityProof,
    },
};

/// The instruction data that is needed for the
/// `ProofInstruction::VerifyCiphertextCommitmentEquality` instruction.
///
/// It includes the cryptographic proof as well as the context data information needed to verify
/// the proof.
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct CiphertextCommitmentEqualityProofData {
    pub context: CiphertextCommitmentEqualityProofContext,
    pub proof: PodCiphertextCommitmentEqualityProof,
}

/// The context data needed to verify a ciphertext-commitment equality proof.
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct CiphertextCommitmentEqualityProofContext {
    /// The ElGamal pubkey
    pub pubkey: PodElGamalPubkey, // 32 bytes

    /// The ciphertext encrypted under the ElGamal pubkey
    pub ciphertext: PodElGamalCiphertext, // 64 bytes

    /// The Pedersen commitment
    pub commitment: PodPedersenCommitment, // 32 bytes
}

impl ZkProofData<CiphertextCommitmentEqualityProofContext>
    for CiphertextCommitmentEqualityProofData
{
    const PROOF_TYPE: ProofType = ProofType::CiphertextCommitmentEquality;

    fn context_data(&self) -> &CiphertextCommitmentEqualityProofContext {
        &self.context
    }
}
