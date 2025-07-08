#![allow(dead_code)]

use syn::{spanned::Spanned, Attribute, DeriveInput, Field, Fields, Ident, Type};

/// Lightweight, validated wrapper around a struct's [`syn::DeriveInput`].
/// Ensures:
/// 1. The item is a *struct* with *named* fields.
/// 2. The struct is **not generic** – generics would complicate the generated
///    impl and were never supported by the original macro.
///
/// This makes the downstream code‐gen layer simpler because it can assume the
/// above invariants and simply panic if violated (the construction already
/// returned an `Err`).
#[derive(Debug)]
pub struct StructInfo {
    pub ident: Ident,
    pub attrs: Vec<Attribute>,
    pub fields: Vec<Field>,
}

impl StructInfo {
    /// Convert the raw macro input into a validated wrapper.
    pub fn from_derive_input(input: DeriveInput) -> syn::Result<Self> {
        if !input.generics.params.is_empty() {
            return Err(syn::Error::new(
                input.generics.span(),
                "Shard derive does not support generic structs",
            ));
        }

        // We only support structs.
        let data = match input.data {
            syn::Data::Struct(data) => data,
            _ => {
                return Err(syn::Error::new_spanned(
                    input.ident,
                    "Shard derive can only be used with structs",
                ));
            }
        };

        // Must be named fields.
        let named_fields = match data.fields {
            Fields::Named(named) => named.named.into_iter().collect::<Vec<_>>(),
            _ => {
                return Err(syn::Error::new_spanned(
                    input.ident,
                    "Shard struct must have named fields",
                ));
            }
        };

        Ok(Self {
            ident: input.ident,
            attrs: input.attrs,
            fields: named_fields,
        })
    }

    /// Find a field (by *Ident*) in the struct.
    pub fn field(&self, ident: &Ident) -> Option<&Field> {
        self.fields
            .iter()
            .find(|f| f.ident.as_ref().map(|id| id == ident).unwrap_or(false))
    }

    /// Convenience – return the field's *type* if it exists.
    pub fn field_type(&self, ident: &Ident) -> Option<Type> {
        self.field(ident).map(|f| f.ty.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn extract_fields_success() {
        let di: DeriveInput = parse_quote! {
            #[repr(C)]
            struct MyShard {
                pool_id: u64,
                btc_utxos: Vec<u8>,
            }
        };
        let info = StructInfo::from_derive_input(di).unwrap();
        assert_eq!(info.ident.to_string(), "MyShard");
        assert!(info.field(&parse_quote!(pool_id)).is_some());
        let ty = info.field_type(&parse_quote!(btc_utxos)).unwrap();
        assert_eq!(ty, parse_quote!(Vec<u8>));
    }

    #[test]
    fn fails_on_tuple_struct() {
        let di: DeriveInput = parse_quote! {
            struct Bad(u8);
        };
        let err = StructInfo::from_derive_input(di).unwrap_err();
        assert!(err.to_string().contains("named fields"));
    }

    #[test]
    fn fails_on_enum() {
        let di: DeriveInput = parse_quote! {
            enum E { A }
        };
        let err = StructInfo::from_derive_input(di).unwrap_err();
        assert!(err.to_string().contains("only be used with structs"));
    }

    #[test]
    fn fails_on_generics() {
        let di: DeriveInput = parse_quote! {
            struct Bad<T> { v: T }
        };
        let err = StructInfo::from_derive_input(di).unwrap_err();
        assert!(err.to_string().contains("generic structs"));
    }
}
