use crate::model::FieldKind;
use crate::validator::{ValidationCtx, Validator};
use quote::quote;
use std::collections::HashSet;
use syn::{spanned::Spanned, Expr, ExprPath, Ident};

/// Per-field syntactic validation wrapper that re-uses the existing
/// `syntax::validate_field` helper for each field in the context.
pub struct PerFieldSyntaxRule;

impl Validator for PerFieldSyntaxRule {
    fn validate(&self, ctx: &ValidationCtx) -> Result<(), syn::Error> {
        for f in ctx.fields {
            // Delegate to the original syntactic validation function.
            super::syntax::validate_field(f, f.ident.span())?;
        }
        Ok(())
    }
}

/// Wrapper around the bump-specific validation logic that was previously
/// invoked inline from `validator::validate`.
pub struct BumpFieldRule;

impl Validator for BumpFieldRule {
    fn validate(&self, ctx: &ValidationCtx) -> Result<(), syn::Error> {
        for f in ctx.fields {
            super::bump::validate_bump(f, f.ident.span())?;
        }
        Ok(())
    }
}

// -----------------------------------------------
// Duplicate identifier rule
// -----------------------------------------------
pub struct DuplicateIdentRule;

impl Validator for DuplicateIdentRule {
    fn validate(&self, ctx: &ValidationCtx) -> Result<(), syn::Error> {
        let mut seen: std::collections::HashSet<String> = HashSet::new();
        for f in ctx.fields {
            let ident_str = f.ident.to_string();
            if !seen.insert(ident_str.clone()) {
                return Err(syn::Error::new(
                    f.ident.span(),
                    format!("duplicate field identifier `{}` in struct", ident_str),
                ));
            }
        }
        Ok(())
    }
}

// -----------------------------------------------
// Payer relationship & ordering rule
// -----------------------------------------------
pub struct PayerRule;

impl Validator for PayerRule {
    fn validate(&self, ctx: &ValidationCtx) -> Result<(), syn::Error> {
        for (field_idx, f) in ctx
            .fields
            .iter()
            .enumerate()
            .filter(|(_, cfg)| cfg.is_init || cfg.is_init_if_needed || cfg.is_realloc)
        {
            let Some(payer_expr) = &f.payer else { continue };

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

            let Some((payer_idx, payer_cfg)) = ctx.by_ident.get(&ident.to_string()) else {
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
            // Must appear before the field that references it.
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
        Ok(())
    }
}

// -----------------------------------------------
// Flag consistency rule (writable / signer interplay)
// -----------------------------------------------
pub struct FlagConsistencyRule;

impl Validator for FlagConsistencyRule {
    fn validate(&self, ctx: &ValidationCtx) -> Result<(), syn::Error> {
        for f in ctx.fields {
            let needs_writable = f.is_init || f.is_init_if_needed || f.is_realloc;
            if needs_writable && !f.is_writable.unwrap_or(false) {
                return Err(syn::Error::new(
                    f.ident.span(),
                    "accounts with `init`, `init_if_needed` or `realloc` must be marked `mut`/`writable`",
                ));
            }

            if f.is_realloc && f.seeds.is_some() && f.is_signer.unwrap_or(false) {
                return Err(syn::Error::new(
                    f.ident.span(),
                    "`realloc` PDA account cannot be marked `signer` - the program signs via seeds",
                ));
            }

            if (f.is_init || f.is_init_if_needed)
                && f.seeds.is_none()
                && !f.is_signer.unwrap_or(false)
            {
                return Err(syn::Error::new(
                    f.ident.span(),
                    "accounts initialised without PDA seeds (`init` or `init_if_needed`) must be marked `signer`",
                ));
            }
        }
        Ok(())
    }
}

// -----------------------------------------------
// Zero copy & of=<Type> coupling rule
// -----------------------------------------------
pub struct ZeroCopyRule;

impl Validator for ZeroCopyRule {
    fn validate(&self, ctx: &ValidationCtx) -> Result<(), syn::Error> {
        for f in ctx.fields {
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
        Ok(())
    }
}

// -----------------------------------------------
// Duplicate PDA detection & bump ↔ PDA linkage rule
// -----------------------------------------------

pub struct PdaBumpRule;

impl Validator for PdaBumpRule {
    fn validate(&self, ctx: &ValidationCtx) -> Result<(), syn::Error> {
        // Collect PDA keys (seeds + program_id) for non-bump fields.
        let mut pda_keys: HashSet<(String, String)> = HashSet::new();
        for f in ctx
            .fields
            .iter()
            .filter(|cfg| cfg.seeds.is_some() && !matches!(cfg.kind, FieldKind::Bump))
        {
            let seeds_expr = f.seeds.as_ref().unwrap();
            let seeds_str = super::canonical(&quote!( #seeds_expr ));
            let prog_str = f
                .program_id
                .as_ref()
                .map(|e| super::canonical(&quote!( #e )))
                .unwrap_or_default();
            if !pda_keys.insert((seeds_str.clone(), prog_str.clone())) {
                return Err(syn::Error::new(
                    f.ident.span(),
                    "duplicate PDA derivation: another field uses the same `seeds` and `program_id`",
                ));
            }
        }

        // Track bump placeholders and ensure each matches a PDA.
        let mut bump_keys: HashSet<(String, String)> = HashSet::new();
        for f in ctx
            .fields
            .iter()
            .filter(|cfg| matches!(cfg.kind, FieldKind::Bump))
        {
            let seeds_expr = f.seeds.as_ref().expect("parser guarantees seeds");
            let program_id_expr = f.program_id.as_ref().expect("parser guarantees program_id");
            let seeds_str = super::canonical(&quote!( #seeds_expr ));
            let prog_str = super::canonical(&quote!( #program_id_expr ));
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
}

/// Registry of all active per-field validators.  Additional rules can be added
/// here without touching the orchestrator logic in `validator.rs`.
pub const ALL_VALIDATORS: &[&dyn Validator] = &[
    &PerFieldSyntaxRule,
    &BumpFieldRule,
    &DuplicateIdentRule,
    &PayerRule,
    &FlagConsistencyRule,
    &ZeroCopyRule,
    &PdaBumpRule,
];
