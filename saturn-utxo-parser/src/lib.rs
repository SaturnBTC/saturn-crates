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
//! struct MyInstructionUtxos {
//!     // Exactly one UTXO worth 10,000 sats with no runes
//!     #[utxo(value = 10_000, runes = "none")]
//!     fee_utxo: UtxoInfo,
//!     
//!     // Optional rune deposit
//!     #[utxo(runes = "some")]
//!     rune_input: Option<UtxoInfo>,
//!     
//!     // All remaining UTXOs
//!     #[utxo(rest)]
//!     others: Vec<UtxoInfo>,
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
use saturn_bitcoin_transactions::utxo_info::UtxoInfo;

// Bring in the host-side test registry when compiling off-chain.
#[cfg(not(target_os = "solana"))]
mod test_registry;

#[cfg(not(target_os = "solana"))]
pub use test_registry::register_test_utxo_info;

// -----------------------------------------------------------------------------
// meta_to_info implementation
// -----------------------------------------------------------------------------
/// Convert a [`UtxoMeta`] into a full [`UtxoInfo`] while compiling for the
/// Solana BPF target we rely on the real on-chain syscall; when building for
/// the host we fall back to a lightweight stub that avoids the syscall.
#[cfg(target_os = "solana")]
pub fn meta_to_info(meta: &UtxoMeta) -> Result<UtxoInfo, ProgramError> {
    UtxoInfo::try_from(meta)
}

#[cfg(not(target_os = "solana"))]
pub fn meta_to_info(meta: &UtxoMeta) -> Result<UtxoInfo, ProgramError> {
    // If the test registered a rich UtxoInfo for this meta, use it.
    if let Some(info) = test_registry::lookup(meta) {
        return Ok(info);
    }

    // Fallback: minimal stub with just the metadata.  Value/rune information
    // will be default-initialised; predicates depending on those will fail.
    let mut info = UtxoInfo::default();
    info.meta = meta.clone();
    Ok(info)
}

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
pub trait TryFromUtxos<'utxos>: Sized {
    /// The accounts view that accompanies this parser.
    /// The internal lifetime of the `Accounts` implementation **does not need** to be the same
    /// as the borrow lifetime of the reference we receive in `try_utxos`.  We therefore leave
    /// it generic (`'any`) and only bind the *reference* itself to `'accs`.
    type Accs<'any>: saturn_account_parser::Accounts<'any>;

    /// Parse and validate a slice of [`UtxoMeta`].
    ///
    /// * `accounts`
    ///
    /// * `accounts` – already-validated account struct (borrowed for the
    ///   duration of the call only).
    /// * `utxos` – slice of UTXO metadata to parse.
    fn try_utxos<'accs, 'info2>(
        accounts: &'accs Self::Accs<'info2>,
        utxos: &'utxos [arch_program::utxo::UtxoMeta],
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
/// struct MyParser {
///     // ... fields with #[utxo(...)] attributes
/// }
/// ```
pub use saturn_utxo_parser_derive::UtxoParser;
