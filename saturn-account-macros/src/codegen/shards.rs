use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::model::FieldCfg;

// Public so orchestrator can call.
#[allow(clippy::too_many_arguments)]
pub(crate) fn generate_shards_binding(
    cfg: &FieldCfg,
    ident: &Ident,
    signer_tok: TokenStream,
    writable_tok: TokenStream,
    address_tok: TokenStream,
    len_ts: &TokenStream,
    element_ty_ts: &TokenStream,
) -> TokenStream {
    // Optional snippet that runs `realloc` on each fetched account when the field is flagged.
    let realloc_snip: TokenStream = if cfg.is_realloc {
        let space_expr = cfg
            .space
            .as_ref()
            .expect("`space` is required for realloc")
            .clone();
        quote! {
            let new_len: usize = (#space_expr) as usize;
            if acc_info_tmp.data_len() != new_len {
                acc_info_tmp.realloc(new_len, true)?;
            }
        }
    } else {
        TokenStream::new()
    };

    let is_acc_info_elem = crate::codegen::utils::is_account_info_path(&cfg.base_ty);

    if cfg.seeds.is_some() {
        let seeds_expr = cfg.seeds.as_ref().unwrap();
        let program_id_expr = cfg
            .program_id
            .as_ref()
            .expect("program_id required when seeds provided");

        // Pre-compute constant PDA references once per field to avoid recomputation in each loop
        let seeds_ident = syn::Ident::new(&format!("__{}_seeds", ident), ident.span());
        let owner_ident = syn::Ident::new(&format!("__{}_owner", ident), ident.span());

        if is_acc_info_elem {
            quote! {
                let len_val: usize = (#len_ts as usize);
                if len_val > 0xFFFF {
                    return Err(arch_program::program_error::ProgramError::InvalidAccountData);
                }

                let slice_start = idx;
                let slice_end = idx + len_val;
                if accounts.len() < slice_end {
                    return Err(arch_program::program_error::ProgramError::NotEnoughAccountKeys);
                }

                let #seeds_ident: &[&[u8]] = #seeds_expr;
                let #owner_ident = &#program_id_expr;

                // Validate & collect AccountInfo PDAs
                let mut vec_tmp: Vec<#element_ty_ts> = Vec::with_capacity(len_val);
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

                    #realloc_snip
                    vec_tmp.push(acc_info_tmp.clone());
                }

                let #ident = vec_tmp;
                idx = slice_end;
            }
        } else {
            quote! {
                let len_val: usize = (#len_ts as usize);
                if len_val > 0xFFFF {
                    return Err(arch_program::program_error::ProgramError::InvalidAccountData);
                }

                let slice_start = idx;
                let slice_end = idx + len_val;
                if accounts.len() < slice_end {
                    return Err(arch_program::program_error::ProgramError::NotEnoughAccountKeys);
                }

                let #seeds_ident: &[&[u8]] = #seeds_expr;
                let #owner_ident = &#program_id_expr;

                let mut vec_tmp: Vec<saturn_account_parser::codec::ZeroCopyAccount<'_, #element_ty_ts>> = Vec::with_capacity(len_val);
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

                    #realloc_snip
                    let loader = saturn_account_parser::codec::ZeroCopyAccount::<#element_ty_ts>::new(acc_info_tmp);
                    vec_tmp.push(loader);
                }

                let #ident = vec_tmp;
                idx = slice_end;
            }
        }
    } else {
        if is_acc_info_elem {
            quote! {
                let len_val: usize = (#len_ts as usize);
                if len_val > 0xFFFF {
                    return Err(arch_program::program_error::ProgramError::InvalidAccountData);
                }

                let slice_start = idx;
                let slice_end = idx + len_val;
                if accounts.len() < slice_end {
                    return Err(arch_program::program_error::ProgramError::NotEnoughAccountKeys);
                }

                let mut vec_tmp: Vec<#element_ty_ts> = Vec::with_capacity(len_val);
                for i in 0..len_val {
                    let acc_info_tmp = saturn_account_parser::get_account(
                        accounts,
                        slice_start + i,
                        #signer_tok,
                        #writable_tok,
                        #address_tok,
                    )?;
                    vec_tmp.push(acc_info_tmp.clone());
                }

                let #ident = vec_tmp;
                idx = slice_end;
            }
        } else {
            quote! {
                let len_val: usize = (#len_ts as usize);
                if len_val > 0xFFFF {
                    return Err(arch_program::program_error::ProgramError::InvalidAccountData);
                }

                let slice_start = idx;
                let slice_end = idx + len_val;
                if accounts.len() < slice_end {
                    return Err(arch_program::program_error::ProgramError::NotEnoughAccountKeys);
                }

                let mut vec_tmp: Vec<saturn_account_parser::codec::ZeroCopyAccount<'_, #element_ty_ts>> = Vec::with_capacity(len_val);
                for i in 0..len_val {
                    let acc_info_tmp = saturn_account_parser::get_account(
                        accounts,
                        slice_start + i,
                        #signer_tok,
                        #writable_tok,
                        #address_tok,
                    )?;

                    #realloc_snip
                    let loader = saturn_account_parser::codec::ZeroCopyAccount::<#element_ty_ts>::new(acc_info_tmp);
                    vec_tmp.push(loader);
                }

                let #ident = vec_tmp;
                idx = slice_end;
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
    fn generates_shards_binding_basic() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(shards, len = 2)]
                shards: Vec<arch_program::account::AccountInfo<'info>>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let (len_ts, elem_ts) = match &cfg.kind {
            crate::model::FieldKind::Shards(l, e) => (l, e),
            _ => panic!("expected shards kind"),
        };
        let ts = generate_shards_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            len_ts,
            elem_ts,
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("len_val"));
    }

    #[test]
    fn generates_shards_binding_pda_account_info() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(shards, len = 2, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
                shards: Vec<arch_program::account::AccountInfo<'info>>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let (len_ts, elem_ts) = match &cfg.kind {
            crate::model::FieldKind::Shards(l, e) => (l, e),
            _ => panic!("expected shards kind"),
        };
        let ts = generate_shards_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            len_ts,
            elem_ts,
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("get_indexed_pda_account"));
    }

    #[test]
    fn generates_shards_binding_non_account_info() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(shards, len = 5)]
                shards: Vec<u64>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let (len_ts, elem_ts) = match &cfg.kind {
            crate::model::FieldKind::Shards(l, e) => (l, e),
            _ => panic!("expected shards kind"),
        };
        let ts = generate_shards_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            len_ts,
            elem_ts,
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("ZeroCopyAccount"));
        // Should reference slice &accounts[..] not cloning into Vec<AccountInfo> when non-AccountInfo
        assert!(!rendered.contains("clone()"));
    }

    #[test]
    fn generates_shards_binding_pda_non_account_info() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(shards, len = 3, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
                shards: Vec<u64>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let (len_ts, elem_ts) = match &cfg.kind {
            crate::model::FieldKind::Shards(l, e) => (l, e),
            _ => panic!("expected shards kind"),
        };
        let ts = generate_shards_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(Some(true)),
            quote!(None),
            len_ts,
            elem_ts,
        );
        let rendered = ts.to_string();
        println!("{}", rendered);
        assert!(rendered.contains("get_indexed_pda_account"));
        assert!(rendered.contains("ZeroCopyAccount"));
        // Writable flag should be forwarded into account fetch helpers.
        // The `TokenStream::to_string()` representation may include a space after `Some`,
        // depending on the rustc version / `proc_macro2` implementation.
        // Accept both variants to make the test robust across toolchain versions.
        assert!(rendered.contains("Some(true)") || rendered.contains("Some (true)"));
    }

    #[test]
    fn generates_shards_binding_len_zero() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(shards, len = 0)]
                shards: Vec<arch_program::account::AccountInfo<'info>>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let (len_ts, elem_ts) = match &cfg.kind {
            crate::model::FieldKind::Shards(l, e) => (l, e),
            _ => panic!("expected shards kind"),
        };
        let ts = generate_shards_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            len_ts,
            elem_ts,
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("len_val"));
        assert!(rendered.contains("slice_end"));
    }

    #[test]
    fn generates_shards_binding_len_overflow_guard() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(shards, len = 70000)]
                shards: Vec<arch_program::account::AccountInfo<'info>>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let (len_ts, elem_ts) = match &cfg.kind {
            crate::model::FieldKind::Shards(l, e) => (l, e),
            _ => panic!("expected shards kind"),
        };
        let ts = generate_shards_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            len_ts,
            elem_ts,
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("0xFFFF"));
        assert!(rendered.contains("InvalidAccountData"));
    }

    #[test]
    fn generates_shards_binding_underflow_guard() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(shards, len = 1)]
                shards: Vec<arch_program::account::AccountInfo<'info>>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let (len_ts, elem_ts) = match &cfg.kind {
            crate::model::FieldKind::Shards(l, e) => (l, e),
            _ => panic!("expected shards kind"),
        };
        let ts = generate_shards_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            len_ts,
            elem_ts,
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("NotEnoughAccountKeys"));
    }

    #[test]
    fn generates_shards_binding_signer_writable_flags() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(shards, len = 2, writable)]
                shards: Vec<arch_program::account::AccountInfo<'info>>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let (len_ts, elem_ts) = match &cfg.kind {
            crate::model::FieldKind::Shards(l, e) => (l, e),
            _ => panic!("expected shards kind"),
        };
        let ts = generate_shards_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(Some(true)),
            quote!(None),
            len_ts,
            elem_ts,
        );
        let rendered = ts.to_string();
        // Signer/writable flags should be forwarded into the account fetch helpers.
        // Accept both formatting variants produced by `TokenStream::to_string()`.
        assert!(rendered.contains("Some(true)") || rendered.contains("Some (true)"));
    }
}
