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

    quote! {
        /// Returns the declared program [`Pubkey`].
        #[inline]
        pub fn id() -> ::arch_program::pubkey::Pubkey {
            use std::str::FromStr;
            ::arch_program::pubkey::Pubkey::from_str(#id_literal)
                .expect("Invalid program ID supplied to declare_id! macro")
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
        // Expect it contains fn id() and the same literal string
        assert!(ts_str.contains("fn id"));
        assert!(ts_str.contains("11111111111111111111111111111111"));
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
