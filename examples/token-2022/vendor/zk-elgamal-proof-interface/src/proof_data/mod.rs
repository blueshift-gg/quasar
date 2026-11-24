use {
    bytemuck_derive::{Pod, Zeroable},
    core::fmt,
    num_derive::{FromPrimitive, ToPrimitive},
    num_traits::{FromPrimitive, ToPrimitive},
};

pub mod batched_grouped_ciphertext_validity;
pub mod batched_range_proof;
pub mod ciphertext_ciphertext_equality;
pub mod ciphertext_commitment_equality;
pub mod grouped_ciphertext_validity;
pub mod percentage_with_cap;
pub mod pubkey_validity;
pub mod zero_ciphertext;

pub use {
    batched_grouped_ciphertext_validity::*, batched_range_proof::*,
    ciphertext_ciphertext_equality::*, ciphertext_commitment_equality::*,
    grouped_ciphertext_validity::*, percentage_with_cap::*, pubkey_validity::*, zero_ciphertext::*,
};

#[derive(Clone, Copy, Debug, FromPrimitive, ToPrimitive, PartialEq, Eq)]
#[repr(u8)]
pub enum ProofType {
    /// Empty proof type used to distinguish if a proof context account is initialized
    Uninitialized,
    ZeroCiphertext,
    CiphertextCiphertextEquality,
    CiphertextCommitmentEquality,
    PubkeyValidity,
    PercentageWithCap,
    BatchedRangeProofU64,
    BatchedRangeProofU128,
    BatchedRangeProofU256,
    GroupedCiphertext2HandlesValidity,
    BatchedGroupedCiphertext2HandlesValidity,
    GroupedCiphertext3HandlesValidity,
    BatchedGroupedCiphertext3HandlesValidity,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodProofType(pub u8);

impl From<ProofType> for PodProofType {
    fn from(proof_type: ProofType) -> Self {
        Self(ToPrimitive::to_u8(&proof_type).unwrap())
    }
}

/// Error returned when attempting to parse an invalid proof type byte.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofTypeError {
    InvalidProofType,
}

impl fmt::Display for ProofTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid proof type")
    }
}

impl core::error::Error for ProofTypeError {}

impl TryFrom<PodProofType> for ProofType {
    type Error = ProofTypeError;

    fn try_from(pod: PodProofType) -> Result<Self, Self::Error> {
        FromPrimitive::from_u8(pod.0).ok_or(ProofTypeError::InvalidProofType)
    }
}

pub trait ZkProofData<T: bytemuck::Pod> {
    const PROOF_TYPE: ProofType;

    fn context_data(&self) -> &T;
}
