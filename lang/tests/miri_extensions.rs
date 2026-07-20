//! Adversarial tests for safe downstream extension points.
//!
//! Unlike the low-level aliasing suite in `miri.rs`, these tests are valid
//! under both Stacked Borrows and Tree Borrows. Most importantly, they execute
//! an actual `#[instruction]`-generated fixed-argument decoder over untrusted
//! bytes rather than mirroring its pointer cast in test code.
#![allow(unexpected_cfgs)]

use {quasar_lang::prelude::*, solana_program_error::ProgramError};

solana_address::declare_id!("11111111111111111111111111111112");

#[repr(transparent)]
#[derive(Copy, Clone)]
struct SmallZc(u8);

impl ZcValidate for SmallZc {
    fn validate_ref(value: &Self) -> Result<(), ZeroPodError> {
        if value.0 <= 7 {
            Ok(())
        } else {
            Err(ZeroPodError::InvalidDiscriminant)
        }
    }
}

// SAFETY: `SmallZc` is transparent over `u8`, so it has alignment 1, no
// padding, and every bit pattern is a valid Rust value. `ZcValidate` enforces
// its semantic range.
unsafe impl ZcElem for SmallZc {}

#[derive(Debug, PartialEq, Eq)]
struct Small(u8);

impl InstructionArg for Small {
    type Zc = SmallZc;

    fn from_zc(zc: &Self::Zc) -> Self {
        Self(zc.0)
    }

    fn to_zc(&self) -> Self::Zc {
        SmallZc(self.0)
    }
}

mod generated_decoder {
    use super::*;

    #[derive(Accounts)]
    pub struct NoAccounts {}

    #[instruction(discriminator = 0)]
    pub fn decode(_ctx: Ctx<NoAccounts>, flag: bool, value: Small) -> Result<(), ProgramError> {
        assert!(flag);
        assert_eq!(value, Small(7));
        Ok(())
    }

    pub fn run(data: &[u8]) -> Result<(), ProgramError> {
        let program_id = [1u8; 32];
        let mut accounts = [];
        let boundary = data.as_ptr();
        // SAFETY: the context has zero declared and remaining accounts, so the
        // shared boundary pointer is never dereferenced as an account entry.
        let context = unsafe {
            Context::from_raw_parts(
                &program_id,
                &mut accounts,
                data,
                boundary.cast_mut(),
                boundary,
            )
        };
        decode(context)
    }
}

#[test]
fn generated_decoder_accepts_valid_extension_values() {
    assert_eq!(generated_decoder::run(&[1, 7]), Ok(()));
}

#[test]
fn generated_decoder_rejects_invalid_bool_before_native_conversion() {
    assert_eq!(
        generated_decoder::run(&[2, 7]),
        Err(ProgramError::InvalidInstructionData)
    );
}

#[test]
fn generated_decoder_runs_custom_zc_validation() {
    assert_eq!(
        generated_decoder::run(&[1, 8]),
        Err(ProgramError::InvalidInstructionData)
    );
}
