use arch_program::program_error::ProgramError;
use saturn_error::require;
use saturn_error::saturn_error;

#[saturn_error(offset = 900)]
#[derive(Debug)]
enum DemoError {
    Alpha,
    Beta,
    // Explicit discriminant should be preserved
    Gamma = 906,
    Delta,
}

#[test]
fn discriminant_assignment() {
    assert_eq!(DemoError::Alpha as u32, 900);
    assert_eq!(DemoError::Beta as u32, 901);
    assert_eq!(DemoError::Gamma as u32, 906);
    // Delta index is 3 → 900 + 3
    assert_eq!(DemoError::Delta as u32, 903);
}

#[test]
fn program_error_conversion() {
    let pe: ProgramError = DemoError::Beta.into();
    assert_eq!(pe, ProgramError::Custom(901));
}

#[test]
fn require_macro_behaviour() {
    fn validate(v: i32) -> saturn_error::Result<()> {
        require!(v > 0, DemoError::Alpha);
        Ok(())
    }

    // Success path
    assert!(validate(10).is_ok());

    // Failing path should convert the error
    let err = validate(0).unwrap_err();
    assert_eq!(err, ProgramError::Custom(900));
}
