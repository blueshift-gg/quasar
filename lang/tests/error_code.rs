use {quasar_lang::prelude::*, solana_program_error::ProgramError};

#[error_code]
enum AutomaticError {
    First,
    Second,
}

#[error_code]
enum ExplicitError {
    First = 7_000,
    Second,
}

#[test]
fn runtime_conversion_matches_resolved_error_codes() {
    assert_eq!(
        ProgramError::from(AutomaticError::First),
        ProgramError::Custom(6_000)
    );
    assert_eq!(
        ProgramError::from(AutomaticError::Second),
        ProgramError::Custom(6_001)
    );
    assert!(matches!(
        AutomaticError::try_from(6_001),
        Ok(AutomaticError::Second)
    ));

    assert_eq!(
        ProgramError::from(ExplicitError::First),
        ProgramError::Custom(7_000)
    );
    assert_eq!(
        ProgramError::from(ExplicitError::Second),
        ProgramError::Custom(7_001)
    );
    assert!(matches!(
        ExplicitError::try_from(7_001),
        Ok(ExplicitError::Second)
    ));
}
