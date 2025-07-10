use syn::{spanned::Spanned, Expr, Type, Attribute};
use crate::model::{FieldCfg, FieldKind};

/// Internal representation of the flags/values found inside an `#[account(..)]` attribute.
///
/// This is *pure syntax* — it does **not** perform any cross-field or semantic
/// validation.  That remains the responsibility of the higher-level parser and
/// the `validator` module.  Its sole job is to turn the nested meta-list into a
/// strongly-typed Rust struct.
#[derive(Debug, Default)]
pub struct RawAccountAttr {
    pub signer: bool,
    pub writable: bool,
    pub address: Option<Expr>,
    pub len: Option<Expr>,
    pub seeds: Option<Expr>,
    pub program_id: Option<Expr>,
    pub payer: Option<Expr>,
    pub owner: Option<Expr>,
    pub is_shards: bool,
    pub of_type: Option<Type>,
    pub zero_copy: bool,
    pub init: bool,
    pub init_if_needed: bool,
    pub realloc: bool,
    pub space: Option<Expr>,
    pub bump: bool,
}

impl RawAccountAttr {
    /// Parse a single `#[account(..)]` attribute into a [`RawAccountAttr`].
    pub fn parse(attr: &Attribute) -> Result<Self, syn::Error> {
        // Sanity-check the attribute path first so callers don’t have to.
        if !attr.path().is_ident("account") {
            return Err(syn::Error::new(attr.span(), "expected #[account(..)] attribute"));
        }

        let mut raw = RawAccountAttr::default();

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("signer") {
                if raw.signer {
                    return Err(meta.error("duplicate `signer` flag"));
                }
                raw.signer = true;
            } else if meta.path.is_ident("mut") || meta.path.is_ident("writable") {
                if raw.writable {
                    return Err(meta.error("duplicate `writable` flag"));
                }
                raw.writable = true;
            } else if meta.path.is_ident("address") {
                if raw.address.is_some() {
                    return Err(meta.error("duplicate `address` attribute"));
                }
                let expr: Expr = meta.value()?.parse()?;
                raw.address = Some(expr);
            } else if meta.path.is_ident("len") {
                if raw.len.is_some() {
                    return Err(meta.error("duplicate `len` attribute"));
                }
                let expr: Expr = meta.value()?.parse()?;
                raw.len = Some(expr);
            } else if meta.path.is_ident("seeds") {
                if raw.seeds.is_some() {
                    return Err(meta.error("duplicate `seeds` attribute"));
                }
                let expr: Expr = meta.value()?.parse()?;
                raw.seeds = Some(expr);
            } else if meta.path.is_ident("program_id") {
                if raw.program_id.is_some() {
                    return Err(meta.error("duplicate `program_id` attribute"));
                }
                let expr: Expr = meta.value()?.parse()?;
                raw.program_id = Some(expr);
            } else if meta.path.is_ident("payer") {
                if raw.payer.is_some() {
                    return Err(meta.error("duplicate `payer` attribute"));
                }
                let expr: Expr = meta.value()?.parse()?;
                raw.payer = Some(expr);
            } else if meta.path.is_ident("owner") {
                if raw.owner.is_some() {
                    return Err(meta.error("duplicate `owner` attribute"));
                }
                let expr: Expr = meta.value()?.parse()?;
                raw.owner = Some(expr);
            } else if meta.path.is_ident("shards") {
                if raw.is_shards {
                    return Err(meta.error("duplicate `shards` flag"));
                }
                raw.is_shards = true;
            } else if meta.path.is_ident("of") {
                if raw.of_type.is_some() {
                    return Err(meta.error("duplicate `of` attribute"));
                }
                let ty: Type = meta.value()?.parse()?;
                raw.of_type = Some(ty);
            } else if meta.path.is_ident("zero_copy") {
                if raw.zero_copy {
                    return Err(meta.error("duplicate `zero_copy` flag"));
                }
                raw.zero_copy = true;
            } else if meta.path.is_ident("init_if_needed") {
                if raw.init_if_needed {
                    return Err(meta.error("duplicate `init_if_needed` flag"));
                }
                raw.init_if_needed = true;
            } else if meta.path.is_ident("init") {
                if raw.init {
                    return Err(meta.error("duplicate `init` flag"));
                }
                raw.init = true;
            } else if meta.path.is_ident("space") {
                if raw.space.is_some() {
                    return Err(meta.error("duplicate `space` attribute"));
                }
                let expr: Expr = meta.value()?.parse()?;
                raw.space = Some(expr);
            } else if meta.path.is_ident("realloc") {
                if raw.realloc {
                    return Err(meta.error("duplicate `realloc` flag"));
                }
                raw.realloc = true;
            } else if meta.path.is_ident("bump") {
                if raw.bump {
                    return Err(meta.error("duplicate `bump` flag"));
                }
                raw.bump = true;
            } else {
                return Err(meta.error("Unknown flag in #[account] attribute"));
            }
            Ok(())
        })?;

        // Reject mutually exclusive attribute combinations that are obvious at syntax level.
        if raw.init && raw.init_if_needed {
            return Err(syn::Error::new(attr.span(), "`init` cannot be combined with `init_if_needed`"));
        }

        Ok(raw)
    }

    /// Transfer the syntactic flags contained in `self` into a [`FieldCfg`].
    /// This performs **no** semantic validation – caller should run
    /// `validator::syntax::validate_field` afterwards.
    pub fn apply_to_cfg(&self, cfg: &mut FieldCfg) {
        // Simple boolean flags
        if self.signer {
            cfg.is_signer = Some(true);
        }
        if self.writable {
            cfg.is_writable = Some(true);
        }

        // Direct option copies
        cfg.address = self.address.clone();
        cfg.seeds = self.seeds.clone();
        cfg.program_id = self.program_id.clone();
        cfg.payer = self.payer.clone();
        cfg.owner = self.owner.clone();
        cfg.is_shards = self.is_shards;
        cfg.of_type = self.of_type.clone();
        cfg.is_zero_copy = self.zero_copy;
        cfg.is_init = self.init;
        cfg.is_init_if_needed = self.init_if_needed;
        cfg.is_realloc = self.realloc;
        cfg.space = self.space.clone();

        // Special-case bump placeholder.
        if self.bump {
            cfg.kind = FieldKind::Bump;
        }
    }
} 