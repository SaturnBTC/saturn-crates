use proc_macro2::TokenStream;
use syn::{Error, FnArg, Item, ItemFn, ItemMod, PatType, Path, Type};

use super::helpers::has_cfg_test;

/// Information about each instruction handler function encountered inside the module.
#[derive(Clone)]
pub struct FnInfo {
    pub fn_ident: syn::Ident,
    pub acc_ty: Path,
    /// List of module identifiers starting at the root of the `#[saturn_program]`
    /// module and leading to this function (empty when the function is in the
    /// root). Used by the transform pass to disambiguate functions that share
    /// a name but are located in different nested modules.
    pub mod_path: Vec<syn::Ident>,
    /// Path of the *second* parameter type (the instruction payload) when it is a simple
    /// `Type::Path`. This is used later by the dispatcher generation step to decide whether
    /// the handler expects the **whole** instruction enum (unit variant) or the inner payload
    /// carried by tuple/struct variants. We store it as an `Option` because the parameter can
    /// legally be something else (e.g. a reference or more complex type) in which case the
    /// dispatcher falls back to the previous behaviour.
    pub second_param_ty: Option<syn::Path>,
}

/// Traverses the items of the inline module, collecting [`FnInfo`] values and
/// compile-error token streams.
///
/// Returns `(fn_infos, errors)`.
pub fn gather_fn_infos(item_mod: &ItemMod) -> (Vec<FnInfo>, Vec<TokenStream>) {
    let mut errors = Vec::<TokenStream>::new();
    let mut fn_infos = Vec::<FnInfo>::new();

    // Recursively walk inline modules to capture functions and their module path.
    fn walk_mod(
        current_mod: &ItemMod,
        mod_path: &mut Vec<syn::Ident>,
        fn_infos: &mut Vec<FnInfo>,
        errors: &mut Vec<TokenStream>,
    ) {
        // Safe unwrap: we only call with inline modules
        let items = match &current_mod.content {
            Some((_, items)) => items,
            None => return,
        };

        for inner_item in items {
            match inner_item {
                Item::Fn(item_fn) => {
                    let vis = &item_fn.vis;
                    let sig = &item_fn.sig;
                    let attrs = &item_fn.attrs;

                    if has_cfg_test(attrs) {
                        continue;
                    }

                    // Visibility check
                    if matches!(vis, syn::Visibility::Inherited) {
                        errors.push(
                            Error::new_spanned(
                                &sig.ident,
                                "handler must be public (e.g. `pub fn name(..)`); private handlers are not visible to the generated dispatcher",
                            )
                            .to_compile_error(),
                        );
                    }

                    // Must have exactly 2 parameters (ctx, params)
                    if sig.inputs.len() < 2 {
                        errors.push(
                            Error::new_spanned(
                                sig,
                                "handler must take (Context<'_, Accounts>, params)",
                            )
                            .to_compile_error(),
                        );
                        continue;
                    }

                    // First argument analysis (extract Accounts type)
                    let mut inputs_iter = sig.inputs.iter();
                    let first = inputs_iter.next().unwrap();

                    // Helper closure: given a `syn::Type::Path` that we already know refers to
                    // `Context< ... >`, extract the `Accounts` generic argument path (ignoring
                    // lifetimes & const generics) or push an error and return `None`.
                    let mut extract_accounts_ty = |type_path: &syn::TypePath| -> Option<Path> {
                        if let Some(seg) = type_path.path.segments.last() {
                            if let syn::PathArguments::AngleBracketed(gen_args) = &seg.arguments {
                                let type_args: Vec<&syn::Type> = gen_args
                                    .args
                                    .iter()
                                    .filter_map(|arg| match arg {
                                        syn::GenericArgument::Type(ty) => Some(ty),
                                        _ => None,
                                    })
                                    .collect();

                                let acc_ty_opt = type_args.iter().find_map(|ty| {
                                    if let syn::Type::Path(tp) = ty {
                                        let last_ident = tp.path.segments.last().map(|s| &s.ident);
                                        match last_ident {
                                            // Skip the injected TxBuilderWrapper generic when BTC is enabled.
                                            Some(ident) if ident == "TxBuilderWrapper" => None,
                                            _ => Some(tp.path.clone()),
                                        }
                                    } else {
                                        None
                                    }
                                });

                                if acc_ty_opt.is_none() {
                                    errors.push(
                                        Error::new_spanned(gen_args, "could not locate Accounts type parameter in Context<> generics").to_compile_error(),
                                    );
                                }
                                return acc_ty_opt;
                            } else {
                                errors.push(
                                    Error::new_spanned(seg, "Context must have generic parameters")
                                        .to_compile_error(),
                                );
                            }
                        }
                        None
                    };

                    let acc_ty_path_opt: Option<Path> = if let FnArg::Typed(PatType {
                        ty, ..
                    }) = first
                    {
                        match &**ty {
                            // ------------------------------
                            // Reject references – only bare `Context` is allowed
                            // ------------------------------
                            Type::Reference(ref ref_ty) => {
                                errors.push(
                                    Error::new_spanned(ref_ty, "first argument must be a bare Context value - references such as `&` or `&mut` are not allowed").to_compile_error(),
                                );
                                None
                            }

                            // ------------------------------
                            // Bare Context<'_, Acc>
                            // ------------------------------
                            Type::Path(type_path) => {
                                // Accept bare Context – the transform pass will rewrite it later.
                                if let Some(seg) = type_path.path.segments.last() {
                                    if seg.ident == "Context" {
                                        extract_accounts_ty(type_path)
                                    } else {
                                        errors.push(
                                            Error::new_spanned(
                                                seg,
                                                "first argument must be Context",
                                            )
                                            .to_compile_error(),
                                        );
                                        None
                                    }
                                } else {
                                    None
                                }
                            }

                            // ------------------------------
                            // Anything else is invalid
                            // ------------------------------
                            _ => {
                                errors.push(
                                    Error::new_spanned(
                                        ty,
                                        "first argument must be Context",
                                    )
                                    .to_compile_error(),
                                );
                                None
                            }
                        }
                    } else {
                        errors.push(
                            Error::new_spanned(first, "unexpected receiver; expected typed arg")
                                .to_compile_error(),
                        );
                        continue;
                    };

                    // ----------------------------------------------------
                    // Second argument becomes variant parameter – ensure it's a path type
                    // ----------------------------------------------------
                    let second = inputs_iter.next().unwrap();
                    if let FnArg::Typed(PatType { ty, .. }) = second {
                        match &**ty {
                            Type::Path(_tp) => { /* OK */ }
                            _ => {
                                errors.push(
                                    Error::new_spanned(ty, "parameter type must be a path")
                                        .to_compile_error(),
                                );
                                continue;
                            }
                        }
                    } else {
                        errors.push(
                            Error::new_spanned(second, "unexpected receiver; expected typed arg")
                                .to_compile_error(),
                        );
                        continue;
                    };

                    // Capture the *second* parameter type path (if it is a plain `Type::Path`).
                    let second_param_ty_opt: Option<syn::Path> =
                        if let FnArg::Typed(PatType { ty, .. }) = second {
                            match &**ty {
                                Type::Path(tp) => Some(tp.path.clone()),
                                _ => None,
                            }
                        } else {
                            None
                        };

                    if let Some(acc_ty) = acc_ty_path_opt {
                        fn_infos.push(FnInfo {
                            fn_ident: sig.ident.clone(),
                            acc_ty,
                            mod_path: mod_path.clone(),
                            second_param_ty: second_param_ty_opt,
                        });
                    }
                }
                Item::Mod(inner_mod) => {
                    // Only recurse into inline sub-modules
                    if inner_mod.content.is_some() {
                        mod_path.push(inner_mod.ident.clone());
                        walk_mod(inner_mod, mod_path, fn_infos, errors);
                        mod_path.pop();
                    }
                }
                _ => {}
            }
        }
    }

    let mut path = Vec::<syn::Ident>::new();
    walk_mod(item_mod, &mut path, &mut fn_infos, &mut errors);

    (fn_infos, errors)
}
