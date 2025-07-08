#![allow(dead_code)]

use syn::{spanned::Spanned, Attribute, Ident};

/// Ensure the struct is annotated with `#[repr(C)]`. Mirrors the behaviour of the
/// original macro – fails if the attribute is missing or does not include the `C` item.
pub fn assert_repr_c(shard_ident: &Ident, attrs: &[Attribute]) -> syn::Result<()> {
    // Look for a #[repr(..)] attribute.
    let repr_attr = attrs
        .iter()
        .find(|attr| attr.path().is_ident("repr"))
        .ok_or_else(|| syn::Error::new_spanned(shard_ident, "Shard struct must be #[repr(C)]"))?;

    let mut has_repr_c = false;

    // Parse nested meta inside the repr attribute, looking for the `C` ident.
    repr_attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("C") {
            has_repr_c = true;
        }

        // Consume potential parenthesised contents like `align(8)` so the parser
        // continues gracefully.
        if meta.input.peek(syn::token::Paren) {
            let content;
            syn::parenthesized!(content in meta.input);
            content.parse::<proc_macro2::TokenStream>()?;
        }
        Ok(())
    })?;

    if has_repr_c {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            shard_ident,
            "Shard struct must be #[repr(C)]",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn accepts_repr_c() {
        let attrs: Vec<Attribute> = parse_quote!(#[repr(C)]);
        assert!(assert_repr_c(&parse_quote!(MyShard), &attrs).is_ok());
    }

    #[test]
    fn accepts_repr_c_with_other() {
        let attrs: Vec<Attribute> = parse_quote!(#[repr(C, align(8))]);
        assert!(assert_repr_c(&parse_quote!(MyShard), &attrs).is_ok());
    }

    #[test]
    fn rejects_missing_repr() {
        let attrs: Vec<Attribute> = vec![];
        let err = assert_repr_c(&parse_quote!(MyShard), &attrs).unwrap_err();
        assert!(err.to_string().contains("#[repr(C)]"));
    }

    #[test]
    fn rejects_non_c_repr() {
        let attrs: Vec<Attribute> = parse_quote!(#[repr(packed)]);
        let err = assert_repr_c(&parse_quote!(MyShard), &attrs).unwrap_err();
        assert!(err.to_string().contains("#[repr(C)]"));
    }
}
