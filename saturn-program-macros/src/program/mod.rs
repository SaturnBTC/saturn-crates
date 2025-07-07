use proc_macro2::TokenStream;
use quote::quote;

mod analysis;
mod attr;
mod dispatcher;

/// Entry point invoked by `lib.rs`.
/// Delegates to smaller, testable helpers located in the sibling modules.
pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    match try_expand(attr, item) {
        Ok(tokens) => tokens,
        Err(tokens) => tokens,
    }
}

/// Internal helper that wires the three phases together:
/// 1. Attribute parsing
/// 2. Module analysis
/// 3. Code generation
fn try_expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream, TokenStream> {
    // Phase 1: attribute parsing
    let attr_cfg = match attr::parse(attr) {
        Ok(cfg) => cfg,
        Err(err) => {
            // Turn the syn::Error into a compile_error! so the user gets feedback.
            let compile_error = err.to_compile_error();
            return Ok(quote! { #item #compile_error });
        }
    };

    // Phase 2: analyse the annotated item (should be an inline module)
    let analysis = match analysis::analyze(item.clone()) {
        Ok(res) => res,
        Err(err_tokens) => return Ok(err_tokens),
    };

    // Phase 3: generate dispatcher + entrypoint
    let generated = dispatcher::generate(&attr_cfg, &analysis);

    Ok(generated)
}
