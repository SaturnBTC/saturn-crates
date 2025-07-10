use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Automatically implements `saturn_account_parser::codec::zero_copy::Discriminator`
/// for a zero-copy account struct.  The discriminator is the first 8 bytes of
/// `sha256(b"account:" ++ <ident>)` – identical to Anchor’s rule so tooling can
/// recognise the layout.
#[proc_macro_derive(Discriminator)]
pub fn derive_discriminator(input: TokenStream) -> TokenStream {
    // Parse the input item (should be a struct or enum, but we only need its ident).
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;

    // Compute the 8-byte discriminator at *macro-expansion* time.
    let hash_bytes: [u8; 8] = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"account:");
        hasher.update(ident.to_string().as_bytes());
        let result = hasher.finalize();
        let mut arr = [0u8; 8];
        arr.copy_from_slice(&result[..8]);
        arr
    };

    // Produce byte literals for code generation.
    let byte_tokens = hash_bytes.iter().map(|b| quote! { #b }).collect::<Vec<_>>();

    TokenStream::from(quote! {
        impl saturn_account_parser::codec::zero_copy::Discriminator for #ident {
            const DISCRIMINATOR: [u8; 8] = [ #( #byte_tokens ),* ];
        }
    })
}
