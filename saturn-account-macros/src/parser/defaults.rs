use crate::model::{FieldCfg, FieldKind};

/// Applies implicit defaults that the parser should set when the user omits
/// them from the `#[account]` attribute list.
///
/// This currently only infers `program_id = crate::ID` in the common PDA &
/// init/realloc cases but can be extended in the future.
pub(super) fn fill_defaults(cfg: &mut FieldCfg) {
    // We need `crate::ID` whenever we must derive/own a PDA *or* we are about
    // to create/reallocate the account.
    let program_id_is_required = cfg.seeds.is_some()
        || cfg.is_init
        || cfg.is_init_if_needed
        || cfg.is_realloc
        || matches!(cfg.kind, FieldKind::Bump);

    if program_id_is_required && cfg.program_id.is_none() {
        cfg.program_id = Some(syn::parse_quote! { crate::ID });
    }
} 