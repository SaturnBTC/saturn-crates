use proc_macro2::TokenTree;
use quote::ToTokens;

/// Returns `true` if the attribute list includes `#[cfg(test)]`.
pub fn has_cfg_test(attrs: &[syn::Attribute]) -> bool {
    fn tokens_contain_test(ts: &proc_macro2::TokenStream) -> bool {
        for tt in ts.clone() {
            match tt {
                TokenTree::Ident(ref ident) if ident == "test" => return true,
                TokenTree::Group(ref grp) if tokens_contain_test(&grp.stream()) => return true,
                _ => {}
            }
        }
        false
    }

    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg") && tokens_contain_test(&attr.meta.to_token_stream())
    })
}
