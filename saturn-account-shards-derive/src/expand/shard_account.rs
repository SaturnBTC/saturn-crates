#![allow(dead_code)]

use quote::quote;
use syn::DeriveInput;

use super::state_shard;
use crate::validate;

pub type Result<T> = syn::Result<T>;

/// Generates both `StateShard` impl and `bytemuck::{Pod,Zeroable}` for the struct.
pub fn derive_shard_account(
    input_ts: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream> {
    // Parse the input once so we can run validation and reuse the AST later.
    let input_ast: DeriveInput = syn::parse2(input_ts.clone())?;

    // Validate that the struct is `#[repr(C)]`.
    validate::assert_repr_c(&input_ast.ident, &input_ast.attrs)?;

    // Delegate to StateShard derive *after* validation to avoid generating code
    // for an invalid input structure.
    let state_impl = state_shard::derive_state_shard(input_ts.clone())?;

    // Extract struct identifier to implement Pod/Zeroable
    let ident = &input_ast.ident;

    // --- Generate bytemuck::Pod & Zeroable impls ---
    let pod_impl = quote! {
        unsafe impl bytemuck::Zeroable for #ident {}
        unsafe impl bytemuck::Pod for #ident {}
    };

    // --- Generate Discriminator impl (same rule as Anchor) ---
    // Compute the 8-byte discriminator during macro expansion using SHA-256("account:" ++ ident).
    let disc_bytes: [u8; 8] = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"account:");
        hasher.update(ident.to_string().as_bytes());
        let result = hasher.finalize();
        let mut arr = [0u8; 8];
        arr.copy_from_slice(&result[..8]);
        arr
    };

    // Turn the bytes into a token stream for code generation.
    let disc_byte_tokens = disc_bytes.iter().map(|b| quote! { #b }).collect::<Vec<_>>();

    let discriminator_impl = quote! {
        impl saturn_account_parser::codec::zero_copy::Discriminator for #ident {
            const DISCRIMINATOR: [u8; 8] = [ #( #disc_byte_tokens ),* ];
        }
    };

    // Re-emit original item + generated impls
    let expanded = quote! {
        #pod_impl
        #discriminator_impl
        #state_impl
    };

    Ok(expanded)
}
