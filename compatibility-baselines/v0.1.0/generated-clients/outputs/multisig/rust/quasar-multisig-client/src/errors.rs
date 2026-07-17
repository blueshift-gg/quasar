#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QuasarMultisigError {
    AccountAlreadyInitialized = 3001,
    AccountNotInitialized = 3000,
    AccountNotMutable = 3010,
    AccountNotRentExempt = 3008,
    AccountNotSigner = 3011,
    AccountOwnedByWrongProgram = 3009,
    AddressMismatch = 3012,
    CompactWriterFieldNotSet = 3014,
    ConstraintViolation = 3004,
    DynamicFieldTooLong = 3013,
    HasOneMismatch = 3005,
    InsufficientSpace = 3007,
    InvalidDiscriminator = 3006,
    InvalidPda = 3002,
    InvalidReturnData = 3019,
    InvalidSeeds = 3003,
    MissingReturnData = 3017,
    RemainingAccountDuplicate = 3016,
    RemainingAccountsOverflow = 3015,
    ReturnDataFromWrongProgram = 3018,
}

impl QuasarMultisigError {
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            3001 => Some(Self::AccountAlreadyInitialized),
            3000 => Some(Self::AccountNotInitialized),
            3010 => Some(Self::AccountNotMutable),
            3008 => Some(Self::AccountNotRentExempt),
            3011 => Some(Self::AccountNotSigner),
            3009 => Some(Self::AccountOwnedByWrongProgram),
            3012 => Some(Self::AddressMismatch),
            3014 => Some(Self::CompactWriterFieldNotSet),
            3004 => Some(Self::ConstraintViolation),
            3013 => Some(Self::DynamicFieldTooLong),
            3005 => Some(Self::HasOneMismatch),
            3007 => Some(Self::InsufficientSpace),
            3006 => Some(Self::InvalidDiscriminator),
            3002 => Some(Self::InvalidPda),
            3019 => Some(Self::InvalidReturnData),
            3003 => Some(Self::InvalidSeeds),
            3017 => Some(Self::MissingReturnData),
            3016 => Some(Self::RemainingAccountDuplicate),
            3015 => Some(Self::RemainingAccountsOverflow),
            3018 => Some(Self::ReturnDataFromWrongProgram),
            _ => None,
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::AccountAlreadyInitialized => "Account discriminator is already set (double-init attempt).",
            Self::AccountNotInitialized => "Account data is all zeros or has no discriminator.",
            Self::AccountNotMutable => "Account was not passed as writable.",
            Self::AccountNotRentExempt => "Account balance is below the rent-exemption minimum.",
            Self::AccountNotSigner => "Account was not passed as a signer.",
            Self::AccountOwnedByWrongProgram => "Account owner does not match the expected program.",
            Self::AddressMismatch => "Account address does not match the expected value.",
            Self::CompactWriterFieldNotSet => "A compact writer commit was attempted before setting every field.",
            Self::ConstraintViolation => "A `#[account(constraint = ...)]` expression evaluated to false.",
            Self::DynamicFieldTooLong => "A dynamic-length field exceeds its maximum byte length.",
            Self::HasOneMismatch => "`#[account(has_one = ...)]` field does not match.",
            Self::InsufficientSpace => "Account data is too small for the declared layout.",
            Self::InvalidDiscriminator => "Account discriminator does not match the expected value.",
            Self::InvalidPda => "PDA derivation does not match the expected address.",
            Self::InvalidReturnData => "Return data bytes do not match the expected fixed-size layout.",
            Self::InvalidSeeds => "Seeds provided for PDA verification are invalid.",
            Self::MissingReturnData => "The callee completed successfully but did not set return data.",
            Self::RemainingAccountDuplicate => "A duplicate remaining-account entry could not be resolved.",
            Self::RemainingAccountsOverflow => "More remaining accounts than can fit in the buffer.",
            Self::ReturnDataFromWrongProgram => "Return data was set by a different program than the one invoked.",
        }
    }
}

impl From<QuasarMultisigError> for u32 {
    fn from(error: QuasarMultisigError) -> Self {
        error as u32
    }
}
