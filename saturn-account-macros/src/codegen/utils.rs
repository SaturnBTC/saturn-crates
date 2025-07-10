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

/// If `ty` is a `saturn_account_parser::codec::Account<'info, T>` or
/// `saturn_account_parser::codec::AccountLoader<'info, T>` path, this helper
/// returns the **inner** generic type `T`. When `ty` does not match either
/// wrapper, `None` is returned so callers can fall back to the original type.
pub(crate) fn extract_inner_data_type(ty: &Type) -> Option<Type> {
    use syn::{GenericArgument, PathArguments, TypePath};

    // We only care about simple type paths – references are stripped earlier.
    let Type::Path(TypePath { path, .. }) = ty else {
        return None;
    };

    let Some(last_segment) = path.segments.last() else { return None };

    // Only consider the exact wrapper identifiers we know about.
    if last_segment.ident != "Account" && last_segment.ident != "AccountLoader" {
        return None;
    }

    // Inspect the angle-bracketed generic parameters `<...>`.
    let PathArguments::AngleBracketed(ref angle_args) = last_segment.arguments else {
        return None;
    };

    // The last generic argument should be the inner data type `T`.  We iterate
    // *in reverse* so we can gracefully skip over the lifetime parameter.
    for arg in angle_args.args.iter().rev() {
        if let GenericArgument::Type(ty) = arg {
            return Some(ty.clone());
        }
    }

    None
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
