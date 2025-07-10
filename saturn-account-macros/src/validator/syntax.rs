use crate::model::{FieldCfg, FieldKind};
use syn::spanned::Spanned;

/// Perform *per-field* syntactic checks that do not depend on other fields.
/// These mirror the behaviour previously embedded directly in `parser.rs` but
/// are now isolated so they can be unit-tested independently and reused by
/// other tooling.
pub(crate) fn validate_field(cfg: &FieldCfg, span: proc_macro2::Span) -> Result<(), syn::Error> {
    // -----------------------------------------------------------------
    // Seeds / address exclusivity & dependency rules
    // -----------------------------------------------------------------
    if cfg.seeds.is_some() && cfg.address.is_some() {
        return Err(syn::Error::new(
            span,
            "`address` cannot be combined with `seeds`; the PDA address is derived from the provided seeds – remove the redundant `address` attribute.",
        ));
    }

    if cfg.seeds.is_some() && cfg.program_id.is_none() {
        return Err(syn::Error::new(
            span,
            "`program_id = <id>` attribute is required whenever `seeds = ...` is specified",
        ));
    }

    if cfg.program_id.is_some() && cfg.seeds.is_none() && !(cfg.is_init || cfg.is_realloc) {
        return Err(syn::Error::new(
            span,
            "`program_id` attribute requires `seeds = ...` to be specified unless the field is also marked `init`",
        ));
    }

    // -----------------------------------------------------------------
    // Additional rules for init / init_if_needed
    // -----------------------------------------------------------------
    if cfg.is_init || cfg.is_init_if_needed {
        if cfg.address.is_some() {
            return Err(syn::Error::new(
                span,
                "`address` cannot be combined with `init` or `init_if_needed`; the account address is determined by the creation logic",
            ));
        }
        if cfg.payer.is_none() {
            return Err(syn::Error::new(
                span,
                "`init` field requires `payer = <account>` in #[account] attribute",
            ));
        }
        if cfg.program_id.is_none() {
            return Err(syn::Error::new(
                span,
                "`init` field requires `program_id = <id>` in #[account] attribute to set the owner",
            ));
        }
    }

    // -----------------------------------------------------------------
    // Rules specific to realloc
    // -----------------------------------------------------------------
    if cfg.is_realloc {
        if cfg.space.is_none() {
            return Err(syn::Error::new(
                span,
                "`realloc` field requires `space = <expr>` in #[account] attribute to set the new data length",
            ));
        }

        if cfg.is_init || cfg.is_init_if_needed {
            return Err(syn::Error::new(
                span,
                "`realloc` cannot be combined with `init` or `init_if_needed`",
            ));
        }
    }

    // Conversely, ensure `init` / `init_if_needed` are not combined with `realloc`.
    if (cfg.is_init || cfg.is_init_if_needed) && cfg.is_realloc {
        return Err(syn::Error::new(
            span,
            "`init` / `init_if_needed` cannot be combined with `realloc`",
        ));
    }

    // -----------------------------------------------------------------
    // Shard vector signing rule
    // -----------------------------------------------------------------
    if cfg.is_shards && cfg.seeds.is_some() && cfg.is_signer.unwrap_or(false) {
        return Err(syn::Error::new(
            span,
            "`signer` attribute cannot be used on shard vectors that are PDAs (`seeds = ...`)",
        ));
    }

    // -----------------------------------------------------------------
    // AccountLoader zero-copy requirement
    // -----------------------------------------------------------------
    // For *single* account fields whose base type is `AccountLoader<...>` the user must
    // explicitly opt-in via the `zero_copy` flag so that the macro can generate the
    // correct initialisation & loading code.  Without the flag the compiler error
    // surfaces as unrelated trait bounds which is confusing.  Emit a dedicated
    // diagnostic instead.
    if matches!(cfg.kind, FieldKind::Single) && !cfg.is_zero_copy {
        use syn::Type;

        let is_account_loader = match &cfg.base_ty {
            // Direct `AccountLoader<'info, T>`
            Type::Path(tp) => tp
                .path
                .segments
                .last()
                .map_or(false, |seg| seg.ident == "AccountLoader"),
            // Reference to `AccountLoader<'info, T>`
            Type::Reference(ref_ty) => match &*ref_ty.elem {
                Type::Path(tp) => tp
                    .path
                    .segments
                    .last()
                    .map_or(false, |seg| seg.ident == "AccountLoader"),
                _ => false,
            },
            _ => false,
        };

        if is_account_loader {
            return Err(syn::Error::new(
                span,
                "fields of type `AccountLoader<_>` require the `zero_copy` flag in #[account] attribute",
            ));
        }
    }

    // All good.
    Ok(())
} 