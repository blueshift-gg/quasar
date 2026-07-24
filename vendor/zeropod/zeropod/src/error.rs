#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZeroPodError {
    BufferTooSmall,
    Overflow,
    InvalidBool,
    InvalidTag,
    InvalidDiscriminant,
    InvalidLength,
    InvalidUtf8,
}

impl core::fmt::Display for ZeroPodError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BufferTooSmall => write!(f, "buffer too small"),
            Self::Overflow => write!(f, "field value exceeds max capacity"),
            Self::InvalidBool => write!(f, "invalid bool: byte must be 0 or 1"),
            Self::InvalidTag => write!(f, "invalid option tag: byte must be 0 or 1"),
            Self::InvalidDiscriminant => write!(f, "invalid enum discriminant"),
            Self::InvalidLength => write!(f, "stored length exceeds capacity"),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 in string field"),
        }
    }
}

#[cfg(feature = "solana-program-error")]
impl From<ZeroPodError> for solana_program_error::ProgramError {
    fn from(e: ZeroPodError) -> Self {
        match e {
            ZeroPodError::BufferTooSmall => solana_program_error::ProgramError::AccountDataTooSmall,
            ZeroPodError::InvalidLength
            | ZeroPodError::InvalidBool
            | ZeroPodError::InvalidTag
            | ZeroPodError::InvalidDiscriminant
            | ZeroPodError::InvalidUtf8
            | ZeroPodError::Overflow => solana_program_error::ProgramError::InvalidAccountData,
        }
    }
}
