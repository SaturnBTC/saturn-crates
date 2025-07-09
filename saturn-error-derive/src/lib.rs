//! # saturn-error-derive
//!
//! Attach the attribute to any `enum` that represents your
//! Solana-compatible error codes and the macro will:
//!
//! * Attach `#[repr(u32)]` and derive `Debug`, `Clone`, `Copy`, `PartialEq`, and
//!   `thiserror::Error`.
//! * Fill in sequential discriminant values starting from a chosen `offset`
//!   (default `6000`) for every variant that does **not** already specify one.
//! * Implement `From<Enum> for u32` as well as
//!   `From<Enum> for arch_program::program_error::ProgramError` so the enum
//!   integrates seamlessly with existing Solana-based frameworks.
//! * Generate an internal implementation of
//!   `num_traits::FromPrimitive` without adding a public dependency on
//!   `num_traits`, keeping your public API surface minimal.
//!
//! ## Quick start
//!
//! ```rust
//! use saturn_error::saturn_error;
//!
//! #[saturn_error(offset = 7000)]
//! pub enum MyError {
//!     #[error("Overflow occurred")]
//!     Overflow,
//!     #[error("Invalid authority")]
//!     InvalidAuthority,
//! }
//! ```
//!
//! After macro expansion `MyError::Overflow as u32 == 7000` and
//! `MyError::InvalidAuthority as u32 == 7001`.
//!
//! ## Attribute syntax
//!
//! * `#[saturn_error]` – uses the default offset `6000`.
//! * `#[saturn_error(offset = N)]` – starts numbering at `N`.
//!
//! The macro aborts with a **compile-time** error if:
//!
//! * it is applied to anything other than an `enum`, or
//! * the provided `offset` is not an unsigned integer literal, or
//! * two variants end up with the same discriminant value.
//!
//! ## Reserved ranges
//!
//! We recommend allocating each on-chain program (or crate) a unique offset
//! range in increments of `1000` to prevent collisions. For instance, reserve
//! `6000–6999` for _Program A_, `7000–7999` for _Program B_, and so on.
//!
//! ## Minimum supported Rust version
//!
//! This crate is guaranteed to compile on Rust 1.73 and later. New MSRV will be
//! announced in the changelog and will always follow semantic versioning.
//!
//! ## Safety
//!
//! This crate is `#![forbid(unsafe_code)]`; it does not use `unsafe` anywhere
//! in its implementation.
//!
//! ## License
//!
//! Licensed under either of
//! * Apache License, Version 2.0,
//! * MIT license
//!
//! at your option.
#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! Proc-macro implementation for `#[saturn_error(offset = N)]`.
//!
//! The macro rewrites the input enum to:
//! 1. Add `#[repr(u32)]` and derives.
//! 2. Assign discriminants = `offset + index` for every variant **without** an
//!    explicit value (variants that already have `= X` are left untouched).
//! 3. Generate `From<Enum> for u32` and `From<Enum> for ProgramError` impls.
//!
//! This makes it painless to keep error codes unique across crates.
extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parse, parse_macro_input, Expr, ItemEnum, LitInt};

/// Parses the attribute input `offset = N`.
struct Offset(u32);

impl Parse for Offset {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Expect: offset = <int>
        let ident: syn::Ident = input.parse()?;
        if ident != "offset" {
            return Err(syn::Error::new_spanned(ident, "expected `offset = <int>`"));
        }
        let _: syn::Token![=] = input.parse()?;
        let lit: LitInt = input.parse()?;
        Ok(Offset(lit.base10_parse()?))
    }
}

/// Attribute macro that converts an `enum` into a
/// Solana-compatible error code set.
///
/// See the [crate-level documentation](crate) for detailed semantics and
/// examples.
///
/// # Examples
///
/// ```rust
/// use saturn_error::saturn_error;
///
/// #[saturn_error(offset = 42)]
/// enum MyError {
///     #[error("Boom")]
///     Boom,
/// }
/// ```
///
/// The variant `Boom` then expands to numeric code `42` and implements
/// `From::<arch_program::program_error::ProgramError>` automatically.
///
/// # Panics
///
/// This macro aborts compilation with a descriptive error message if mis-used.
#[proc_macro_attribute]
pub fn saturn_error(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Default error code offset (mirrors Anchor's 6000).
    const DEFAULT_OFFSET: u32 = 6000;

    // Parse attribute args. If the attribute is omitted (e.g. `#[saturn_error]`),
    // fall back to the default offset. Otherwise expect `offset = <int>`.
    let offset: u32 = if attr.is_empty() {
        DEFAULT_OFFSET
    } else {
        parse_macro_input!(attr as Offset).0
    };

    // Parse enum.
    let mut enum_item = parse_macro_input!(item as ItemEnum);

    let enum_ident = &enum_item.ident;

    // Ensure #[repr(u32)] exists.
    let mut has_repr = false;
    for attr in &enum_item.attrs {
        if attr.path().is_ident("repr") {
            has_repr = true;
            break;
        }
    }
    if !has_repr {
        enum_item.attrs.push(syn::parse_quote!(#[repr(u32)]));
    }

    // Ensure derives include our desired traits (Debug, Clone, Copy, PartialEq).
    let has_derive = enum_item.attrs.iter().any(|a| a.path().is_ident("derive"));
    if !has_derive {
        enum_item
            .attrs
            .push(syn::parse_quote!(#[derive(Debug, Clone, Copy, PartialEq)]));
    }

    // Always derive `Error` via the re-export path so users don't need to declare the crate
    // manually. We *avoid* deriving `FromPrimitive` here because the `num_derive` macro expands
    // to code that references the `num_traits` crate directly, which would force every downstream
    // crate using `#[saturn_error]` to add an explicit `num_traits` dependency.  Instead we
    // generate a lightweight manual implementation of `FromPrimitive` below that relies on the
    // re-exported path `saturn_error::__private::num_traits`, keeping the dependency fully
    // encapsulated inside the `saturn-error` crate.
    enum_item
        .attrs
        .push(syn::parse_quote!(#[derive(saturn_error::__private::thiserror::Error)]));

    // Build new variants with discriminants if missing.
    let mut new_variants = Vec::new();
    for (idx, variant) in enum_item.variants.iter().enumerate() {
        let mut v = variant.clone();
        if v.discriminant.is_none() {
            let disc_val: u32 = offset + idx as u32;
            v.discriminant = Some((
                syn::token::Eq {
                    spans: [proc_macro2::Span::call_site()],
                },
                Expr::Lit(syn::ExprLit {
                    attrs: Vec::new(),
                    lit: syn::Lit::Int(LitInt::new(
                        &disc_val.to_string(),
                        proc_macro2::Span::call_site(),
                    )),
                }),
            ));
        }
        // If the variant has no #[error(...)] attribute, synthesize one with the
        // variant's name in sentence case.
        let has_error_attr = v.attrs.iter().any(|a| a.path().is_ident("error"));
        if !has_error_attr {
            // Use the variant's identifier as the default error message so that
            // `thiserror` displays a sensible message without requiring users
            // to annotate every variant manually.
            let msg = syn::LitStr::new(&v.ident.to_string(), proc_macro2::Span::call_site());
            v.attrs.push(syn::parse_quote!(#[error(#msg)]));
        }
        new_variants.push(v);
    }
    enum_item.variants = syn::punctuated::Punctuated::from_iter(new_variants);

    // Collect variant identifiers for later code generation (e.g. manual `FromPrimitive`).
    let variant_idents: Vec<syn::Ident> =
        enum_item.variants.iter().map(|v| v.ident.clone()).collect();

    // Generate impl blocks.
    let program_error_path: syn::Path =
        syn::parse_quote!(arch_program::program_error::ProgramError);
    let decode_error_path: syn::Path = syn::parse_quote!(arch_program::decode_error::DecodeError);
    let print_program_error_path: syn::Path =
        syn::parse_quote!(arch_program::program_error::PrintProgramError);

    // Manual `FromPrimitive` implementation to avoid bringing in `num_traits` as a public
    // dependency of every crate that uses `#[saturn_error]`.
    let from_primitive_impl = quote! {
        impl saturn_error::__private::num_traits::FromPrimitive for #enum_ident {
            #[inline]
            fn from_i64(n: i64) -> Option<Self> {
                Self::from_u64(n as u64)
            }

            #[inline]
            fn from_u64(n: u64) -> Option<Self> {
                match n {
                    #(
                        x if x == (#enum_ident::#variant_idents as u64) => Some(#enum_ident::#variant_idents),
                    )*
                    _ => None,
                }
            }
        }
    };

    let gen = quote! {
        #enum_item

        #from_primitive_impl

        impl #decode_error_path<#enum_ident> for #enum_ident {
            fn type_of() -> &'static str {
                stringify!(#enum_ident)
            }
        }

        impl #print_program_error_path for #enum_ident {
            fn print<E>(&self)
            where
                E: 'static
                    + std::error::Error
                    + #decode_error_path<E>
                    + #print_program_error_path
                    + saturn_error::__private::num_traits::FromPrimitive,
            {
                arch_program::msg!("{}", &self.to_string());
            }
        }

        impl From<#enum_ident> for u32 {
            #[inline]
            fn from(e: #enum_ident) -> Self {
                e as u32
            }
        }

        impl From<#enum_ident> for #program_error_path {
            #[inline]
            fn from(e: #enum_ident) -> Self {
                #program_error_path::Custom(e as u32)
            }
        }
    };
    gen.into()
}
