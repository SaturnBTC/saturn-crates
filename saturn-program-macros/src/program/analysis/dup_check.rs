use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use std::collections::HashSet;

use super::gather::FnInfo;
use syn::Error;

/// Detects duplicate instruction variant names after case conversion.
/// Returns a list of compile-error token streams.
pub fn check_duplicate_variants(fn_infos: &[FnInfo]) -> Vec<TokenStream> {
    let mut errors = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for FnInfo { fn_ident, .. } in fn_infos {
        let variant_name = fn_ident.to_string().to_case(Case::Pascal);
        if !seen.insert(variant_name) {
            errors.push(
                Error::new_spanned(fn_ident, "duplicate instruction variant after case conversion; rename the handler or use distinct names")
                    .to_compile_error(),
            );
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Span;
    use syn::parse_str;

    fn make_fn_info(name: &str) -> FnInfo {
        FnInfo {
            fn_ident: syn::Ident::new(name, Span::call_site()),
            acc_ty: parse_str::<syn::Path>("crate::Acc").unwrap(),
        }
    }

    #[test]
    fn detects_no_duplicates() {
        let infos = vec![make_fn_info("handle_transfer"), make_fn_info("handle_mint")];
        let errors = check_duplicate_variants(&infos);
        assert!(errors.is_empty(), "Expected no duplicate errors");
    }

    #[test]
    fn detects_duplicates_after_case_conversion() {
        // These convert to the same PascalCase variant `DoSomething`
        let infos = vec![make_fn_info("do_something"), make_fn_info("DoSomething")];
        let errors = check_duplicate_variants(&infos);
        assert_eq!(errors.len(), 1, "Expected exactly one duplicate error");
    }

    #[test]
    fn detects_multiple_duplicates() {
        let infos = vec![
            make_fn_info("update_price"),
            make_fn_info("UpdatePrice"),
            make_fn_info("updatePrice"),
        ];
        let errors = check_duplicate_variants(&infos);
        // One error per duplicate occurrence beyond the first unique entry
        assert_eq!(errors.len(), 2);
    }
}
