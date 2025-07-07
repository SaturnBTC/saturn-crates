use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, ExprPath, Generics, Ident, Index, Lifetime};

use crate::model::{FieldCfg, FieldKind};

use super::{fixed_slice, shards, single};

/// Generate the final implementation `TokenStream` for a struct deriving `Accounts`.
pub(crate) fn generate(
    struct_ident: &Ident,
    generics: &Generics,
    fields: &[FieldCfg],
) -> Result<TokenStream, syn::Error> {
    // Generate per-field extraction snippets ---------------------------------
    let field_bindings: Vec<TokenStream> = fields
        .iter()
        .enumerate()
        .map(|(field_idx, cfg)| generate_field_binding(cfg, field_idx, fields))
        .collect();

    let field_initialisers: Vec<_> = fields.iter().map(|cfg| &cfg.ident).collect();

    // Find the `'info` lifetime parameter (required by convention).
    let lifetime_ident_opt = generics
        .lifetimes()
        .find(|lt_def| lt_def.lifetime.ident == "info")
        .map(|lt_def| lt_def.lifetime.clone());

    let lifetime_ident: Lifetime = if let Some(l) = lifetime_ident_opt {
        l
    } else {
        return Err(syn::Error::new_spanned(
            struct_ident,
            "Struct deriving Accounts must declare a lifetime parameter named `'info` which is used for the account slice (e.g. `struct MyAccs<'info> { .. }`)",
        ));
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let output = quote! {
        impl #impl_generics saturn_account_parser::Accounts<#lifetime_ident> for #struct_ident #ty_generics #where_clause {
            fn try_accounts(
                accounts: &#lifetime_ident [arch_program::account::AccountInfo<#lifetime_ident>],
            ) -> Result<Self, arch_program::program_error::ProgramError> {
                let mut idx: usize = 0;

                // Field-by-field extraction
                #(#field_bindings)*

                // Ensure we've consumed exactly all provided accounts
                if idx != accounts.len() {
                    return Err(arch_program::program_error::ProgramError::InvalidAccountData);
                }

                Ok(Self {
                    #(#field_initialisers),*
                })
            }
        }
    };

    Ok(output)
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

fn generate_field_binding(cfg: &FieldCfg, field_idx: usize, fields: &[FieldCfg]) -> TokenStream {
    let ident = &cfg.ident;

    let signer_tok = match cfg.is_signer {
        Some(true) => quote!(Some(true)),
        _ => quote!(None),
    };

    let writable_tok = match cfg.is_writable {
        Some(true) => quote!(Some(true)),
        _ => quote!(None),
    };

    let address_tok = match &cfg.address {
        Some(expr) => quote!(Some(#expr)),
        None => quote!(None),
    };

    // Payer and owner tokens (only relevant when `init` is set)
    let payer_tok_opt = cfg.payer.as_ref().map(|expr| {
        // Attempt to detect simple identifier path so we can compute forward reference offset.
        if let Expr::Path(ExprPath { ref path, .. }) = expr {
            if let Some(ident_ref) = path.get_ident() {
                // Locate the referenced field in the list.
                if let Some(target_idx) = fields.iter().position(|c| c.ident == *ident_ref) {
                    if target_idx > field_idx {
                        // Forward reference: generate slice-based expression so no undefined identifier error occurs.
                        let offset = target_idx - field_idx;
                        // Use literal to avoid type inference issues.
                        let offset_lit = syn::Index::from(offset);
                        return quote! { accounts[idx + #offset_lit] };
                    }
                }
            }
        }
        // Fallback: keep original expression.
        quote! { #expr }
    });
    let owner_tok_opt = cfg.program_id.as_ref().map(|e| quote! { #e });

    match &cfg.kind {
        FieldKind::Single => single::generate_single_binding(
            cfg,
            ident,
            signer_tok,
            writable_tok,
            address_tok,
            payer_tok_opt,
            owner_tok_opt,
        ),
        FieldKind::FixedSlice(len_ts) => fixed_slice::generate_fixed_slice_binding(
            cfg,
            ident,
            signer_tok,
            writable_tok,
            address_tok,
            len_ts,
        ),
        FieldKind::Shards(len_ts, element_ty_ts) => shards::generate_shards_binding(
            cfg,
            ident,
            signer_tok,
            writable_tok,
            address_tok,
            len_ts,
            element_ty_ts,
        ),
        FieldKind::Phantom => {
            // For marker fields we simply create a default PhantomData value (does not consume accounts).
            quote! {
                let #ident = core::marker::PhantomData;
            }
        }
        FieldKind::Bump => {
            let seeds_expr = cfg.seeds.as_ref().expect("seeds required for bump");
            let program_id_expr = cfg
                .program_id
                .as_ref()
                .expect("program_id required for bump");

            // Detect if the declared type is `[u8; 1]` (value) instead of primitive `u8`.
            let is_array1 = matches!(&cfg.base_ty, syn::Type::Array(arr)
                if matches!(&*arr.elem, syn::Type::Path(tp)
                    if tp.path.segments.last().map_or(false, |seg| seg.ident == "u8")));

            if is_array1 {
                quote! {
                    let (_pda_key, bump_seed_tmp) = arch_program::pubkey::Pubkey::find_program_address(#seeds_expr, &#program_id_expr);
                    let #ident: [u8; 1] = [bump_seed_tmp];
                }
            } else {
                quote! {
                    let (_pda_key, bump_seed_tmp) = arch_program::pubkey::Pubkey::find_program_address(#seeds_expr, &#program_id_expr);
                    let #ident: u8 = bump_seed_tmp;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use syn::{parse_quote, Data, DeriveInput, Fields};

    fn extract_named_fields(
        di: &DeriveInput,
    ) -> &syn::punctuated::Punctuated<syn::Field, syn::token::Comma> {
        match &di.data {
            Data::Struct(data) => match &data.fields {
                Fields::Named(named) => &named.named,
                _ => panic!("expected named fields"),
            },
            _ => panic!("expected struct"),
        }
    }

    #[test]
    fn snapshot_orchestrator_simple() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(signer)]
                user: saturn_account_parser::codec::BorshAccount<'info, u64>,
                #[account(len = 2)]
                pdas: Vec<arch_program::account::AccountInfo<'info>>,
            }
        };
        let cfgs = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let ts = super::generate(&di.ident, &di.generics, &cfgs).expect("generate ok");
        let compact = ts.to_string();
        assert!(compact.contains("impl"));
        assert!(compact.contains("try_accounts"));
    }

    #[test]
    fn generates_bump_placeholder() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
                pda: saturn_account_parser::codec::BorshAccount<'info, u64>,
                #[account(bump, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
                pda_bump: u8,
            }
        };

        let cfgs = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let ts = super::generate(&di.ident, &di.generics, &cfgs).expect("generate ok");
        let rendered = ts.to_string();
        // Ensure bump derivation call is present and the bump variable is initialised.
        assert!(rendered.contains("find_program_address"));
        assert!(rendered.contains("pda_bump"));
    }

    #[test]
    fn generates_bump_placeholder_array() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
                pda: saturn_account_parser::codec::BorshAccount<'info, u64>,
                #[account(bump, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
                pda_bump: [u8; 1],
            }
        };

        let cfgs = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let ts = super::generate(&di.ident, &di.generics, &cfgs).expect("generate ok");
        let rendered = ts.to_string();
        assert!(rendered.contains("[bump_seed_tmp]"));
    }
}
