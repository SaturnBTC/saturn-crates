//! Saturn core error handling.
//!
//! This crate provides two things:
//! 1. A small built-in `SaturnErrorCode` enum (range 0–999) for framework-level
//!    failures.
//! 2. Utility macros (`require!`) and re-exports so each program can declare its
//!    own error enum with the `#[saturn_error(offset = ...)]` attribute macro.
//!
//! The attribute macro lives in the companion crate `saturn-error-derive`, but
//! we re-export it here for ergonomic `use saturn_error::saturn_error;`.
// NOTE: Currently compiled with the standard library. Remove this attribute
// or add proper `alloc`/`core` support if full `no_std` capability is needed.

use arch_program::program_error::ProgramError;

/// Errors produced by the Saturn framework itself.
///
/// These are assigned the numeric range `0..=999`.
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
#[repr(u32)]
pub enum SaturnErrorCode {
    /// Invalid account data encountered.
    #[error("Invalid account data")]
    InvalidAccountData = 0,
    /// Integer overflow / underflow.
    #[error("Math overflow or underflow")]
    MathOverflow = 1,
    /// The provided account did not match the expected program id.
    #[error("Invalid program id")]
    InvalidProgramId = 2,
    /// Generic error placeholder. Prefer adding concrete variants.
    #[error("Generic Saturn framework error")]
    GenericError = 999,
}

impl From<SaturnErrorCode> for ProgramError {
    #[inline]
    fn from(e: SaturnErrorCode) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl From<SaturnErrorCode> for u32 {
    #[inline]
    fn from(e: SaturnErrorCode) -> Self {
        e as u32
    }
}

/// Convenience alias for returning `ProgramError`s.
pub type Result<T> = core::result::Result<T, ProgramError>;

/// Mirror of Anchor's `require!` macro. Evaluates the provided expression and
/// returns the supplied error (converted into `ProgramError`) if the condition
/// is `false`.
#[macro_export]
macro_rules! require {
    ($cond:expr, $err:expr $(,)?) => {
        if !$cond {
            return core::result::Result::Err($err.into());
        }
    };
}

// -------------------------------------------------------------------------
// Re-exports
// -------------------------------------------------------------------------

pub use saturn_error_derive::saturn_error;

/// Internal re-exports that the procedural macros rely on.
///
/// Users should **not** depend on anything inside this module— its layout is
/// considered private and may change without notice.  The only guarantee is
/// that the macros we ship (`#[saturn_error]`) will compile against it.
#[doc(hidden)]
pub mod __private {
    pub use num_derive;
    pub use num_traits;
    pub use thiserror;
}

/// Creates a [`ProgramError`] from the given error value **and** emits a log
/// message containing file & line information.
///
/// # Example
///
/// ```ignore
/// #[saturn_error(offset = 6000)]
/// pub enum MyError {
///     #[error("Invalid foo")]
///     InvalidFoo,
/// }
///
/// fn do_something() -> saturn_error::Result<()> {
///     // ...
///     Err(error!(MyError::InvalidFoo))
/// }
/// ```
///
/// Internally the macro performs two tasks:
/// 1. Emits an `arch_program::msg!` log along the lines of
///    `"SaturnError thrown in <file>:<line>. Error Code: <N>."` so that the
///    Solana transaction logs include human-readable diagnostics.
/// 2. Converts the supplied value into [`ProgramError`] via `Into` and returns
///    it **as an expression** (it does *not* wrap it in `Err(..)` — that is the
///    responsibility of the call-site just like in Anchor).
///
/// Any type implementing `Into<ProgramError>` is accepted. This includes both
/// the built-in [`SaturnErrorCode`] as well as user-defined enums annotated with
/// `#[saturn_error]`.
#[macro_export]
macro_rules! error {
    ($err:expr $(,)?) => {{
        // Log message for easier debugging in transaction logs. Use "{:?}" formatting.
        let code: arch_program::program_error::ProgramError = ($err).into();
        // Extract numeric code for custom errors to aid clients. We purposefully
        // avoid pattern matching on all variants to keep the macro simple and
        // forward-compatible.
        let numeric: u32 = match code {
            arch_program::program_error::ProgramError::Custom(x) => x,
            // For built-in `ProgramError` variants we just emit `0`.
            _ => 0,
        };
        arch_program::msg!(
            concat!(
                "SaturnError thrown in ",
                file!(),
                ":",
                line!(),
                ". Error Code: {}. Error: {:?}"
            ),
            numeric,
            &$err
        );
        code
    }};
}

// -------------------------------------------------------------------------
// Comparison helper macros
// -------------------------------------------------------------------------

/// Require that two expressions are equal (`==`).
///
/// Usage: `require_eq!(a, b, MyError::NotEqual);`
#[macro_export]
macro_rules! require_eq {
    ($left:expr, $right:expr, $err:expr $(,)?) => {
        $crate::require!($left == $right, $err);
    };
}

/// Require that two expressions are **not** equal (`!=`).
#[macro_export]
macro_rules! require_neq {
    ($left:expr, $right:expr, $err:expr $(,)?) => {
        $crate::require!($left != $right, $err);
    };
}

/// Require that `$left` is strictly greater than `$right` (`>`).
#[macro_export]
macro_rules! require_gt {
    ($left:expr, $right:expr, $err:expr $(,)?) => {
        $crate::require!($left > $right, $err);
    };
}

/// Require that `$left` is greater than **or equal** to `$right` (`>=`).
#[macro_export]
macro_rules! require_gte {
    ($left:expr, $right:expr, $err:expr $(,)?) => {
        $crate::require!($left >= $right, $err);
    };
}

/// Require that `$left` is strictly less than `$right` (`<`).
#[macro_export]
macro_rules! require_lt {
    ($left:expr, $right:expr, $err:expr $(,)?) => {
        $crate::require!($left < $right, $err);
    };
}

/// Require that `$left` is less than **or equal** to `$right` (`<=`).
#[macro_export]
macro_rules! require_lte {
    ($left:expr, $right:expr, $err:expr $(,)?) => {
        $crate::require!($left <= $right, $err);
    };
}

/// Require that two [`arch_program::pubkey::Pubkey`] values are equal.
/// This uses `PartialEq` (`==`) internally but provides clearer intent at call-sites.
#[macro_export]
macro_rules! require_keys_eq {
    ($left:expr, $right:expr, $err:expr $(,)?) => {
        $crate::require!($left == $right, $err);
    };
}
