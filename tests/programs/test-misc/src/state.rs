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

#[account(discriminator = 1, set_inner)]
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

/// Pod-dynamic account test — uses PodString/PodVec with dynamic sizing.
#[account(discriminator = 10, set_inner)]
pub struct PodDynamicAccount {
    pub authority: Address,
    pub bump: u8,
    pub label: PodString<32>,
    pub members: PodVec<Address, 10>,
}

/// Fixed-capacity account — PodString/PodVec are inlined in the ZC struct
/// at full capacity. Zero-copy reads AND writes. No DynGuard needed.
#[account(discriminator = 11, fixed_capacity)]
pub struct FixedCapacityAccount {
    pub authority: Address,
    pub label: String<32>,
    pub scores: Vec<u8, 10>,
}

/// Same shape as SimpleAccount but with a different seed prefix — for
/// space-override test.
#[account(discriminator = 1, set_inner)]
#[seeds(b"spacetest", authority: Address)]
pub struct SpaceTestAccount {
    pub authority: Address,
    pub value: u64,
    pub bump: u8,
}

/// Same shape as SimpleAccount but with a different seed prefix — for
/// explicit-payer test.
#[account(discriminator = 1, set_inner)]
#[seeds(b"explicit", authority: Address)]
pub struct ExplicitPayerAccount {
    pub authority: Address,
    pub value: u64,
    pub bump: u8,
}

/// Account with no discriminator — size-only validation.
#[account(unsafe_no_disc, set_inner)]
#[seeds(b"nodisc", authority: Address)]
pub struct NoDiscAccount {
    pub authority: Address,
    pub value: u64,
}

// ---------------------------------------------------------------------------
// InterfaceAccount migration test: VaultV1 / VaultV2 / VaultInterface
// ---------------------------------------------------------------------------

// VaultV1: disc=20, { authority, value } = 1 + 32 + 8 = 41 bytes
#[account(discriminator = 20)]
pub struct VaultV1 {
    pub authority: Address,
    pub value: u64,
}

// VaultV2: disc=21, { authority, value, fee } = 1 + 32 + 8 + 8 = 49 bytes
#[account(discriminator = 21)]
pub struct VaultV2 {
    pub authority: Address,
    pub value: u64,
    pub fee: u64,
}

/// Interface type that accepts EITHER VaultV1 or VaultV2 accounts.
///
/// This is the migration pattern: one instruction handles both old (V1) and
/// new (V2) account layouts. `Owners` returns the test-misc program ID.
/// `AccountCheck` accepts disc=20 (V1) or disc=21 (V2) with sufficient data.
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
    fn owners() -> &'static [Address] {
        static OWNERS: [Address; 1] = [crate::ID];
        &OWNERS
    }
}

impl quasar_lang::traits::AccountCheck for VaultInterface {
    type Params = ();

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
