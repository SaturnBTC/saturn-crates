//! Error types returned by `saturn_account_parser`.
//!
//! The numeric representation of each variant is produced by the
//! `#[saturn_error(offset = 200)]` derive, yielding values in the range
//! **200–*** to avoid collisions with other `ProgramError::Custom` codes.
//!
//! Use the variants via `ProgramError::Custom(ErrorCode::XYZ.into())`.
use arch_program::program_error::ProgramError;
use saturn_error::saturn_error;

/// Parser-specific error codes.
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
    #[error("The account's public key did not match the expected address")]
    InvalidAccountKey,
}

pub type Result<T> = core::result::Result<T, ProgramError>;
