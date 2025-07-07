use super::utils::is_account_info_path;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::model::FieldCfg;

// Re-exported so the parent orchestrator can call it directly.
pub(crate) fn generate_single_binding(
    cfg: &FieldCfg,
    ident: &Ident,
    signer_tok: TokenStream,
    writable_tok: TokenStream,
    address_tok: TokenStream,
    payer_tok_opt: Option<TokenStream>,
    owner_tok_opt: Option<TokenStream>,
) -> TokenStream {
    let fetch_account = if cfg.seeds.is_some() {
        let seeds_expr = cfg.seeds.as_ref().unwrap();
        let program_id_expr = cfg
            .program_id
            .as_ref()
            .expect("program_id required when seeds provided");
        quote! {
            saturn_account_parser::get_pda_account(
                accounts,
                idx,
                #signer_tok,
                #writable_tok,
                #seeds_expr,
                &#program_id_expr,
            )?
        }
    } else {
        quote! {
            saturn_account_parser::get_account(
                accounts,
                idx,
                #signer_tok,
                #writable_tok,
                #address_tok,
            )?
        }
    };

    let inner_ty_ts = {
        let ty_inner = &cfg.base_ty;
        quote! { #ty_inner }
    };

    // Determine allocation size
    let space_ts = if let Some(space_expr) = &cfg.space {
        quote! { (#space_expr as u64) }
    } else {
        quote! { core::mem::size_of::<#inner_ty_ts>() as u64 }
    };

    // Treat both `init` and `init_if_needed` as requiring initialisation logic.
    let is_any_init = cfg.is_init || cfg.is_init_if_needed;

    if cfg.is_realloc {
        generate_single_realloc(
            cfg,
            ident,
            fetch_account,
            inner_ty_ts,
            space_ts,
            payer_tok_opt,
        )
    } else if cfg.is_zero_copy {
        generate_single_zero_copy(
            cfg,
            ident,
            fetch_account,
            inner_ty_ts,
            space_ts,
            payer_tok_opt,
            owner_tok_opt,
        )
    } else if is_any_init {
        generate_single_borsh_init(
            cfg,
            ident,
            fetch_account,
            inner_ty_ts,
            space_ts,
            payer_tok_opt,
            owner_tok_opt,
        )
    } else {
        generate_single_default(
            cfg,
            ident,
            fetch_account,
            inner_ty_ts,
            signer_tok,
            writable_tok,
            address_tok,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_single_zero_copy(
    cfg: &FieldCfg,
    ident: &Ident,
    fetch_account: TokenStream,
    inner_ty_ts: TokenStream,
    space_ts: TokenStream,
    payer_tok_opt: Option<TokenStream>,
    owner_tok_opt: Option<TokenStream>,
) -> TokenStream {
    if cfg.is_init || cfg.is_init_if_needed {
        let payer_expr = payer_tok_opt.as_ref().expect("payer required");
        let owner_expr = owner_tok_opt.as_ref().expect("program_id required");

        let already_init_guard = if cfg.is_init {
            quote! {
                return Err(arch_program::program_error::ProgramError::AccountAlreadyInitialized);
            }
        } else {
            quote! {}
        };

        if cfg.seeds.is_some() {
            // ---------------- PDA + zero-copy + init ----------------
            let seeds_expr = cfg.seeds.as_ref().unwrap();
            let program_id_expr = cfg.program_id.as_ref().unwrap();
            quote! {
                let acc_info_tmp = { #fetch_account };
                idx += 1;

                let already_initialised = *acc_info_tmp.owner == #owner_expr;

                if !already_initialised {
                    let space: u64 = #space_ts;
                    let lamports: u64 = arch_program::account::MIN_ACCOUNT_LAMPORTS;
                    let create_ix = arch_program::system_instruction::create_account(
                        #payer_expr.key,
                        acc_info_tmp.key,
                        lamports,
                        space,
                        &#owner_expr,
                    );

                    // Build signer seeds (base + bump)
                    let base_seeds: &[&[u8]] = #seeds_expr;
                    let (_expected, bump_seed) = arch_program::pubkey::Pubkey::find_program_address(base_seeds, &#program_id_expr);
                    let bump_seed_slice: &[u8] = &[bump_seed];
                    let signer_seeds_vec: Vec<&[u8]> = {
                        let mut v = Vec::with_capacity(base_seeds.len() + 1);
                        v.extend_from_slice(base_seeds);
                        v.push(bump_seed_slice);
                        v
                    };
                    let signer_seeds: &[&[&[u8]]] = &[&signer_seeds_vec];

                    arch_program::program::invoke_signed(
                        &create_ix,
                        &[#payer_expr.clone(), acc_info_tmp.clone()],
                        signer_seeds,
                    )?;
                }

                // Decide whether to error (init) or continue (init_if_needed)
                if already_initialised {
                    #already_init_guard
                }

                let loader = saturn_account_parser::codec::ZeroCopyAccount::<#inner_ty_ts>::new(acc_info_tmp);
                if !already_initialised {
                    loader.load_init()?;
                }
                let #ident = loader;
            }
        } else {
            // ---------------- Non-PDA + zero-copy + init ----------------
            quote! {
                let acc_info_tmp = { #fetch_account };
                idx += 1;

                let already_initialised = *acc_info_tmp.owner == #owner_expr;

                if !already_initialised {
                    let space: u64 = #space_ts;
                    let lamports: u64 = arch_program::account::MIN_ACCOUNT_LAMPORTS;
                    let create_ix = arch_program::system_instruction::create_account(
                        #payer_expr.key,
                        acc_info_tmp.key,
                        lamports,
                        space,
                        &#owner_expr,
                    );

                    arch_program::program::invoke(
                        &create_ix,
                        &[#payer_expr.clone(), acc_info_tmp.clone()],
                    )?;
                }

                // Decide whether to error (init) or continue (init_if_needed)
                if already_initialised {
                    #already_init_guard
                }

                let loader = saturn_account_parser::codec::ZeroCopyAccount::<#inner_ty_ts>::new(acc_info_tmp);
                if !already_initialised {
                    loader.load_init()?;
                }
                let #ident = loader;
            }
        }
    } else {
        // ---------------- zero-copy (no init) ----------------
        quote! {
            let acc_info_tmp = { #fetch_account };
            idx += 1;
            let #ident = saturn_account_parser::codec::ZeroCopyAccount::<#inner_ty_ts>::new(acc_info_tmp);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_single_borsh_init(
    cfg: &FieldCfg,
    ident: &Ident,
    fetch_account: TokenStream,
    inner_ty_ts: TokenStream,
    space_ts: TokenStream,
    payer_tok_opt: Option<TokenStream>,
    owner_tok_opt: Option<TokenStream>,
) -> TokenStream {
    let payer_expr = payer_tok_opt.as_ref().expect("payer required");
    let owner_expr = owner_tok_opt.as_ref().expect("program_id required");

    let already_init_guard = if cfg.is_init {
        quote! {
            return Err(arch_program::program_error::ProgramError::AccountAlreadyInitialized);
        }
    } else {
        quote! {}
    };

    if cfg.seeds.is_some() {
        // ---------------- PDA + Borsh + init ----------------
        let seeds_expr = cfg.seeds.as_ref().unwrap();
        let program_id_expr = cfg.program_id.as_ref().unwrap();
        quote! {
            let acc_info_tmp = { #fetch_account };
            idx += 1;

            let already_initialised = *acc_info_tmp.owner == #owner_expr;

            if !already_initialised {
                let space: u64 = #space_ts;
                let lamports: u64 = arch_program::account::MIN_ACCOUNT_LAMPORTS;
                let create_ix = arch_program::system_instruction::create_account(
                    #payer_expr.key,
                    acc_info_tmp.key,
                    lamports,
                    space,
                    &#owner_expr,
                );

                let base_seeds: &[&[u8]] = #seeds_expr;
                let (_expected, bump_seed) = arch_program::pubkey::Pubkey::find_program_address(base_seeds, &#program_id_expr);
                let bump_seed_slice: &[u8] = &[bump_seed];
                let signer_seeds_vec: Vec<&[u8]> = {
                    let mut v = Vec::with_capacity(base_seeds.len() + 1);
                    v.extend_from_slice(base_seeds);
                    v.push(bump_seed_slice);
                    v
                };
                let signer_seeds: &[&[&[u8]]] = &[&signer_seeds_vec];

                arch_program::program::invoke_signed(
                    &create_ix,
                    &[#payer_expr.clone(), acc_info_tmp.clone()],
                    signer_seeds,
                )?;
            }

            if already_initialised {
                #already_init_guard
            }
            let #ident = if already_initialised {
                saturn_account_parser::codec::BorshAccount::<#inner_ty_ts>::load(acc_info_tmp)?
            } else {
                saturn_account_parser::codec::BorshAccount::<#inner_ty_ts>::init(acc_info_tmp)?
            };
        }
    } else {
        // ---------------- Non-PDA + Borsh + init ----------------
        quote! {
            let acc_info_tmp = { #fetch_account };
            idx += 1;

            let already_initialised = *acc_info_tmp.owner == #owner_expr;

            if !already_initialised {
                let space: u64 = #space_ts;
                let lamports: u64 = arch_program::account::MIN_ACCOUNT_LAMPORTS;
                let create_ix = arch_program::system_instruction::create_account(
                    #payer_expr.key,
                    acc_info_tmp.key,
                    lamports,
                    space,
                    &#owner_expr,
                );

                arch_program::program::invoke(
                    &create_ix,
                    &[#payer_expr.clone(), acc_info_tmp.clone()],
                )?;
            }

            if already_initialised {
                #already_init_guard
            }
            let #ident = if already_initialised {
                saturn_account_parser::codec::BorshAccount::<#inner_ty_ts>::load(acc_info_tmp)?
            } else {
                saturn_account_parser::codec::BorshAccount::<#inner_ty_ts>::init(acc_info_tmp)?
            };
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_single_realloc(
    cfg: &FieldCfg,
    ident: &Ident,
    _fetch_account: TokenStream,
    inner_ty_ts: TokenStream,
    space_ts: TokenStream,
    payer_tok_opt: Option<TokenStream>,
) -> TokenStream {
    let _payer_expr = payer_tok_opt.as_ref().expect("payer required for realloc");

    // Realloc path differs for zero copy vs borsh/accountinfo types.
    if cfg.is_zero_copy {
        quote! {
            let acc_info_tmp = { #_fetch_account };
            idx += 1;

            let new_len: usize = #space_ts as usize;
            if acc_info_tmp.data_len() != new_len {
                acc_info_tmp.realloc(new_len, true)?;
            }

            let #ident = {
                let loader = saturn_account_parser::codec::ZeroCopyAccount::<#inner_ty_ts>::new(acc_info_tmp);
                loader
            };
        }
    } else {
        // Borsh path
        quote! {
            let acc_info_tmp = { #_fetch_account };
            idx += 1;

            let new_len: usize = #space_ts as usize;
            if acc_info_tmp.data_len() != new_len {
                acc_info_tmp.realloc(new_len, true)?;
            }

            let #ident = saturn_account_parser::codec::BorshAccount::<#inner_ty_ts>::load(acc_info_tmp)?;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_single_default(
    cfg: &FieldCfg,
    ident: &Ident,
    fetch_account: TokenStream,
    inner_ty_ts: TokenStream,
    signer_tok: TokenStream,
    writable_tok: TokenStream,
    address_tok: TokenStream,
) -> TokenStream {
    // Detect if type is AccountInfo path.
    let is_acc_info_ty = is_account_info_path(&cfg.base_ty);

    if is_acc_info_ty {
        if cfg.seeds.is_some() {
            let seeds_expr = cfg.seeds.as_ref().unwrap();
            let program_id_expr = cfg
                .program_id
                .as_ref()
                .expect("program_id required when seeds provided");
            quote! {
                let acc_info_tmp = saturn_account_parser::get_pda_account(
                    accounts,
                    idx,
                    #signer_tok,
                    #writable_tok,
                    #seeds_expr,
                    &#program_id_expr,
                )?;
                idx += 1;
                // Return the account **by value** (clone) so the user can declare `AccountInfo<'info>` directly.
                let #ident: #inner_ty_ts = (*acc_info_tmp).clone();
            }
        } else {
            quote! {
                let acc_info_tmp = saturn_account_parser::get_account(
                    accounts,
                    idx,
                    #signer_tok,
                    #writable_tok,
                    #address_tok,
                )?;
                idx += 1;
                let #ident: #inner_ty_ts = (*acc_info_tmp).clone();
            }
        }
    } else {
        // Borsh default: fetch account and deserialize
        let fetch_tok = if cfg.seeds.is_some() {
            let seeds_expr = cfg.seeds.as_ref().unwrap();
            let program_id_expr = cfg
                .program_id
                .as_ref()
                .expect("program_id required when seeds provided");
            quote! {
                saturn_account_parser::get_pda_account(
                    accounts,
                    idx,
                    #signer_tok,
                    #writable_tok,
                    #seeds_expr,
                    &#program_id_expr,
                )?
            }
        } else {
            quote! {
                saturn_account_parser::get_account(
                    accounts,
                    idx,
                    #signer_tok,
                    #writable_tok,
                    #address_tok,
                )?
            }
        };

        quote! {
            let acc_info_tmp = { #fetch_tok };
            idx += 1;
            let #ident = saturn_account_parser::codec::BorshAccount::<#inner_ty_ts>::load(acc_info_tmp)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::{parse_quote, Data, DeriveInput, Fields};

    // Helper to extract named fields from DeriveInput
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
    fn generates_default_account_path() {
        // A minimal single AccountInfo field without special flags.
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                caller: arch_program::account::AccountInfo<'info>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];

        let ts = generate_single_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            None,
            None,
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("get_account"));
    }

    #[test]
    fn generates_zero_copy_init_path() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(zero_copy, init, payer = payer, program_id = arch_program::pubkey::Pubkey::default())]
                data: saturn_account_parser::codec::ZeroCopyAccount<'info, u64>,
                #[account(signer)]
                payer: arch_program::account::AccountInfo<'info>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let ts = generate_single_binding(
            cfg,
            &cfg.ident,
            quote!(Some(true)),
            quote!(None),
            quote!(None),
            Some(quote!(payer)),
            Some(quote!(arch_program::pubkey::Pubkey::default())),
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("ZeroCopyAccount"));
        assert!(rendered.contains("load_init"));
    }

    #[test]
    fn generates_zero_copy_init_pda_path() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(zero_copy, init, payer = payer, program_id = arch_program::pubkey::Pubkey::default(), seeds = &[b"seed"])]
                data: saturn_account_parser::codec::ZeroCopyAccount<'info, u64>,
                #[account(signer)]
                payer: arch_program::account::AccountInfo<'info>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let ts = generate_single_binding(
            cfg,
            &cfg.ident,
            quote!(None), // signer
            quote!(None), // writable
            quote!(None), // address override
            Some(quote!(payer)),
            Some(quote!(arch_program::pubkey::Pubkey::default())),
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("invoke_signed"));
        assert!(rendered.contains("ZeroCopyAccount"));
        assert!(rendered.contains("load_init"));
    }

    #[test]
    fn generates_zero_copy_no_init_path() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(zero_copy)]
                data: saturn_account_parser::codec::ZeroCopyAccount<'info, u64>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let ts = generate_single_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            None,
            None,
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("ZeroCopyAccount"));
        // Ensure we do NOT eagerly call load_init
        assert!(!rendered.contains("load_init"));
    }

    #[test]
    fn generates_borsh_init_pda_path() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(init, payer = payer, program_id = arch_program::pubkey::Pubkey::default(), seeds = &[b"seed"])]
                data: saturn_account_parser::codec::BorshAccount<'info, u64>,
                #[account(signer)]
                payer: arch_program::account::AccountInfo<'info>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let ts = generate_single_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            Some(quote!(payer)),
            Some(quote!(arch_program::pubkey::Pubkey::default())),
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("invoke_signed"));
        assert!(rendered.contains("BorshAccount"));
        assert!(rendered.contains("init"));
    }

    #[test]
    fn generates_borsh_init_non_pda_path() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(init, payer = payer, program_id = arch_program::pubkey::Pubkey::default())]
                data: saturn_account_parser::codec::BorshAccount<'info, u64>,
                #[account(signer)]
                payer: arch_program::account::AccountInfo<'info>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let ts = generate_single_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            Some(quote!(payer)),
            Some(quote!(arch_program::pubkey::Pubkey::default())),
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("invoke(") || rendered.contains("invoke ("));
        assert!(rendered.contains("BorshAccount"));
        assert!(rendered.contains("init"));
    }

    #[test]
    fn generates_account_info_pda_path() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
                pda_ai: arch_program::account::AccountInfo<'info>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let ts = generate_single_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            None,
            None,
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("get_pda_account"));
    }

    #[test]
    fn generates_borsh_load_non_pda_path() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                data: saturn_account_parser::codec::BorshAccount<'info, u64>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let ts = generate_single_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            None,
            None,
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("get_account"));
        assert!(rendered.contains("BorshAccount"));
        assert!(rendered.contains("load"));
    }

    #[test]
    fn generates_borsh_load_pda_path() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
                data: saturn_account_parser::codec::BorshAccount<'info, u64>,
            }
        };
        let parsed = crate::parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &parsed[0];
        let ts = generate_single_binding(
            cfg,
            &cfg.ident,
            quote!(None),
            quote!(None),
            quote!(None),
            None,
            None,
        );
        let rendered = ts.to_string();
        assert!(rendered.contains("get_pda_account"));
        assert!(rendered.contains("BorshAccount"));
        assert!(rendered.contains("load"));
    }

    // ---------------- Negative unit tests (parser-level validation) ----------------
    #[test]
    fn init_without_payer_validation_error() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(init, program_id = arch_program::pubkey::Pubkey::default())]
                data: saturn_account_parser::codec::BorshAccount<'info, u64>,
            }
        };
        let result = crate::parser::parse_fields(extract_named_fields(&di));
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("payer"));
    }
}
