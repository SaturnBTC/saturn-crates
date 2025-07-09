use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{LitStr, Result as SynResult};

/// Internal helper for parsing the macro input – we expect exactly one string literal.
struct IdLiteral(LitStr);

impl Parse for IdLiteral {
    fn parse(input: ParseStream<'_>) -> SynResult<Self> {
        let lit: LitStr = input.parse()?;
        Ok(Self(lit))
    }
}

/// Declare the program ID for the current crate, similar to Anchor's `declare_id!` macro.
///
/// Usage:
/// ```ignore
/// use arch_program::declare_id;
///
/// declare_id!("11111111111111111111111111111111");
///
/// fn some_function() {
///     let program_id = id();
///     // ...
/// }
/// ```
///
/// The macro expands to a public `fn id() -> Pubkey` that returns the program's
/// [`Pubkey`]. The string must be a valid base-58 representation of a 32-byte
/// public key; otherwise compilation will fail at the first invocation.
pub fn declare_id(input: TokenStream) -> TokenStream {
    // Validate we were given a single string literal so that errors show up at compile-time.
    let parsed: IdLiteral = match syn::parse2(input) {
        Ok(res) => res,
        Err(err) => return err.to_compile_error(),
    };
    let id_literal = parsed.0;
    // Parse the provided base-58 string into a `Pubkey`
    use std::str::FromStr;
    let id_str = id_literal.value();
    let pk = match ::arch_program::pubkey::Pubkey::from_str(&id_str) {
        Ok(pk) => pk,
        Err(e) => {
            return syn::Error::new_spanned(
                id_literal,
                format!("`declare_id!` got invalid program ID: {e}"),
            )
            .to_compile_error();
        }
    };

    // Convert the pubkey bytes into numeric literals for embedding in generated code.
    let byte_literals: Vec<proc_macro2::TokenStream> =
        pk.0.iter()
            .map(|b| {
                let lit = syn::LitInt::new(&b.to_string(), proc_macro2::Span::call_site());
                quote! { #lit }
            })
            .collect();

    quote! {
        /// **Saturn generated:** Constant program ID for this crate.
        pub const ID: ::arch_program::pubkey::Pubkey = ::arch_program::pubkey::Pubkey([
            #( #byte_literals ),*
        ]);

        /// Returns the declared program [`Pubkey`]. Equivalent to `ID`.
        #[inline]
        pub fn id() -> ::arch_program::pubkey::Pubkey {
            ID
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn generates_id_function_for_valid_literal() {
        let input: TokenStream = quote!("11111111111111111111111111111111");
        let ts = declare_id(input);
        let ts_str = ts.to_string();
        // Expect it contains fn id() and `pub const ID` definition
        assert!(ts_str.contains("fn id"));
        assert!(ts_str.contains("pub const ID"));
    }

    #[test]
    fn returns_compile_error_for_non_literal() {
        let input: TokenStream = quote!(12345);
        let ts = declare_id(input);
        let ts_str = ts.to_string();
        assert!(
            ts_str.contains("compile_error"),
            "Non-literal input should produce compile_error tokens"
        );
    }

    #[test]
    fn returns_compile_error_for_missing_input() {
        let input: TokenStream = quote!();
        let ts = declare_id(input);
        let ts_str = ts.to_string();
        assert!(ts_str.contains("compile_error"));
    }
}
