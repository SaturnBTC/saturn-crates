use proc_macro2::TokenStream;
use syn::{parse2, Error, ItemMod};

/// Parses the annotated item into an inline `ItemMod`.
///
/// On success returns the parsed module; on failure returns a `TokenStream`
/// containing compile errors that should be emitted to the user.
pub fn parse_item_mod(item: TokenStream) -> Result<ItemMod, TokenStream> {
    // Attempt to parse the proc-macro input as a module.
    let mut item_mod: ItemMod = match parse2(item.clone()) {
        Ok(m) => m,
        Err(e) => return Err(e.to_compile_error()),
    };

    // The module must be inline – i.e. has a body (`{ .. }`).
    match &mut item_mod.content {
        Some((_brace, _items)) => {}
        None => {
            let err = Error::new_spanned(
                &item_mod,
                "module must be inline (e.g. `mod foo { .. }`) for #[saturn_program]",
            );
            return Err(err.to_compile_error());
        }
    }

    Ok(item_mod)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn parses_inline_module() {
        let ts: TokenStream = quote! {
            mod sample {
                fn foo() {}
            }
        };
        let res = parse_item_mod(ts);
        assert!(res.is_ok(), "Expected inline module to parse without error");
        let item_mod = res.unwrap();
        assert_eq!(item_mod.ident.to_string(), "sample");
        assert!(item_mod.content.is_some());
    }

    #[test]
    fn error_on_module_without_body() {
        let ts: TokenStream = quote!(
            mod external;
        );
        let res = parse_item_mod(ts);
        assert!(res.is_err(), "Expected non-inline module to be rejected");
    }

    #[test]
    fn error_when_not_a_module() {
        // Provide a struct instead of a module
        let ts: TokenStream = quote!(
            struct Foo;
        );
        let res = parse_item_mod(ts);
        assert!(res.is_err(), "Expected non-module item to be rejected");
    }
}
