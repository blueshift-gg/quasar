use quasar_lang::prelude::*;

#[account(discriminator = 1)]
pub struct ErrorTestAccount {
    pub authority: Address,
    pub value: u64,
}

pub struct TestErrorsProgram;

impl Id for TestErrorsProgram {
    const ID: Address = crate::ID;
}

/// 12-byte return payload (u64 + u32): deliberately NOT 8 bytes, so a caller
/// that decodes it as `u64` exercises the `InvalidReturnData` length check.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ReturnPair {
    pub a: u64,
    pub b: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ReturnPairZc {
    pub a: <u64 as InstructionArg>::Zc,
    pub b: <u32 as InstructionArg>::Zc,
}

impl InstructionArg for ReturnPair {
    type Zc = ReturnPairZc;

    #[inline(always)]
    fn from_zc(zc: &Self::Zc) -> Self {
        Self {
            a: <u64 as InstructionArg>::from_zc(&zc.a),
            b: <u32 as InstructionArg>::from_zc(&zc.b),
        }
    }

    #[inline(always)]
    fn to_zc(&self) -> Self::Zc {
        ReturnPairZc {
            a: <u64 as InstructionArg>::to_zc(&self.a),
            b: <u32 as InstructionArg>::to_zc(&self.b),
        }
    }
}

pub const RETURN_PAIR_VALUE: ReturnPair = ReturnPair { a: 99, b: 7 };
