use crate::model::{FieldCfg, FieldKind};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{spanned::Spanned, Expr, ExprPath, Ident};

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
    // Build index for quick look-ups by identifier **and** keep field order.
    let mut by_ident = std::collections::HashMap::<String, (usize, &FieldCfg)>::new();
    for (idx, f) in fields.iter().enumerate() {
        if by_ident.insert(f.ident.to_string(), (idx, f)).is_some() {
            return Err(syn::Error::new(
                f.ident.span(),
                format!("duplicate field identifier `{}` in struct", f.ident),
            ));
        }
    }

    // ---------------------------------------------------------------------
    // 1. Validate payer relationships and ordering.
    // ---------------------------------------------------------------------
    for (field_idx, f) in fields
        .iter()
        .enumerate()
        .filter(|(_, cfg)| cfg.is_init || cfg.is_init_if_needed || cfg.is_realloc)
    {
        let Some(payer_expr) = &f.payer else { continue }; // already syntactically required.

        // Only support simple identifier paths.
        let Expr::Path(ExprPath { ref path, .. }) = payer_expr else {
            return Err(syn::Error::new(
                payer_expr.span(),
                "`payer = ...` must be a single identifier referring to another field",
            ));
        };
        let Some(seg) = path.segments.first() else {
            return Err(syn::Error::new(
                payer_expr.span(),
                "`payer` expression cannot be empty",
            ));
        };
        let ident: Ident = seg.ident.clone();

        let Some((payer_idx, payer_cfg)) = by_ident.get(&ident.to_string()) else {
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
        // Must be writable.
        if !payer_cfg.is_writable.unwrap_or(false) {
            return Err(syn::Error::new(
                ident.span(),
                format!(
                    "field `{}` used as payer must be marked `mut`/`writable`",
                    ident
                ),
            ));
        }
        // Must appear **before** the field that references it (Anchor ordering rule).
        if *payer_idx >= field_idx {
            return Err(syn::Error::new(
                ident.span(),
                format!(
                    "field `{}` (payer) must be declared before the account that it pays for",
                    ident
                ),
            ));
        }
    }

    // ---------------------------------------------------------------------
    // 2. Enforce writability / signer interplay on specific flags.
    // ---------------------------------------------------------------------
    for f in fields {
        let needs_writable = f.is_init || f.is_init_if_needed || f.is_realloc;
        if needs_writable && !f.is_writable.unwrap_or(false) {
            return Err(syn::Error::new(
                f.ident.span(),
                "accounts with `init`, `init_if_needed` or `realloc` must be marked `mut`/`writable`",
            ));
        }

        // For PDA accounts (have `seeds`), signing is done via program-derived address; forbid explicit `signer`. For plain accounts allow it.
        if f.is_realloc && f.seeds.is_some() && f.is_signer.unwrap_or(false) {
            return Err(syn::Error::new(
                f.ident.span(),
                "`realloc` PDA account cannot be marked `signer` - the program signs via seeds",
            ));
        }

        // For non-PDA account creation (`init` / `init_if_needed` *without* `seeds`)
        // the target account must sign the transaction because the program cannot
        // sign on its behalf via `invoke_signed`.
        if (f.is_init || f.is_init_if_needed) && f.seeds.is_none() && !f.is_signer.unwrap_or(false)
        {
            return Err(syn::Error::new(
                f.ident.span(),
                "accounts initialised without PDA seeds (`init` or `init_if_needed`) must be marked `signer`",
            ));
        }
    }

    // ---------------------------------------------------------------------
    // 3. `zero_copy` must specify `of = <Type>`.
    //    Conversely, specifying `of = <Type>` on a *single* account without `zero_copy`
    //    is meaningless and therefore rejected.
    // ---------------------------------------------------------------------
    for f in fields {
        if f.is_zero_copy && f.of_type.is_none() {
            return Err(syn::Error::new(
                f.ident.span(),
                "`zero_copy` attribute requires `of = <Type>` to specify the account data type",
            ));
        }
        if matches!(f.kind, FieldKind::Single) && !f.is_zero_copy && f.of_type.is_some() {
            return Err(syn::Error::new(
                f.ident.span(),
                "`of = <Type>` on single account fields requires `zero_copy`",
            ));
        }
    }

    // ---------------------------------------------------------------------
    // 4. Duplicate PDA detection + bump ↔ PDA linkage.
    // ---------------------------------------------------------------------

    // Collect PDA keys (seeds + program_id) for non-bump fields.
    let mut pda_keys: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    for f in fields
        .iter()
        .filter(|cfg| cfg.seeds.is_some() && !matches!(cfg.kind, FieldKind::Bump))
    {
        let seeds_expr = f.seeds.as_ref().unwrap();
        let seeds_str = canonical(&quote!( #seeds_expr ));
        let prog_str = f
            .program_id
            .as_ref()
            .map(|e| canonical(&quote!( #e )))
            .unwrap_or_default();
        if !pda_keys.insert((seeds_str.clone(), prog_str.clone())) {
            return Err(syn::Error::new(
                f.ident.span(),
                "duplicate PDA derivation: another field uses the same `seeds` and `program_id`",
            ));
        }
    }

    // Track bump placeholders and ensure each matches a PDA.
    let mut bump_keys: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    for f in fields
        .iter()
        .filter(|cfg| matches!(cfg.kind, FieldKind::Bump))
    {
        let seeds_expr = f.seeds.as_ref().expect("parser guarantees seeds");
        let program_id_expr = f.program_id.as_ref().expect("parser guarantees program_id");
        let seeds_str = canonical(&quote!( #seeds_expr ));
        let prog_str = canonical(&quote!( #program_id_expr ));
        let key = (seeds_str.clone(), prog_str.clone());

        if !bump_keys.insert(key.clone()) {
            return Err(syn::Error::new(
                f.ident.span(),
                "duplicate `bump` placeholder for the same seeds/program_id",
            ));
        }
        if !pda_keys.contains(&key) {
            return Err(syn::Error::new(
                f.ident.span(),
                "`bump` field has no corresponding PDA account with matching seeds and program_id",
            ));
        }
    }

    Ok(())
}
