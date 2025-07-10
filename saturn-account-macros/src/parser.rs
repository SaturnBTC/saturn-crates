use crate::model::{FieldCfg, FieldKind};
mod attr_ast;
mod defaults;
mod helpers;
use syn::{spanned::Spanned, Expr, Field};

/// Parse the `fields` of a struct annotated with `#[derive(Accounts)]` and
/// return a vector with one [`FieldCfg`] per field.
///
/// This function performs *syntactic* parsing and returns [`syn::Error`] for
/// syntax / attribute misuse.  Semantic, cross-field rules are handled later
/// by the `validator` module.
pub fn parse_fields(
    fields: &syn::punctuated::Punctuated<Field, syn::token::Comma>,
) -> Result<Vec<FieldCfg>, syn::Error> {
    let mut parsed_fields = Vec::<FieldCfg>::with_capacity(fields.len());

    for field in fields.iter() {
        // ===== Phase 1: Field pre-processing & initial cfg =====
        let ident = field
            .ident
            .clone()
            .ok_or_else(|| syn::Error::new(field.span(), "Unnamed fields are not supported"))?;

        // Initialise the configuration with defaults.
        let mut cfg = FieldCfg {
            ident,
            is_signer: None,
            is_writable: None,
            address: None,
            seeds: None,
            program_id: None,
            payer: None,
            is_shards: false,
            kind: FieldKind::Single,
            is_zero_copy: false,
            is_init: false,
            is_realloc: false,
            is_init_if_needed: false,
            base_ty: syn::parse_quote! { () },
            space: None,
            of_type: None,
            owner: None,
        };

        // Determine the underlying base type (strip reference if present)
        cfg.base_ty = match &field.ty {
            syn::Type::Reference(ref ref_ty) => (*ref_ty.elem).clone(),
            _ => field.ty.clone(),
        };

        // ===== Phase 2: Parse #[account(...)] attribute via RawAccountAttr =====
        let mut raw_attr_opt: Option<attr_ast::RawAccountAttr> = None;
        for attr in &field.attrs {
            if attr.path().is_ident("account") {
                let raw = attr_ast::RawAccountAttr::parse(attr)?;
                raw_attr_opt = Some(raw);
                break; // Only one #[account] per field is allowed.
            }
        }

        let has_account_attr = raw_attr_opt.is_some();

        // Apply parsed attributes to cfg (if any).
        if let Some(raw_attr) = &raw_attr_opt {
            raw_attr.apply_to_cfg(&mut cfg);
        }

        // We'll need the optional `len = ...` expression later for kind detection.
        let slice_len_expr: Option<Expr> = raw_attr_opt.as_ref().and_then(|r| r.len.clone());

        // If the field does not have an #[account] attribute we only skip it when it's clearly
        // a marker field such as `PhantomData`. Otherwise we treat it as an account field and
        // apply the usual validation so that unsupported bare types (e.g., `u64`) are caught.
        if !has_account_attr {
            let is_phantom = is_phantom_marker(&cfg.base_ty);

            if is_phantom {
                // Treat marker fields as Phantom kind so codegen can initialise them explicitly.
                cfg.kind = FieldKind::Phantom;
                // No further account-specific validation required.
                parsed_fields.push(cfg);
                continue;
            }
        }

        // ===== Phase 3: Determine collection kind (fixed slice, shards, …) =====
        if !matches!(cfg.kind, FieldKind::Phantom | FieldKind::Bump) {
            cfg.kind = helpers::detect_field_kind(
                &cfg.base_ty,
                cfg.is_shards,
                slice_len_expr.clone(),
                field.span(),
            )?;
        }

        // ===== Phase 4: Apply implicit defaults =====
        defaults::fill_defaults(&mut cfg);

        // ===== Phase 5: Per-field syntactic validation =====
        crate::validator::syntax::validate_field(&cfg, field.span())?;

        parsed_fields.push(cfg);
    }

    Ok(parsed_fields)
}

/// Returns true if `ty` is exactly `core::marker::PhantomData<_>` or `std::marker::PhantomData<_>`.
/// This mirrors Anchor's behaviour where only those two canonical paths are treated as zero-sized
/// marker fields and therefore ignored by the macro. Any other type that merely ends with
/// `PhantomData` will be treated as a regular field and validated accordingly.
fn is_phantom_marker(ty: &syn::Type) -> bool {
    use syn::Type;

    let segments_equal = |segments: &[String]| -> bool {
        segments == ["core", "marker", "PhantomData"]
            || segments == ["std", "marker", "PhantomData"]
    };

    match ty {
        Type::Path(type_path) => {
            let segs: Vec<String> = type_path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            segments_equal(&segs)
        }
        _ => false,
    }
}
