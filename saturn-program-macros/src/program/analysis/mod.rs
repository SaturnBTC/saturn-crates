use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemMod;

use crate::program::attr::AttrConfig;

mod dup_check;
mod gather;
mod helpers;
mod parse;
mod transform;

// Re-export so upstream modules remain unchanged
pub use gather::FnInfo;

/// Result of analyzing the `#[saturn_program]`-annotated module.
pub struct AnalysisResult {
    pub item_mod: ItemMod,
    pub fn_infos: Vec<FnInfo>,
}

/// Performs analysis and injects helper type aliases into the module depending on the attribute configuration.
/// This is a thin orchestration layer delegating the heavy lifting to smaller modules.
pub fn analyze(attr_cfg: &AttrConfig, item: TokenStream) -> Result<AnalysisResult, TokenStream> {
    // ------------------------------------------------------------
    // 1. Parse the annotated item: must be an *inline* module
    // ------------------------------------------------------------
    let mut item_mod = match parse::parse_item_mod(item) {
        Ok(m) => m,
        Err(err_tokens) => return Err(err_tokens),
    };

    // ------------------------------------------------------------
    // 2. Gather function information and perform basic checks
    // ------------------------------------------------------------
    let (fn_infos, mut errors) = gather::gather_fn_infos(&item_mod);


    // ------------------------------------------------------------
    // 3. Detect duplicate variant names after convert_case transformation
    // ------------------------------------------------------------
    errors.extend(dup_check::check_duplicate_variants(&fn_infos));

    // ------------------------------------------------------------
    // 4. Rewrite handler parameter types so users can keep concise `Context` without injected aliases
    // ------------------------------------------------------------
    transform::rewrite_context_params(&mut item_mod, &fn_infos, attr_cfg);

    // If we encountered errors, return them now so the caller can embed them.
    if !errors.is_empty() {
        let combined = quote! { #item_mod #( #errors )* };
        return Err(combined);
    }

    if fn_infos.is_empty() {
        let ts = quote! {
            #item_mod
            compile_error!("#[saturn_program] module must define at least one non-#[cfg(test)] instruction handler");
        };
        return Err(ts);
    }

    Ok(AnalysisResult { item_mod, fn_infos })
}
