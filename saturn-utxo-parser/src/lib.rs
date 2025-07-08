//! # Saturn UTXO Parser
//!
//! A declarative UTXO parsing and validation library for Saturn programs.
//!
//! This crate provides a derive macro [`UtxoParser`] that automatically generates
//! parsing logic for Bitcoin UTXOs based on your struct definition and attributes.
//! It eliminates boilerplate code and reduces errors when handling UTXO inputs in
//! Saturn programs.
//!
//! ## Key Features
//!
//! - **Declarative**: Define your UTXO structure with attributes, get parsing for free
//! - **Type-safe**: Strong typing ensures UTXOs match your expectations
//! - **Flexible**: Support for optional UTXOs, arrays, and variable-length lists
//! - **Validation**: Built-in checks for values, rune presence, and specific rune amounts
//! - **Error handling**: Clear error codes for different failure scenarios
//!
//! ## Quick Start
//!
//! ```rust
//! use saturn_utxo_parser::{UtxoParser, TryFromUtxos};
//! use saturn_bitcoin_transactions::utxo_info::UtxoInfo;
//!
//! #[derive(UtxoParser)]
//! struct MyInstructionUtxos<'a> {
//!     // Exactly one UTXO worth 10,000 sats with no runes
//!     #[utxo(value = 10_000, runes = "none")]
//!     fee_utxo: &'a UtxoInfo,
//!     
//!     // Optional rune deposit
//!     #[utxo(runes = "some")]
//!     rune_input: Option<&'a UtxoInfo>,
//!     
//!     // All remaining UTXOs
//!     #[utxo(rest)]
//!     others: Vec<&'a UtxoInfo>,
//! }
//!
//! // Usage in your program
//! fn process_utxos(utxos: &[UtxoInfo]) -> Result<(), ProgramError> {
//!     let parsed = MyInstructionUtxos::try_utxos(utxos)?;
//!     // Use parsed.fee_utxo, parsed.rune_input, etc.
//!     Ok(())
//! }
//! ```
//!
//! ## Architecture
//!
//! The crate defines two main traits:
//!
//! - [`TryFromUtxos`]: The core trait for parsing UTXO slices (implemented by the derive macro)
//!
//! The [`UtxoParser`] derive macro generates implementations of [`TryFromUtxos`],
//! while [`TryFromUtxoMetas`] is provided via a blanket implementation that handles
//! the conversion from [`UtxoMeta`] to [`UtxoInfo`].
//!
//! [`UtxoMeta`]: arch_program::utxo::UtxoMeta

pub mod prelude {
    pub use crate::TryFromUtxos;
}

use arch_program::{program_error::ProgramError, utxo::UtxoMeta};

pub mod error;
pub use error::ErrorCode;
/// Core trait for parsing and validating UTXO information.
///
/// This trait converts a slice of [`UtxoInfo`] into a strongly-typed
/// structure that matches your parsing requirements. This is the main trait
/// implemented by the [`UtxoParser`] derive macro.
///
/// Unlike [`TryFromUtxoMetas`], this trait operates directly on the rich
/// [`UtxoInfo`] type, which provides access to full transaction details,
/// rune information, and other metadata needed for comprehensive validation.
///
/// ## Implementation
///
/// Typically you won't implement this trait manually. Instead, use the
/// [`UtxoParser`] derive macro which generates the implementation automatically
/// based on your struct definition and `#[utxo(...)]` attributes.
///
/// [`UtxoInfo`]: saturn_bitcoin_transactions::utxo_info::UtxoInfo
pub trait TryFromUtxos<'a>: Sized {
    /// Concrete `Accounts` type that accompanies this UTXO parser.  The derive
    /// macro emits this associated type automatically so implementations stay
    /// ergonomic while still allowing the compiler to know the exact struct
    /// that supplies account references used inside the parser.
    type Accs: saturn_account_parser::Accounts<'a>;

    /// Parse a slice of [`UtxoInfo`] into this strongly-typed structure,
    /// **using the already-validated `Accounts` struct of the instruction**.
    ///
    /// Implementations may inspect the accounts (e.g. to resolve an `anchor`
    /// target) while parsing UTXOs.
    ///
    /// # Parameters
    ///
    /// * `accounts` - The already-validated accounts struct of the instruction
    /// * `utxos` - Slice of UTXO information to parse and validate
    ///
    /// # Returns
    ///
    /// Returns `Ok(Self)` if all UTXOs are successfully matched and validated,
    /// or a [`ProgramError`] if parsing fails.
    ///
    /// # Errors
    ///
    /// This method returns specific error codes based on the failure type:
    ///
    /// - [`ErrorCode::MissingRequiredUtxo`] - A required UTXO couldn't be found
    /// - [`ErrorCode::UnexpectedExtraUtxos`] - UTXOs remain after all fields are satisfied
    /// - [`ErrorCode::InvalidUtxoValue`] - UTXO value doesn't match requirements
    /// - [`ErrorCode::InvalidRunesPresence`] - Rune presence doesn't match requirements
    /// - [`ErrorCode::InvalidRuneId`] - Specific rune ID not found
    /// - [`ErrorCode::InvalidRuneAmount`] - Rune amount doesn't match requirements
    ///
    /// [`UtxoInfo`]: saturn_bitcoin_transactions::utxo_info::UtxoInfo
    /// [`ProgramError`]: arch_program::program_error::ProgramError
    fn try_utxos(
        accounts: &'a Self::Accs,
        utxos: &'a [saturn_bitcoin_transactions::utxo_info::UtxoInfo],
    ) -> Result<Self, ProgramError>;
}

/// Re-export the derive macro so downstream crates need only one dependency.
///
/// This allows users to import both the trait and derive macro from the same crate:
///
/// ```rust
/// use saturn_utxo_parser::{UtxoParser, TryFromUtxos};
///
/// #[derive(UtxoParser)]
/// struct MyParser<'a> {
///     // ... fields with #[utxo(...)] attributes
/// }
/// ```
pub use saturn_utxo_parser_derive::UtxoParser;
