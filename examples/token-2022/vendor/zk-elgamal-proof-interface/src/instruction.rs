//! Instructions provided by the [`ZK ElGamal proof`] program.
//!
//! There are two types of instructions in the proof program: proof verification instructions and
//! the `CloseContextState` instruction.
//!
//! Each proof verification instruction verifies a certain type of zero-knowledge proof. These
//! instructions are processed by the program in two steps:
//!   1. The program verifies the zero-knowledge proof.
//!   2. The program optionally stores the context component of the zero-knowledge proof to a
//!      dedicated [`context-state`] account.
//!
//! In step 1, the zero-knowledge proof can either be included directly as the instruction data or
//! pre-written to an account. The program determines the mode by inspecting the length of the
//! instruction data.
//!
//! **Case A: Proof in a separate account**
//! If the instruction data is exactly 5 bytes (1-byte instruction discriminator plus 4-byte unsigned
//! integer for offset), the program assumes that the first account provided with the instruction
//! contains the zero-knowledge proof. It then verifies the account data at the offset specified in
//! the instruction.
//!
//! **Case B: Proof in instruction data**
//! If two additional accounts are provided (for the context state and its owner), the program
//! interprets this as a request to store the proof's context data and writes it to the specified
//! context-state account.
//!
//! In step 2, the program determines whether to create a context-state account by inspecting the
//! number of accounts provided with the instruction. If two additional accounts are provided with
//! the instruction after verifying the zero-knowledge proof, then the program writes the context
//! data to the specified context-state account.
//!
//! NOTE: A context-state account must be pre-allocated to the exact size of the context data that
//! is expected for a proof type before it is included as part of a proof verification instruction.
//!
//! The `CloseContextState` instruction closes a context state account. A transaction containing
//! this instruction must be signed by the context account's owner. This instruction can be used by
//! the account owner to reclaim lamports for storage.
//!
//! [`ZK ElGamal proof`]: https://docs.solanalabs.com/runtime/zk-token-proof
//! [`context-state`]: https://docs.solanalabs.com/runtime/zk-token-proof#context-data

use {
    crate::proof_data::ZkProofData,
    alloc::vec,
    bytemuck::{bytes_of, Pod},
    num_derive::{FromPrimitive, ToPrimitive},
    num_traits::{FromPrimitive, ToPrimitive},
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
};

#[derive(Clone, Copy, Debug, FromPrimitive, ToPrimitive, PartialEq, Eq)]
#[repr(u8)]
pub enum ProofInstruction {
    /// Close a zero-knowledge proof context state.
    ///
    /// Accounts expected by this instruction:
    ///   0. `[writable]` The proof context account to close
    ///   1. `[writable]` The destination account for lamports
    ///   2. `[signer]` The context account's owner
    ///
    /// Data expected by this instruction:
    ///   None
    ///
    CloseContextState,

    /// Verify a zero-ciphertext proof.
    ///
    /// A zero-ciphertext proof certifies that an ElGamal ciphertext encrypts the value zero.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   There are four ways to structure the accounts, depending on whether the
    ///   proof is provided as instruction data or in a separate account, and whether
    ///   a proof context is created.
    ///
    ///   1. **Proof in instruction data, no context state:**
    ///      - No accounts are required.
    ///
    ///   2. **Proof in instruction data, with context state:**
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    ///   3. **Proof in account, no context state:**
    ///      - `[]` Account to read the proof from.
    ///
    ///   4. **Proof in account, with context state:**
    ///      - `[]` Account to read the proof from.
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    /// The instruction expects either:
    ///   i. `ZeroCiphertextProofData` if proof is provided as instruction data
    ///   ii. `u32` byte offset if proof is provided as an account
    ///
    /// Note that the context state account is not required to be a signer and
    /// MUST be included within the same Transaction as the system program's
    /// `CreateAccount` instruction that creates the account being initialized.
    /// Otherwise another party can acquire ownership of the uninitialized
    /// account.
    ///
    VerifyZeroCiphertext,

    /// Verify a ciphertext-ciphertext equality proof.
    ///
    /// A ciphertext-ciphertext equality proof certifies that two ElGamal ciphertexts encrypt the
    /// same message.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   There are four ways to structure the accounts, depending on whether the
    ///   proof is provided as instruction data or in a separate account, and whether
    ///   a proof context is created.
    ///
    ///   1. **Proof in instruction data, no context state:**
    ///      - No accounts are required.
    ///
    ///   2. **Proof in instruction data, with context state:**
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    ///   3. **Proof in account, no context state:**
    ///      - `[]` Account to read the proof from.
    ///
    ///   4. **Proof in account, with context state:**
    ///      - `[]` Account to read the proof from.
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    /// The instruction expects either:
    ///   i. `CiphertextCiphertextEqualityProofData` if proof is provided as instruction data
    ///   ii. `u32` byte offset if proof is provided as an account
    ///
    /// Note that the context state account is not required to be a signer and
    /// MUST be included within the same Transaction as the system program's
    /// `CreateAccount` instruction that creates the account being initialized.
    /// Otherwise another party can acquire ownership of the uninitialized
    /// account.
    ///
    VerifyCiphertextCiphertextEquality,

    /// Verify a ciphertext-commitment equality proof.
    ///
    /// A ciphertext-commitment equality proof certifies that an ElGamal ciphertext and a Pedersen
    /// commitment encrypt/encode the same message.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   There are four ways to structure the accounts, depending on whether the
    ///   proof is provided as instruction data or in a separate account, and whether
    ///   a proof context is created.
    ///
    ///   1. **Proof in instruction data, no context state:**
    ///      - No accounts are required.
    ///
    ///   2. **Proof in instruction data, with context state:**
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    ///   3. **Proof in account, no context state:**
    ///      - `[]` Account to read the proof from.
    ///
    ///   4. **Proof in account, with context state:**
    ///      - `[]` Account to read the proof from.
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    /// The instruction expects either:
    ///   i. `CiphertextCommitmentEqualityProofData` if proof is provided as instruction data
    ///   ii. `u32` byte offset if proof is provided as an account
    ///
    /// Note that the context state account is not required to be a signer and
    /// MUST be included within the same Transaction as the system program's
    /// `CreateAccount` instruction that creates the account being initialized.
    /// Otherwise another party can acquire ownership of the uninitialized
    /// account.
    ///
    VerifyCiphertextCommitmentEquality,

    /// Verify a public key validity zero-knowledge proof.
    ///
    /// A public key validity proof certifies that an ElGamal public key is well-formed and the
    /// prover knows the corresponding secret key.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   There are four ways to structure the accounts, depending on whether the
    ///   proof is provided as instruction data or in a separate account, and whether
    ///   a proof context is created.
    ///
    ///   1. **Proof in instruction data, no context state:**
    ///      - No accounts are required.
    ///
    ///   2. **Proof in instruction data, with context state:**
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    ///   3. **Proof in account, no context state:**
    ///      - `[]` Account to read the proof from.
    ///
    ///   4. **Proof in account, with context state:**
    ///      - `[]` Account to read the proof from.
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    /// The instruction expects either:
    ///   i. `PubkeyValidityData` if proof is provided as instruction data
    ///   ii. `u32` byte offset if proof is provided as an account
    ///
    /// Note that the context state account is not required to be a signer and
    /// MUST be included within the same Transaction as the system program's
    /// `CreateAccount` instruction that creates the account being initialized.
    /// Otherwise another party can acquire ownership of the uninitialized
    /// account.
    ///
    VerifyPubkeyValidity,

    /// Verify a percentage-with-cap proof.
    ///
    /// A percentage-with-cap proof certifies that a tuple of Pedersen commitments satisfy a
    /// percentage relation.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   There are four ways to structure the accounts, depending on whether the
    ///   proof is provided as instruction data or in a separate account, and whether
    ///   a proof context is created.
    ///
    ///   1. **Proof in instruction data, no context state:**
    ///      - No accounts are required.
    ///
    ///   2. **Proof in instruction data, with context state:**
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    ///   3. **Proof in account, no context state:**
    ///      - `[]` Account to read the proof from.
    ///
    ///   4. **Proof in account, with context state:**
    ///      - `[]` Account to read the proof from.
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    /// The instruction expects either:
    ///   i. `PercentageWithCapProofData` if proof is provided as instruction data
    ///   ii. `u32` byte offset if proof is provided as an account
    ///
    /// Note that the context state account is not required to be a signer and
    /// MUST be included within the same Transaction as the system program's
    /// `CreateAccount` instruction that creates the account being initialized.
    /// Otherwise another party can acquire ownership of the uninitialized
    /// account.
    ///
    VerifyPercentageWithCap,

    /// Verify a 64-bit batched range proof.
    ///
    /// A batched range proof is defined with respect to a sequence of Pedersen commitments `[C_1,
    /// ..., C_N]` and bit-lengths `[n_1, ..., n_N]`. It certifies that each commitment `C_i` is a
    /// commitment to a positive number of bit-length `n_i`. Batch verifying range proofs is more
    /// efficient than verifying independent range proofs on commitments `C_1, ..., C_N`
    /// separately.
    ///
    /// The bit-length of a batched range proof specifies the sum of the individual bit-lengths
    /// `n_1, ..., n_N`. For example, this instruction can be used to certify that two commitments
    /// `C_1` and `C_2` each hold positive 32-bit numbers.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   There are four ways to structure the accounts, depending on whether the
    ///   proof is provided as instruction data or in a separate account, and whether
    ///   a proof context is created.
    ///
    ///   1. **Proof in instruction data, no context state:**
    ///      - No accounts are required.
    ///
    ///   2. **Proof in instruction data, with context state:**
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    ///   3. **Proof in account, no context state:**
    ///      - `[]` Account to read the proof from.
    ///
    ///   4. **Proof in account, with context state:**
    ///      - `[]` Account to read the proof from.
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    /// The instruction expects either:
    ///   i. `BatchedRangeProofU64Data` if proof is provided as instruction data
    ///   ii. `u32` byte offset if proof is provided as an account
    ///
    /// Note that the context state account is not required to be a signer and
    /// MUST be included within the same Transaction as the system program's
    /// `CreateAccount` instruction that creates the account being initialized.
    /// Otherwise another party can acquire ownership of the uninitialized
    /// account.
    ///
    VerifyBatchedRangeProofU64,

    /// Verify 128-bit batched range proof.
    ///
    /// The bit-length of a batched range proof specifies the sum of the individual bit-lengths
    /// `n_1, ..., n_N`. For example, this instruction can be used to certify that two commitments
    /// `C_1` and `C_2` each hold positive 64-bit numbers.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   There are four ways to structure the accounts, depending on whether the
    ///   proof is provided as instruction data or in a separate account, and whether
    ///   a proof context is created.
    ///
    ///   1. **Proof in instruction data, no context state:**
    ///      - No accounts are required.
    ///
    ///   2. **Proof in instruction data, with context state:**
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    ///   3. **Proof in account, no context state:**
    ///      - `[]` Account to read the proof from.
    ///
    ///   4. **Proof in account, with context state:**
    ///      - `[]` Account to read the proof from.
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    /// The instruction expects either:
    ///   i. `BatchedRangeProofU128Data` if proof is provided as instruction data
    ///   ii. `u32` byte offset if proof is provided as an account
    ///
    /// Note that the context state account is not required to be a signer and
    /// MUST be included within the same Transaction as the system program's
    /// `CreateAccount` instruction that creates the account being initialized.
    /// Otherwise another party can acquire ownership of the uninitialized
    /// account.
    ///
    VerifyBatchedRangeProofU128,

    /// Verify 256-bit batched range proof.
    ///
    /// The bit-length of a batched range proof specifies the sum of the individual bit-lengths
    /// `n_1, ..., n_N`. For example, this instruction can be used to certify that four commitments
    /// `[C_1, C_2, C_3, C_4]` each hold positive 64-bit numbers.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   There are four ways to structure the accounts, depending on whether the
    ///   proof is provided as instruction data or in a separate account, and whether
    ///   a proof context is created.
    ///
    ///   1. **Proof in instruction data, no context state:**
    ///      - No accounts are required.
    ///
    ///   2. **Proof in instruction data, with context state:**
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    ///   3. **Proof in account, no context state:**
    ///      - `[]` Account to read the proof from.
    ///
    ///   4. **Proof in account, with context state:**
    ///      - `[]` Account to read the proof from.
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    /// The instruction expects either:
    ///   i. `BatchedRangeProofU256Data` if proof is provided as instruction data
    ///   ii. `u32` byte offset if proof is provided as an account
    ///
    /// Note that the context state account is not required to be a signer and
    /// MUST be included within the same Transaction as the system program's
    /// `CreateAccount` instruction that creates the account being initialized.
    /// Otherwise another party can acquire ownership of the uninitialized
    /// account.
    ///
    VerifyBatchedRangeProofU256,

    /// Verify a grouped-ciphertext with 2 handles validity proof.
    ///
    /// A grouped-ciphertext validity proof certifies that a grouped ElGamal ciphertext is
    /// well-defined, i.e. the ciphertext can be decrypted by private keys associated with its
    /// decryption handles.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   There are four ways to structure the accounts, depending on whether the
    ///   proof is provided as instruction data or in a separate account, and whether
    ///   a proof context is created.
    ///
    ///   1. **Proof in instruction data, no context state:**
    ///      - No accounts are required.
    ///
    ///   2. **Proof in instruction data, with context state:**
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    ///   3. **Proof in account, no context state:**
    ///      - `[]` Account to read the proof from.
    ///
    ///   4. **Proof in account, with context state:**
    ///      - `[]` Account to read the proof from.
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    /// The instruction expects either:
    ///   i. `GroupedCiphertext2HandlesValidityProofData` if proof is provided as instruction data
    ///   ii. `u32` byte offset if proof is provided as an account
    ///
    /// Note that the context state account is not required to be a signer and
    /// MUST be included within the same Transaction as the system program's
    /// `CreateAccount` instruction that creates the account being initialized.
    /// Otherwise another party can acquire ownership of the uninitialized
    /// account.
    ///
    VerifyGroupedCiphertext2HandlesValidity,

    /// Verify a batched grouped-ciphertext with 2 handles validity proof.
    ///
    /// A batched grouped-ciphertext validity proof certifies the validity of two grouped ElGamal
    /// ciphertext that are encrypted using the same set of ElGamal public keys. A batched
    /// grouped-ciphertext validity proof is shorter and more efficient than two individual
    /// grouped-ciphertext validity proofs.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   There are four ways to structure the accounts, depending on whether the
    ///   proof is provided as instruction data or in a separate account, and whether
    ///   a proof context is created.
    ///
    ///   1. **Proof in instruction data, no context state:**
    ///      - No accounts are required.
    ///
    ///   2. **Proof in instruction data, with context state:**
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    ///   3. **Proof in account, no context state:**
    ///      - `[]` Account to read the proof from.
    ///
    ///   4. **Proof in account, with context state:**
    ///      - `[]` Account to read the proof from.
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    /// The instruction expects either:
    ///   i. `BatchedGroupedCiphertext2HandlesValidityProofData` if proof is provided as instruction data
    ///   ii. `u32` byte offset if proof is provided as an account
    ///
    /// Note that the context state account is not required to be a signer and
    /// MUST be included within the same Transaction as the system program's
    /// `CreateAccount` instruction that creates the account being initialized.
    /// Otherwise another party can acquire ownership of the uninitialized
    /// account.
    ///
    VerifyBatchedGroupedCiphertext2HandlesValidity,

    /// Verify a grouped-ciphertext with 3 handles validity proof.
    ///
    /// A grouped-ciphertext validity proof certifies that a grouped ElGamal ciphertext is
    /// well-defined, i.e. the ciphertext can be decrypted by private keys associated with its
    /// decryption handles.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   There are four ways to structure the accounts, depending on whether the
    ///   proof is provided as instruction data or in a separate account, and whether
    ///   a proof context is created.
    ///
    ///   1. **Proof in instruction data, no context state:**
    ///      - No accounts are required.
    ///
    ///   2. **Proof in instruction data, with context state:**
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    ///   3. **Proof in account, no context state:**
    ///      - `[]` Account to read the proof from.
    ///
    ///   4. **Proof in account, with context state:**
    ///      - `[]` Account to read the proof from.
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    /// The instruction expects either:
    ///   i. `GroupedCiphertext3HandlesValidityProofData` if proof is provided as instruction data
    ///   ii. `u32` byte offset if proof is provided as an account
    ///
    /// Note that the context state account is not required to be a signer and
    /// MUST be included within the same Transaction as the system program's
    /// `CreateAccount` instruction that creates the account being initialized.
    /// Otherwise another party can acquire ownership of the uninitialized
    /// account.
    ///
    VerifyGroupedCiphertext3HandlesValidity,

    /// Verify a batched grouped-ciphertext with 3 handles validity proof.
    ///
    /// A batched grouped-ciphertext validity proof certifies the validity of two grouped ElGamal
    /// ciphertext that are encrypted using the same set of ElGamal public keys. A batched
    /// grouped-ciphertext validity proof is shorter and more efficient than two individual
    /// grouped-ciphertext validity proofs.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   There are four ways to structure the accounts, depending on whether the
    ///   proof is provided as instruction data or in a separate account, and whether
    ///   a proof context is created.
    ///
    ///   1. **Proof in instruction data, no context state:**
    ///      - No accounts are required.
    ///
    ///   2. **Proof in instruction data, with context state:**
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    ///   3. **Proof in account, no context state:**
    ///      - `[]` Account to read the proof from.
    ///
    ///   4. **Proof in account, with context state:**
    ///      - `[]` Account to read the proof from.
    ///      - `[writable]` The proof context account to create.
    ///      - `[]` The proof context account owner.
    ///
    /// The instruction expects either:
    ///   i. `BatchedGroupedCiphertext3HandlesValidityProofData` if proof is provided as instruction data
    ///   ii. `u32` byte offset if proof is provided as an account
    ///
    /// Note that the context state account is not required to be a signer and
    /// MUST be included within the same Transaction as the system program's
    /// `CreateAccount` instruction that creates the account being initialized.
    /// Otherwise another party can acquire ownership of the uninitialized
    /// account.
    ///
    VerifyBatchedGroupedCiphertext3HandlesValidity,
}

/// Pubkeys associated with a context state account to be used as parameters to functions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextStateInfo<'a> {
    pub context_state_account: &'a Address,
    pub context_state_authority: &'a Address,
}

/// Create a `CloseContextState` instruction.
pub fn close_context_state(
    context_state_info: ContextStateInfo,
    destination_account: &Address,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*context_state_info.context_state_account, false),
        AccountMeta::new(*destination_account, false),
        AccountMeta::new_readonly(*context_state_info.context_state_authority, true),
    ];

    let data = vec![ToPrimitive::to_u8(&ProofInstruction::CloseContextState).unwrap()];

    Instruction {
        program_id: crate::id(),
        accounts,
        data,
    }
}

impl ProofInstruction {
    pub fn encode_verify_proof<T, U>(
        &self,
        context_state_info: Option<ContextStateInfo>,
        proof_data: &T,
    ) -> Instruction
    where
        T: Pod + ZkProofData<U>,
        U: Pod,
    {
        let accounts = if let Some(context_state_info) = context_state_info {
            vec![
                AccountMeta::new(*context_state_info.context_state_account, false),
                AccountMeta::new_readonly(*context_state_info.context_state_authority, false),
            ]
        } else {
            vec![]
        };

        let mut data = vec![ToPrimitive::to_u8(self).unwrap()];
        data.extend_from_slice(bytes_of(proof_data));

        Instruction {
            program_id: crate::id(),
            accounts,
            data,
        }
    }

    pub fn encode_verify_proof_from_account(
        &self,
        context_state_info: Option<ContextStateInfo>,
        proof_account: &Address,
        offset: u32,
    ) -> Instruction {
        let accounts = if let Some(context_state_info) = context_state_info {
            vec![
                AccountMeta::new_readonly(*proof_account, false),
                AccountMeta::new(*context_state_info.context_state_account, false),
                AccountMeta::new_readonly(*context_state_info.context_state_authority, false),
            ]
        } else {
            vec![AccountMeta::new_readonly(*proof_account, false)]
        };

        let mut data = vec![ToPrimitive::to_u8(self).unwrap()];
        data.extend_from_slice(&offset.to_le_bytes());

        Instruction {
            program_id: crate::id(),
            accounts,
            data,
        }
    }

    pub fn instruction_type(input: &[u8]) -> Option<Self> {
        input
            .first()
            .and_then(|instruction| FromPrimitive::from_u8(*instruction))
    }

    pub fn proof_data<T, U>(input: &[u8]) -> Option<&T>
    where
        T: Pod + ZkProofData<U>,
        U: Pod,
    {
        input
            .get(1..)
            .and_then(|data| bytemuck::try_from_bytes(data).ok())
    }
}
