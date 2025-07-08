#![allow(dead_code)]

use quote::format_ident;
use syn::{parse::Parse, Attribute, Ident, Lit, Result, Type};

/// Parsed form of the `#[shard(..)]` attribute.
#[derive(Debug, Clone, PartialEq)]
pub struct ShardArgs {
    pub btc_utxos_ident: Ident,
    pub rune_utxo_ident: Ident,
    pub utxo_info_ty: Option<Type>,
    pub rune_set_ty: Option<Type>,
    pub fixed_option_ty: Option<Type>,
}

impl Default for ShardArgs {
    fn default() -> Self {
        Self {
            btc_utxos_ident: format_ident!("btc_utxos"),
            rune_utxo_ident: format_ident!("rune_utxo"),
            utxo_info_ty: None,
            rune_set_ty: None,
            fixed_option_ty: None,
        }
    }
}

impl ShardArgs {
    /// Parse the `#[shard(..)]` attribute list from an optional attribute.
    /// If the attribute is not supplied, default values are returned.
    pub fn from_attrs(attrs: &[Attribute]) -> Result<Self> {
        // Locate the first #[shard(..)] attribute, if any
        let shard_attr = attrs.iter().find(|a| a.path().is_ident("shard"));
        if let Some(attr) = shard_attr {
            Self::from_attribute(attr)
        } else {
            Ok(Self::default())
        }
    }

    /// Parse from a single `#[shard(..)]` attribute.
    pub fn from_attribute(attr: &Attribute) -> Result<Self> {
        let mut args = ShardArgs::default();

        attr.parse_nested_meta(|meta| {
            let key_path = meta.path.clone();
            // Expect key = "value"
            if meta.input.peek(syn::Token![=]) {
                let _eq_token: syn::Token![=] = meta.input.parse()?;
                let lit: Lit = meta.input.parse()?;
                let lit_str = if let Lit::Str(s) = lit {
                    s.value()
                } else {
                    return Err(syn::Error::new_spanned(lit, "Expected string literal"));
                };

                if key_path.is_ident("btc_utxos_attr") {
                    args.btc_utxos_ident = format_ident!("{}", lit_str);
                } else if key_path.is_ident("rune_utxo_attr") {
                    args.rune_utxo_ident = format_ident!("{}", lit_str);
                } else if key_path.is_ident("utxo_info_type") {
                    args.utxo_info_ty = Some(syn::parse_str::<Type>(&lit_str)?);
                } else if key_path.is_ident("rune_set_type") {
                    args.rune_set_ty = Some(syn::parse_str::<Type>(&lit_str)?);
                } else if key_path.is_ident("fixed_option_type") {
                    args.fixed_option_ty = Some(syn::parse_str::<Type>(&lit_str)?);
                } else {
                    return Err(syn::Error::new_spanned(
                        key_path,
                        "Unknown key in #[shard] attribute",
                    ));
                }
            }
            Ok(())
        })?;

        Ok(args)
    }
}

// Implement `Parse` so we could also call `syn::parse2::<ShardArgs>(tokens)` if desired.
impl Parse for ShardArgs {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        // Build a fake #[shard(..)] attribute token stream so we can reuse the existing logic.
        // We wrap the input tokens in parentheses to mimic the attribute meta list.
        let tokens = input.parse::<proc_macro2::TokenStream>()?;
        let attr: Attribute = syn::parse_quote! { #[shard(#tokens)] };
        ShardArgs::from_attribute(&attr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_when_no_attribute() {
        let args = ShardArgs::from_attrs(&[]).unwrap();
        assert_eq!(args, ShardArgs::default());
    }

    #[test]
    fn custom_field_names() {
        let attr: Attribute = syn::parse_quote! { #[shard(btc_utxos_attr = "bitcoin_utxos", rune_utxo_attr = "rune_data")] };
        let args = ShardArgs::from_attrs(&[attr]).unwrap();
        assert_eq!(args.btc_utxos_ident, format_ident!("bitcoin_utxos"));
        assert_eq!(args.rune_utxo_ident, format_ident!("rune_data"));
    }

    #[test]
    fn custom_types_parse() {
        let attr: Attribute = syn::parse_quote! { #[shard(utxo_info_type = "MyUtxo", rune_set_type = "MyRune", fixed_option_type = "MyOpt")] };
        let args = ShardArgs::from_attrs(&[attr]).unwrap();
        assert_eq!(args.utxo_info_ty.unwrap(), syn::parse_quote!(MyUtxo));
        assert_eq!(args.rune_set_ty.unwrap(), syn::parse_quote!(MyRune));
        assert_eq!(args.fixed_option_ty.unwrap(), syn::parse_quote!(MyOpt));
    }

    #[test]
    fn unknown_key_errors() {
        let attr: Attribute = syn::parse_quote! { #[shard(unknown_key = "val")] };
        let err = ShardArgs::from_attrs(&[attr]).unwrap_err();
        assert!(err.to_string().contains("Unknown key"));
    }
}
