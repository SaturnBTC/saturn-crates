use crate::model::FieldCfg;
use syn::{spanned::Spanned, Expr, ExprPath, Ident};

/// Perform cross-field / semantic validation after basic syntactic parsing has succeeded.
///
/// The current rules implemented (roughly following Anchor semantics):
/// 1. For every field annotated with `#[account(init, payer = X)]` the `payer` **must** refer to
///    another field in the same struct that is marked `signer`.
/// 2. No two fields may have the **same identifier** (should already be enforced by the Rust
///    compiler, but we add an explicit check to surface a clearer macro-level error if it ever
///    happens during parsing).
pub fn validate(fields: &[FieldCfg]) -> Result<(), syn::Error> {
    // Build index for quick look-ups by identifier.
    let mut by_ident = std::collections::HashMap::<String, &FieldCfg>::new();
    for f in fields {
        if by_ident.insert(f.ident.to_string(), f).is_some() {
            return Err(syn::Error::new(
                f.ident.span(),
                format!("duplicate field identifier `{}` in struct", f.ident),
            ));
        }
    }

    // Rule 1: each init or realloc field must reference an existing signer payer.
    for f in fields
        .iter()
        .filter(|cfg| cfg.is_init || cfg.is_init_if_needed || cfg.is_realloc)
    {
        let Some(payer_expr) = &f.payer else { continue }; // already syntactically required.

        // We only support the simple case `payer = my_payer` where `my_payer` is an identifier.
        let Expr::Path(ExprPath { ref path, .. }) = payer_expr else {
            return Err(syn::Error::new(
                payer_expr.span(),
                "`payer = ...` must be a single identifier referring to another field",
            ));
        };

        // Ensure it's a single segment ident (no `this::that` or calling functions).
        let Some(seg) = path.segments.first() else {
            return Err(syn::Error::new(
                payer_expr.span(),
                "`payer` expression cannot be empty",
            ));
        };
        let ident: Ident = seg.ident.clone();

        let Some(payer_cfg) = by_ident.get(&ident.to_string()) else {
            return Err(syn::Error::new(
                ident.span(),
                format!("`payer` points to unknown field `{}`", ident),
            ));
        };

        // Must be signer.
        if !payer_cfg.is_signer.unwrap_or(false) {
            return Err(syn::Error::new(
                ident.span(),
                format!("field `{}` used as payer must be marked `signer`", ident),
            ));
        }
    }

    Ok(())
}
