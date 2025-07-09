use super::utils::is_account_info_path;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::model::FieldCfg;

// Public so orchestrator can call.
pub(crate) fn generate_fixed_slice_binding(
    cfg: &FieldCfg,
    ident: &Ident,
    signer_tok: TokenStream,
    writable_tok: TokenStream,
    address_tok: TokenStream,
    len_ts: &TokenStream,
) -> TokenStream {
    // Prepare token stream for the element type (with same generics)
    let element_ty_ts = {
        let ty_inner = &cfg.base_ty;
        quote! { #ty_inner }
    };

    // Determine if the slice element type is AccountInfo (after stripping reference).
    let is_acc_info_elem = is_account_info_path(&cfg.base_ty);

    // Helper preamble reused across branches.
    let common_preamble = quote! {
        let len_val: usize = (#len_ts as usize);
        if len_val > 0xFFFF {
            return Err(arch_program::program_error::ProgramError::InvalidAccountData);
        }

        let slice_start = idx;
        let slice_end = idx + len_val;
        if accounts.len() < slice_end {
            return Err(arch_program::program_error::ProgramError::NotEnoughAccountKeys);
        }
    };

    if is_acc_info_elem {
        // ------------------- Element type is AccountInfo --------------------
        if cfg.seeds.is_some() {
            let seeds_expr = cfg.seeds.as_ref().unwrap();
            let program_id_expr = cfg
                .program_id
                .as_ref()
                .expect("program_id required when seeds provided");
            // Pre-compute seeds & owner references once (avoid repeating the expressions in every
            // loop iteration). Use ident-scoped variable names so we do not clash between fields.
            let seeds_ident = syn::Ident::new(&format!("__{}_seeds", ident), ident.span());
            let owner_ident = syn::Ident::new(&format!("__{}_owner", ident), ident.span());
            quote! {
                #common_preamble

                // Pre-compute constant PDA data once per field.
                let #seeds_ident: &[&[u8]] = #seeds_expr;
                let #owner_ident = &#program_id_expr;

                // Validate each element against the derived PDA address
                for i in 0..len_val {
                    saturn_account_parser::get_indexed_pda_account(
                        accounts,
                        slice_start + i,
                        #signer_tok,
                        #writable_tok,
                        #seeds_ident,
                        i as u16,
                        #owner_ident,
                    )?;
                }

                let mut vec_tmp: Vec<#element_ty_ts> = Vec::with_capacity(len_val);
                for i in 0..len_val {
                    vec_tmp.push(accounts[slice_start + i].clone());
                }
                idx = slice_end;
                let #ident = vec_tmp;
            }
        } else {
            // Non-PDA path
            quote! {
                #common_preamble

                for i in 0..len_val {
                    saturn_account_parser::get_account(
                        accounts,
                        slice_start + i,
                        #signer_tok,
                        #writable_tok,
                        #address_tok,
                    )?;
                }

                let mut vec_tmp: Vec<#element_ty_ts> = Vec::with_capacity(len_val);
                for i in 0..len_val {
                    vec_tmp.push(accounts[slice_start + i].clone());
                }
                idx = slice_end;
                let #ident = vec_tmp;
            }
        }
    } else {
        // ------------------- Non AccountInfo element --------------------
        // Generate vector of the **declared element type** (e.g. `Account<'info, T>` or
        // `AccountLoader<'info, T>`).  We must *load* or *wrap* each `AccountInfo` so the
        // resulting type matches the user-declared field.

        // ── Helper: determine inner (T) type for Account / AccountLoader wrappers ──
        let (is_loader, inner_ty_ts): (bool, TokenStream) = {
            if let syn::Type::Path(tp) = &cfg.base_ty {
                if let Some(last) = tp.path.segments.last() {
                    if ["Account", "AccountLoader"].contains(&last.ident.to_string().as_str()) {
                        let is_loader = last.ident == "AccountLoader" || cfg.is_zero_copy;
                        if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                            let maybe_ty = args
                                .args
                                .iter()
                                .find_map(|a| {
                                    if let syn::GenericArgument::Type(t) = a {
                                        Some(t.clone())
                                    } else {
                                        None
                                    }
                                })
                                .expect("generic type param missing");
                            (is_loader, quote! { #maybe_ty })
                        } else {
                            (is_loader, quote! { () })
                        }
                    } else {
                        // Plain (non-wrapper) element – treat as Borsh Account
                        (false, quote! { #element_ty_ts })
                    }
                } else {
                    (false, quote! { #element_ty_ts })
                }
            } else {
                (false, quote! { #element_ty_ts })
            }
        };

        // Build per-element push snippet and the concrete vector element type ------------------
        let (vec_ty_ts, push_element_snip) = if is_loader {
            (
                quote! { saturn_account_parser::codec::AccountLoader<'_, #inner_ty_ts> },
                quote! {
                    let loader = saturn_account_parser::codec::AccountLoader::<#inner_ty_ts>::new(acc_info_tmp);
                    vec_tmp.push(loader);
                },
            )
        } else {
            (
                quote! { saturn_account_parser::codec::Account<'_, #inner_ty_ts> },
                quote! {
                    let acc_decoded = saturn_account_parser::codec::Account::<#inner_ty_ts>::load(acc_info_tmp)?;
                    vec_tmp.push(acc_decoded);
                },
            )
        };

        if cfg.seeds.is_some() {
            let seeds_expr = cfg.seeds.as_ref().unwrap();
            let program_id_expr = cfg
                .program_id
                .as_ref()
                .expect("program_id required when seeds provided");
            let seeds_ident = syn::Ident::new(&format!("__{}_seeds", ident), ident.span());
            let owner_ident = syn::Ident::new(&format!("__{}_owner", ident), ident.span());
            quote! {
                #common_preamble

                let #seeds_ident: &[&[u8]] = #seeds_expr;
                let #owner_ident = &#program_id_expr;

                let mut vec_tmp: Vec<#vec_ty_ts> = Vec::with_capacity(len_val);
                for i in 0..len_val {
                    let acc_info_tmp = saturn_account_parser::get_indexed_pda_account(
                        accounts,
                        slice_start + i,
                        #signer_tok,
                        #writable_tok,
                        #seeds_ident,
                        i as u16,
                        #owner_ident,
                    )?;
                    #push_element_snip
                }

                idx = slice_end;
                let #ident = vec_tmp;
            }
        } else {
            quote! {
                #common_preamble

                let mut vec_tmp: Vec<#vec_ty_ts> = Vec::with_capacity(len_val);
                for i in 0..len_val {
                    let acc_info_tmp = saturn_account_parser::get_account(
                        accounts,
                        slice_start + i,
                        #signer_tok,
                        #writable_tok,
                        #address_tok,
                    )?;
                    #push_element_snip
                }

                idx = slice_end;
                let #ident = vec_tmp;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::{parse_quote, Data, DeriveInput, Fields};

    fn extract_named_fields(
        di: &DeriveInput,
    ) -> &syn::punctuated::Punctuated<syn::Field, syn::token::Comma> {
        match &di.data {
            Data::Struct(data) => match &data.fields {
                Fields::Named(named) => &named.named,
                _ => panic!("named"),
            },
            _ => panic!("struct"),
        }
    }

    #[test]
    fn generates_fixed_slice_account() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(len = 2)]
                vec_accs: Vec<arch_program::account::AccountInfo<'info>>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let len_ts = match &cfg.kind {
            crate::model::FieldKind::FixedSlice(l) => l,
            _ => panic!("expected fixed slice"),
        };
        let ts = generate_fixed_slice_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            len_ts,
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("len_val"));
        assert!(rendered.contains("get_account"));
    }

    #[test]
    fn generates_fixed_slice_account_pda() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(len = 3, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
                vec_pdas: Vec<arch_program::account::AccountInfo<'info>>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let len_ts = match &cfg.kind {
            crate::model::FieldKind::FixedSlice(l) => l,
            _ => panic!("expected fixed slice"),
        };
        let ts = generate_fixed_slice_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            len_ts,
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("get_indexed_pda_account"));
    }

    #[test]
    fn generates_fixed_slice_non_account_info() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(len = 4)]
                vec_borsh: Vec<saturn_account_parser::codec::Account<'info, u64>>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let len_ts = match &cfg.kind {
            crate::model::FieldKind::FixedSlice(l) => l,
            _ => panic!("expected fixed slice"),
        };
        let ts = generate_fixed_slice_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            len_ts,
        );
        let rendered = ts.to_string();
        // The generator should use Account::load for each element.
        assert!(rendered.contains("Account :: <"));
        assert!(rendered.contains("load ("));
        assert!(!rendered.contains("clone()"));
    }

    #[test]
    fn generates_fixed_slice_account_pda_non_account_info() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(len = 3, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
                vec_borsh: Vec<saturn_account_parser::codec::Account<'info, u64>>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let len_ts = match &cfg.kind {
            crate::model::FieldKind::FixedSlice(l) => l,
            _ => panic!("expected fixed slice"),
        };
        let ts = generate_fixed_slice_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            len_ts,
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("get_indexed_pda_account"));
        // Should use Account::load inside the loop.
        assert!(rendered.contains("Account :: <"));
        assert!(rendered.contains("load ("));
        assert!(!rendered.contains("clone()"));
    }

    #[test]
    fn generates_fixed_slice_len_zero() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(len = 0)]
                empty_vec: Vec<arch_program::account::AccountInfo<'info>>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let len_ts = match &cfg.kind {
            crate::model::FieldKind::FixedSlice(l) => l,
            _ => panic!("expected fixed slice"),
        };
        let ts = generate_fixed_slice_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            len_ts,
        );
        let rendered = ts.to_string();
        // Even with len=0 we still expect len_val checks to be present.
        assert!(rendered.contains("len_val"));
        // Ensure the slice bounds are calculated.
        assert!(rendered.contains("slice_end"));
    }

    #[test]
    fn generates_fixed_slice_len_overflow_guard() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(len = 70000)]
                big_vec: Vec<arch_program::account::AccountInfo<'info>>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let len_ts = match &cfg.kind {
            crate::model::FieldKind::FixedSlice(l) => l,
            _ => panic!("expected fixed slice"),
        };
        let ts = generate_fixed_slice_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            len_ts,
        );
        let rendered = ts.to_string();
        // Guard constant should always be emitted.
        assert!(rendered.contains("0xFFFF"));
        // Ensure invalid size error path mentioned.
        assert!(rendered.contains("InvalidAccountData"));
    }

    #[test]
    fn generates_fixed_slice_underflow_guard() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(len = 1)]
                single_acc: Vec<arch_program::account::AccountInfo<'info>>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let len_ts = match &cfg.kind {
            crate::model::FieldKind::FixedSlice(l) => l,
            _ => panic!("expected fixed slice"),
        };
        let ts = generate_fixed_slice_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            len_ts,
        );
        let rendered = ts.to_string();
        // Ensure the not-enough-accounts guard is in the generated code.
        assert!(rendered.contains("NotEnoughAccountKeys"));
    }
}
