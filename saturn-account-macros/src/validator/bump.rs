use crate::model::{FieldCfg, FieldKind};
use syn::{spanned::Spanned, Expr, Lit, Type};

/// Perform *per-field* validation specific to `#[account(bump)]` placeholder
/// fields.  Must be called **after** basic syntactic validation so that
/// `seeds` / `program_id` presence is already syntactically enforced.
pub(crate) fn validate_bump(cfg: &FieldCfg, span: proc_macro2::Span) -> Result<(), syn::Error> {
    if !matches!(cfg.kind, FieldKind::Bump) {
        return Ok(()); // Ignore non-bump fields.
    }

    // ------------------------------------------------------------------
    // 1. Type must be `u8`, `[u8; 1]`, or reference to `[u8; 1]`.
    // ------------------------------------------------------------------
    let is_u8_primitive = matches!(&cfg.base_ty, Type::Path(tp)
        if tp.path.segments.last().map_or(false, |seg| seg.ident == "u8"));

    // `[u8; 1]` literal array type
    let is_u8_array1 = matches!(&cfg.base_ty, Type::Array(arr)
        if matches!(&*arr.elem, Type::Path(tp)
            if tp.path.segments.last().map_or(false, |seg| seg.ident == "u8"))
            && is_len_one(&arr.len));

    // Reference to `[u8; 1]` (any lifetime)
    let is_ref_u8_array1 = matches!(&cfg.base_ty, Type::Reference(ref_ty)
        if matches!(&*ref_ty.elem, Type::Array(arr)
            if matches!(&*arr.elem, Type::Path(tp)
                if tp.path.segments.last().map_or(false, |seg| seg.ident == "u8"))
                && is_len_one(&arr.len)));

    if !(is_u8_primitive || is_u8_array1 || is_ref_u8_array1) {
        return Err(syn::Error::new(
            span,
            "`bump` field must have type `u8`, `[u8; 1]`, or reference to `[u8; 1]`",
        ));
    }

    // ------------------------------------------------------------------
    // 2. Ensure seeds & program_id exist – parser guarantees program_id via
    //    defaulting, but we double-check for safety.
    // ------------------------------------------------------------------
    if cfg.seeds.is_none() {
        return Err(syn::Error::new(
            span,
            "`bump` field requires `seeds = ...` attribute",
        ));
    }
    if cfg.program_id.is_none() {
        return Err(syn::Error::new(
            span,
            "`bump` field requires `program_id = <id>` attribute",
        ));
    }

    // ------------------------------------------------------------------
    // 3. Disallow mixing other account-specific flags.
    // ------------------------------------------------------------------
    if cfg.is_signer.is_some()
        || cfg.is_writable.is_some()
        || cfg.is_init
        || cfg.is_init_if_needed
        || cfg.is_realloc
        || cfg.is_zero_copy
        || cfg.is_shards
    {
        return Err(syn::Error::new(
            span,
            "`bump` field cannot combine other account-specific flags",
        ));
    }

    Ok(())
}

/// Helper that recognises the literal length `1` in an array type expression.
fn is_len_one(expr: &Expr) -> bool {
    matches!(expr, Expr::Lit(lit) if matches!(&lit.lit, Lit::Int(int_lit) if int_lit.base10_parse::<u64>().map_or(false, |v| v == 1)))
}
