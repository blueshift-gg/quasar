use quasar_lang::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ReturnPayload {
    pub amount: u64,
    pub flag: bool,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ReturnPayloadZc {
    pub amount: <u64 as InstructionArg>::Zc,
    pub flag: <bool as InstructionArg>::Zc,
}

impl ZcValidate for ReturnPayloadZc {
    fn validate_ref(value: &Self) -> Result<(), ZeroPodError> {
        ZcValidate::validate_ref(&value.amount)?;
        ZcValidate::validate_ref(&value.flag)
    }
}

// SAFETY: both fields are alignment-1 `ZcElem` wrappers and the C layout has
// no padding. Validation delegates to every field, including `PodBool`.
unsafe impl ZcElem for ReturnPayloadZc {}

impl InstructionArg for ReturnPayload {
    type Zc = ReturnPayloadZc;

    #[inline(always)]
    fn from_zc(zc: &Self::Zc) -> Self {
        Self {
            amount: <u64 as InstructionArg>::from_zc(&zc.amount),
            flag: <bool as InstructionArg>::from_zc(&zc.flag),
        }
    }

    #[inline(always)]
    fn to_zc(&self) -> Self::Zc {
        ReturnPayloadZc {
            amount: <u64 as InstructionArg>::to_zc(&self.amount),
            flag: <bool as InstructionArg>::to_zc(&self.flag),
        }
    }
}

pub const RETURN_U64_VALUE: u64 = 777;
pub const RETURN_PAYLOAD_VALUE: ReturnPayload = ReturnPayload {
    amount: 55,
    flag: true,
};

pub struct TestMiscProgram;

impl Id for TestMiscProgram {
    const ID: Address = crate::ID;
}

#[account(discriminator = 2, set_inner)]
#[seeds(b"simple", authority: Address)]
pub struct SimpleAccount {
    pub authority: Address,
    pub value: u64,
    pub bump: u8,
}

#[account(discriminator = [1, 2])]
pub struct MultiDiscAccount {
    pub data: u64,
}

#[account(discriminator = 5, set_inner)]
pub struct DynamicAccount {
    pub name: String<8>,
    pub tags: Vec<Address, 2>,
}

#[account(discriminator = 6)]
pub struct MixedAccount {
    pub authority: Address,
    pub value: u64,
    pub label: String<32>,
}

/// Fixed-layout scratch account for the two-dynamic-arg wire-format test (A1).
/// The handler packs the decoded `a`/`b` bytes here so the suite can read them
/// back and confirm the on-chain decode matched the client's compact layout.
#[account(discriminator = 22, set_inner)]
pub struct TwoDynArgsAccount {
    pub tag: u64,
    pub a_len: u8,
    pub a: u64,
    pub b_len: u8,
    pub b: u64,
}

#[account(discriminator = 7)]
pub struct SmallPrefixAccount {
    pub tag: String<100>,
    pub scores: Vec<u8, 10>,
}

#[account(discriminator = 8)]
pub struct DynStrAccount {
    pub authority: Address,
    pub label: String<255>,
}

#[account(discriminator = 9)]
pub struct DynBytesAccount {
    pub authority: Address,
    pub data: Vec<u8, 1024>,
}

/// Dynamic account using PodString/PodVec with runtime-sized storage.
#[account(discriminator = 10, set_inner)]
pub struct PodDynamicAccount {
    pub authority: Address,
    pub bump: u8,
    pub label: PodString<32>,
    pub members: PodVec<Address, 10>,
}

/// Fixed-capacity dynamic fields are inlined in the zero-copy struct.
#[account(discriminator = 11, fixed_capacity)]
pub struct FixedCapacityAccount {
    pub authority: Address,
    pub label: String<32>,
    pub scores: Vec<u8, 10>,
}

/// Same shape as SimpleAccount but with a different seed prefix for the
/// space-override test.
#[account(discriminator = 3, set_inner)]
#[seeds(b"spacetest", authority: Address)]
pub struct SpaceTestAccount {
    pub authority: Address,
    pub value: u64,
    pub bump: u8,
}

/// Same shape as SimpleAccount but with a different seed prefix for the
/// explicit-payer test.
#[account(discriminator = 4, set_inner)]
#[seeds(b"explicit", authority: Address)]
pub struct ExplicitPayerAccount {
    pub authority: Address,
    pub value: u64,
    pub bump: u8,
}

/// Account with no discriminator and size-only validation.
#[account(unsafe_no_disc, set_inner)]
#[seeds(b"nodisc", authority: Address)]
pub struct NoDiscAccount {
    pub authority: Address,
    pub value: u64,
}

// VaultV1 layout: discriminator + authority + value.
#[account(discriminator = 20)]
pub struct VaultV1 {
    pub authority: Address,
    pub value: u64,
}

// VaultV2 layout: discriminator + authority + value + fee.
#[account(discriminator = 21)]
pub struct VaultV2 {
    pub authority: Address,
    pub value: u64,
    pub fee: u64,
}

/// Migration interface that accepts either VaultV1 or VaultV2 accounts.
///
/// `Owners` returns the test-misc program ID. `AccountLoad` accepts
/// discriminator 20 or 21 with the matching minimum size.
#[repr(transparent)]
pub struct VaultInterface {
    __view: AccountView,
}

impl AsAccountView for VaultInterface {
    fn to_account_view(&self) -> &AccountView {
        &self.__view
    }
}

impl quasar_lang::traits::Owners for VaultInterface {
    fn check_owner(view: &AccountView) -> Result<(), ProgramError> {
        if quasar_lang::keys_eq(view.owner(), &crate::ID) {
            Ok(())
        } else {
            Err(ProgramError::IllegalOwner)
        }
    }
}

// SAFETY: `VaultInterface` is `#[repr(transparent)]` over `AccountView`.
unsafe impl quasar_lang::traits::StaticView for VaultInterface {}

impl quasar_lang::account_load::AccountLoad for VaultInterface {
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        let data = unsafe { view.borrow_unchecked() };
        if data.is_empty() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        match data[0] {
            20 => {
                // VaultV1: disc(1) + authority(32) + value(8) = 41
                if data.len() < 41 {
                    return Err(ProgramError::AccountDataTooSmall);
                }
                Ok(())
            }
            21 => {
                // VaultV2: disc(1) + authority(32) + value(8) + fee(8) = 49
                if data.len() < 49 {
                    return Err(ProgramError::AccountDataTooSmall);
                }
                Ok(())
            }
            _ => Err(ProgramError::InvalidAccountData),
        }
    }
}
