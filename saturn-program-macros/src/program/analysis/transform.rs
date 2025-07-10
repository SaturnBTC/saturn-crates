use proc_macro2::Span;
use quote::ToTokens;
use syn::parse_quote;
use syn::visit::Visit;
use syn::{FnArg, GenericParam, Item, ItemMod, LitInt, Type};

use crate::program::attr::AttrConfig;

use super::gather::FnInfo;
use super::helpers::has_cfg_test;

/// Rewrites the first parameter type of each handler function to the fully-qualified
/// `saturn_account_parser::Context<..>` path so users can write simply `Context` in
/// their source code without relying on injected type aliases.
pub fn rewrite_context_params(item_mod: &mut ItemMod, fn_infos: &[FnInfo], attr_cfg: &AttrConfig) {
    // Helper that performs the recursive parameter rewrite.
    fn rewrite_in_mod(
        current_mod: &mut ItemMod,
        mod_path: &mut Vec<syn::Ident>,
        fn_infos: &[FnInfo],
        attr_cfg: &AttrConfig,
    ) {
        if let Some((_brace, ref mut items)) = current_mod.content {
            for inner in items.iter_mut() {
                match inner {
                    Item::Fn(fn_item) => {
                        if has_cfg_test(&fn_item.attrs) {
                            continue;
                        }

                        // locate fn_info by matching ident and mod_path
                        let maybe_info = fn_infos.iter().find(|info| {
                            info.fn_ident == fn_item.sig.ident && info.mod_path == *mod_path
                        });
                        let acc_ty_path = match maybe_info {
                            Some(info) => &info.acc_ty,
                            None => continue,
                        };

                        let new_param_ty: Type = if attr_cfg.enable_bitcoin_tx {
                            let max_inputs = LitInt::new(
                                &attr_cfg.btc_tx_cfg.max_inputs_to_sign.unwrap().to_string(),
                                Span::call_site(),
                            );
                            let max_modified = LitInt::new(
                                &attr_cfg
                                    .btc_tx_cfg
                                    .max_modified_accounts
                                    .unwrap()
                                    .to_string(),
                                Span::call_site(),
                            );
                            let rune_set_path = attr_cfg.btc_tx_cfg.rune_set.clone().unwrap();

                            parse_quote! {
                                saturn_account_parser::Context<
                                    '_, '_, '_, 'info,
                                    #acc_ty_path,
                                    saturn_account_parser::TxBuilderWrapper<'info, #max_modified, #max_inputs, #rune_set_path>
                                >
                            }
                        } else {
                            parse_quote! {
                                saturn_account_parser::Context<
                                    '_, '_, '_, 'info,
                                    #acc_ty_path
                                >
                            }
                        };

                        // -------------------------------------------------------------
                        // Detect if the `'info` lifetime is already in scope either as
                        // an explicit generic parameter or used anywhere inside the
                        // `where`-clause.  We switch from the previous token-stream
                        // scan to a proper AST walk (mirroring Anchor’s approach) to
                        // avoid false positives coming from docs/attributes.
                        // -------------------------------------------------------------
                        let lifetime_in_generics = fn_item.sig.generics.params.iter().any(|gp| {
                            matches!(gp, GenericParam::Lifetime(lt) if lt.lifetime.ident == "info")
                        });

                        // Visitor that searches for a specific lifetime inside a syntax tree.
                        struct FindLifetime<'a> {
                            target: &'a str,
                            found: bool,
                        }

                        impl<'ast, 'a> Visit<'ast> for FindLifetime<'a> {
                            fn visit_lifetime(&mut self, lt: &'ast syn::Lifetime) {
                                if lt.ident == self.target {
                                    self.found = true;
                                }
                            }
                        }

                        let lifetime_in_where = fn_item
                            .sig
                            .generics
                            .where_clause
                            .as_ref()
                            .map(|wc| {
                                let mut visitor = FindLifetime {
                                    target: "info",
                                    found: false,
                                };
                                visitor.visit_where_clause(wc);
                                visitor.found
                            })
                            .unwrap_or(false);

                        let has_info_lifetime = lifetime_in_generics || lifetime_in_where;

                        if !has_info_lifetime {
                            fn_item.sig.generics.params.push(syn::parse_quote!('info));
                        }

                        if let Some(first_arg) = fn_item.sig.inputs.first_mut() {
                            if let FnArg::Typed(pat_ty) = first_arg {
                                pat_ty.ty = Box::new(new_param_ty);
                            }
                        }
                    }
                    Item::Mod(inner_mod) => {
                        if inner_mod.content.is_some() {
                            mod_path.push(inner_mod.ident.clone());
                            rewrite_in_mod(inner_mod, mod_path, fn_infos, attr_cfg);
                            mod_path.pop();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let mut path = Vec::<syn::Ident>::new();
    rewrite_in_mod(item_mod, &mut path, fn_infos, attr_cfg);
}
