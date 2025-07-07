use arch_program::program_error::ProgramError;
use saturn_error::saturn_error;

#[saturn_error(offset = 200)]
pub enum ErrorCode {
    #[error("The provided account's `is_signer` flag does not match the expected value")]
    IncorrectIsSignerFlag,
    #[error("The provided account's `is_writable` flag does not match the expected value")]
    IncorrectIsWritableFlag,
    #[error("Account required for the instruction was not found in the account list")]
    AccountNotFound,
    #[error("The provided PDA does not match the expected address derived from the given seeds")]
    InvalidPda,
}

pub type Result<T> = core::result::Result<T, ProgramError>;
