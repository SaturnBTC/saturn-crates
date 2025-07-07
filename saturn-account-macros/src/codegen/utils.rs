use syn::Type;

/// Returns `true` if the provided `syn::Type` (with any level of references)
/// ultimately resolves to a path whose last segment is `AccountInfo`.
pub(crate) fn is_account_info_path(ty: &Type) -> bool {
    match ty {
        Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .map_or(false, |seg| seg.ident == "AccountInfo"),
        Type::Reference(ref ref_ty) => is_account_info_path(&*ref_ty.elem),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn detects_account_info_simple() {
        let ty: Type = parse_quote! { arch_program::account::AccountInfo<'info> };
        assert!(is_account_info_path(&ty));
    }

    #[test]
    fn detects_account_info_reference() {
        let ty: Type = parse_quote! { &arch_program::account::AccountInfo<'info> };
        assert!(is_account_info_path(&ty));
    }

    #[test]
    fn non_account_info_returns_false() {
        let ty: Type = parse_quote! { u64 };
        assert!(!is_account_info_path(&ty));
    }

    #[test]
    fn detects_account_info_multiple_reference() {
        let ty: Type = parse_quote! { &&&arch_program::account::AccountInfo<'info> };
        assert!(is_account_info_path(&ty));
    }

    #[test]
    fn non_account_info_similar_name_returns_false() {
        // A type whose last segment is not exactly `AccountInfo` should be rejected
        let ty: Type = parse_quote! { some_crate::AccountInfos<'info> };
        assert!(!is_account_info_path(&ty));
    }

    #[test]
    fn non_account_info_wrapper_returns_false() {
        // A type whose last path segment *contains* `AccountInfo` but is not exactly it
        let ty: Type = parse_quote! { my_crate::AccountInfoWrapper<'info> };
        assert!(!is_account_info_path(&ty));
    }
}
