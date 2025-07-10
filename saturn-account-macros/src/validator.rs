use proc_macro2::TokenStream as TokenStream2;
use crate::model::FieldCfg;

pub(crate) mod syntax;
pub(crate) mod bump;
pub(crate) mod ctx;
pub(crate) use ctx::{ValidationCtx, Validator};
pub(crate) mod rules;

/// Convert a TokenStream into a canonical, whitespace-free `String` so logically equal
/// expressions written with different formatting still compare equal.  This is *not*
/// a perfect semantic normalisation, but avoids the most common false negatives such
/// as `&[b"seed"]` vs `&[
///     b"seed"
/// ]`.
fn canonical(ts: &TokenStream2) -> String {
    ts.to_string().split_whitespace().collect::<String>()
}

/// Perform cross-field / semantic validation after basic syntactic parsing has succeeded.
///
/// The current rules implemented (roughly following Anchor semantics):
/// 1. For every field annotated with `#[account(init, payer = X)]` the `payer` **must** refer to
///    another field in the same struct that is marked `signer`.
/// 2. No two fields may have the **same identifier** (should already be enforced by the Rust
///    compiler, but we add an explicit check to surface a clearer macro-level error if it ever
///    happens during parsing).
pub fn validate(fields: &[FieldCfg]) -> Result<(), syn::Error> {
    // ---------------------------------------------------------------------
    // Run pluggable validators (per-field and cross-field).
    // ---------------------------------------------------------------------
    {
        let ctx = ValidationCtx::new(fields);
        for v in rules::ALL_VALIDATORS {
            v.validate(&ctx)?;
        }
    }

    Ok(())
}
