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
    // Identifiers reused across PDA branches (defined early so all quotes can reference them).
    let seeds_ident = syn::Ident::new(&format!("__{}_seeds", ident), ident.span());
    let owner_ident = syn::Ident::new(&format!("__{}_owner", ident), ident.span());

    // Optional snippet that performs a SystemProgram allocate CPI (when growing) and then
    // updates the local slice via `realloc`. Handles both PDA and non-PDA cases.
    let realloc_snip: TokenStream = if cfg.is_realloc {
        let space_expr = cfg
            .space
            .as_ref()
            .expect("`space` is required for realloc")
            .clone();

        if cfg.seeds.is_some() {
            // PDA path – relies on `#seeds_ident`, `#owner_ident`, and loop variable `i`.
            quote! {
                let new_len: usize = (#space_expr) as usize;
                if new_len > acc_info_tmp.data_len() {
                    // Build signer seeds = base_seeds + idx_le + bump
                    let idx_le: [u8; 2] = (i as u16).to_le_bytes();

                    let mut seed_vec: Vec<&[u8]> = {
                        let mut v = Vec::with_capacity(#seeds_ident.len() + 2);
                        v.extend_from_slice(#seeds_ident);
                        v.push(&idx_le);
                        v
                    };

                    let (_expected, bump_seed) = arch_program::pubkey::Pubkey::find_program_address(&seed_vec, #owner_ident);
                    let bump_slice: &[u8] = &[bump_seed];
                    seed_vec.push(bump_slice);

                    let signer_seeds: &[&[&[u8]]] = &[&seed_vec];

                    arch_program::program::invoke_signed(
                        &arch_program::system_instruction::allocate(acc_info_tmp.key, new_len as u64),
                        &[acc_info_tmp.clone()],
                        signer_seeds,
                    )?;
                }

                if acc_info_tmp.data_len() != new_len {
                    acc_info_tmp.realloc(new_len, true)?;
                }
            }
        } else {
            // Non-PDA path – account signs directly.
            quote! {
                let new_len: usize = (#space_expr) as usize;
                if new_len > acc_info_tmp.data_len() {
                    arch_program::program::invoke(
                        &arch_program::system_instruction::allocate(acc_info_tmp.key, new_len as u64),
                        &[acc_info_tmp.clone()],
                    )?;
                }

                if acc_info_tmp.data_len() != new_len {
                    acc_info_tmp.realloc(new_len, true)?;
                }
            }
        }
    } else {
        TokenStream::new()
    };

    let is_acc_info_elem = crate::codegen::utils::is_account_info_path(&cfg.base_ty);

    // -------------------------------------------------------------------------------------
    // Detect whether we need initialisation logic for the shard accounts.  We only support
    // initialising **zero-copy** (AccountLoader) shard vectors; AccountInfo shard vectors
    // remain load-only.  The parser already guarantees that `payer` and `program_id` are
    // present whenever `init` / `init_if_needed` are set.
    // -------------------------------------------------------------------------------------
    let needs_init: bool = cfg.is_init || cfg.is_init_if_needed;

    // Token stream representing the compile-time boolean so we can embed it inside generated code.
    let needs_init_ts: proc_macro2::TokenStream = if needs_init {
        quote!(true)
    } else {
        quote!(false)
    };

    // Convert selected attributes into token streams for later interpolation. They will
    // only be referenced when `needs_init == true` so we unwrap safely.
    let payer_ts: proc_macro2::TokenStream = if needs_init {
        let payer_expr = cfg
            .payer
            .as_ref()
            .expect("`payer` is required when using `init` / `init_if_needed` on shard vectors");
        quote! { #payer_expr }
    } else {
        proc_macro2::TokenStream::new()
    };

    // Helper that converts the optional `space = ...` attribute into the final u64 expression.
    // Follows the same defaulting rules as single-account code-gen.
    let space_ts: proc_macro2::TokenStream = if let Some(space_expr) = &cfg.space {
        quote! { (#space_expr as u64) }
    } else if is_acc_info_elem {
        // For raw AccountInfo shards default to zero data – user must specify `space` if needed.
        quote! { 0u64 }
    } else {
        // Zero-copy shards default to the size of their inner type.
        quote! { core::mem::size_of::<#element_ty_ts>() as u64 }
    };

    // Owner token stream for account initialization (non-PDA path). Only needed when `needs_init` is true.
    let owner_ts: proc_macro2::TokenStream = if needs_init {
        let owner_expr = cfg
            .program_id
            .as_ref()
            .expect("`program_id` is required when using `init` / `init_if_needed`");
        quote! { #owner_expr }
    } else {
        proc_macro2::TokenStream::new()
    };

    // Compile-time branch that errors out if the account is already initialised and the strict `init` flag is used.
    let already_init_guard: proc_macro2::TokenStream = if cfg.is_init {
        quote! {
            return Err(arch_program::program_error::ProgramError::AccountAlreadyInitialized);
        }
    } else {
        proc_macro2::TokenStream::new()
    };

    // -------------------------------------------------------------------------------------------------------------
    // Pre-compute the snippets that perform **non-PDA** account creation / initialisation when `needs_init` is set.
    // These snippets are spliced directly into the `for` loop body; when `needs_init == false` they collapse to an
    // empty `TokenStream`, so no code referencing `owner_ts` is emitted at all.
    // -------------------------------------------------------------------------------------------------------------
    let init_account_snip: proc_macro2::TokenStream = if needs_init {
        quote! {
            let owner_expected = { #owner_ts }; // program_id verified by parser
            let already_initialised = acc_info_tmp.owner == &owner_expected;

            if !already_initialised {
                let space: u64 = #space_ts;
                let lamports: u64 = arch_program::account::MIN_ACCOUNT_LAMPORTS;

                let create_ix = arch_program::system_instruction::create_account(
                    saturn_account_parser::ToAccountInfo::to_account_info(&#payer_ts).key,
                    acc_info_tmp.key,
                    lamports,
                    space,
                    &owner_expected,
                );

                arch_program::program::invoke(
                    &create_ix,
                    &[saturn_account_parser::ToAccountInfo::to_account_info(&#payer_ts).clone(), acc_info_tmp.clone()],
                )?;
            } else {
                #already_init_guard
            }
        }
    } else {
        proc_macro2::TokenStream::new()
    };

    let loader_init_snip: proc_macro2::TokenStream = if needs_init {
        quote! {
            let owner_expected = { #owner_ts };
            let was_initialised = acc_info_tmp.owner == &owner_expected;
            if !was_initialised {
                loader.load_init()?;
            }
        }
    } else {
        proc_macro2::TokenStream::new()
    };

    // ---------------------------------------------------------------------
    // PDA-specific snippets – emitted only when `needs_init` is true so that
    // no `#payer_ts` token is generated otherwise.
    // ---------------------------------------------------------------------
    let pda_init_account_snip: proc_macro2::TokenStream = if needs_init {
        quote! {
            // -------------------------------- Initialization --------------------------------
            let already_initialised = acc_info_tmp.owner == #owner_ident;
            if !already_initialised {
                let space: u64 = #space_ts;
                let lamports: u64 = arch_program::account::MIN_ACCOUNT_LAMPORTS;

                // signer seeds = base + idx_le + bump
                let idx_le: [u8; 2] = (i as u16).to_le_bytes();
                let mut seed_vec: Vec<&[u8]> = {
                    let mut v = Vec::with_capacity(#seeds_ident.len() + 2);
                    v.extend_from_slice(#seeds_ident);
                    v.push(&idx_le);
                    v
                };
                let (_expected, bump_seed) = arch_program::pubkey::Pubkey::find_program_address(&seed_vec, #owner_ident);
                let bump_slice: &[u8] = &[bump_seed];
                seed_vec.push(bump_slice);
                let signer_seeds: &[&[&[u8]]] = &[&seed_vec];

                let create_ix = arch_program::system_instruction::create_account(
                    saturn_account_parser::ToAccountInfo::to_account_info(&#payer_ts).key,
                    acc_info_tmp.key,
                    lamports,
                    space,
                    #owner_ident,
                );

                arch_program::program::invoke_signed(
                    &create_ix,
                    &[saturn_account_parser::ToAccountInfo::to_account_info(&#payer_ts).clone(), acc_info_tmp.clone()],
                    signer_seeds,
                )?;
            } else {
                #already_init_guard
            }
        }
    } else {
        proc_macro2::TokenStream::new()
    };

    let pda_loader_init_snip: proc_macro2::TokenStream = if needs_init {
        quote! {
            let already_initialised = acc_info_tmp.owner == #owner_ident;
            if !already_initialised {
                loader.load_init()?;
            }
        }
    } else {
        proc_macro2::TokenStream::new()
    };

    if cfg.seeds.is_some() {
        let seeds_expr = cfg.seeds.as_ref().unwrap();
        let program_id_expr = cfg
            .program_id
            .as_ref()
            .expect("program_id required when seeds provided");

        // Pre-compute constant PDA references once per field to avoid recomputation in each loop
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

                let mut vec_tmp: Vec<saturn_account_parser::codec::AccountLoader<'_, #element_ty_ts>> = Vec::with_capacity(len_val);
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

                    #pda_init_account_snip

                    #realloc_snip

                    let loader = saturn_account_parser::codec::AccountLoader::<#element_ty_ts>::new(acc_info_tmp);
                    #pda_loader_init_snip
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

                let mut vec_tmp: Vec<saturn_account_parser::codec::AccountLoader<'_, #element_ty_ts>> = Vec::with_capacity(len_val);
                for i in 0..len_val {
                    let acc_info_tmp = saturn_account_parser::get_account(
                        accounts,
                        slice_start + i,
                        #signer_tok,
                        #writable_tok,
                        #address_tok,
                    )?;

                    // ----------------------------- optional initialisation -----------------------------
                    #init_account_snip

                    #realloc_snip

                    let loader = saturn_account_parser::codec::AccountLoader::<#element_ty_ts>::new(acc_info_tmp);

                    // Optional zero-copy struct initialisation
                    #loader_init_snip

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
        assert!(rendered.contains("AccountLoader"));
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
        assert!(rendered.contains("AccountLoader"));
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
