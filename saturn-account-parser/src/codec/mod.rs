//! Serialization codecs used by Saturn account wrappers.
//!
//! The available codecs are:
//! * [`borsh`] – copy-based Borsh (de)serialization.
//! * [`zero_copy`] – zero-copy reinterpretation into Plain-Old-Data structs for
//!   maximum performance on-chain.

pub mod borsh;
pub mod zero_copy;

pub use borsh::{Account, BorshAccount, BorshCodec};
pub use zero_copy::{AccountLoader, ZeroCopyAccount, ZeroCopyCodec};
