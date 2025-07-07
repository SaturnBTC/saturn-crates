use proc_macro2::TokenStream;
use syn::{Error, FnArg, Item, ItemFn, ItemMod, PatType, Path, Type};

use super::helpers::has_cfg_test;

/// Information about each instruction handler function encountered inside the module.
#[derive(Clone)]
pub struct FnInfo {
    pub fn_ident: syn::Ident,
    pub acc_ty: Path,
}

/// Traverses the items of the inline module, collecting [`FnInfo`] values and
/// compile-error token streams.
///
/// Returns `(fn_infos, errors)`.
pub fn gather_fn_infos(item_mod: &ItemMod) -> (Vec<FnInfo>, Vec<TokenStream>) {
    let mut errors = Vec::<TokenStream>::new();
    let mut fn_infos = Vec::<FnInfo>::new();

    let (_brace, items) = match &item_mod.content {
        Some((brace, items)) => (brace, items),
        None => unreachable!("parse_item_mod already verified inline module"),
    };

    for inner_item in items.iter() {
        if let Item::Fn(ItemFn { sig, attrs, .. }) = inner_item {
            // Skip handlers that are only compiled when `cfg(test)` is active.
            if has_cfg_test(attrs) {
                continue;
            }

            let fn_ident = sig.ident.clone();

            // Expect at least 2 arguments
            if sig.inputs.len() < 2 {
                errors.push(
                    Error::new_spanned(
                        sig,
                        "handler must take (&mut Context<'_, Accounts>, params)",
                    )
                    .to_compile_error(),
                );
                continue;
            }

            // ----------------------------------------------------
            // First argument: &mut Context<'_, Accounts>
            // ----------------------------------------------------
            let mut inputs_iter = sig.inputs.iter();
            let first = inputs_iter.next().unwrap();
            let acc_ty_path_opt: Option<Path> = if let FnArg::Typed(PatType { ty, .. }) = first {
                match &**ty {
                    Type::Reference(ref ref_ty) => {
                        if let Type::Path(type_path) = &*ref_ty.elem {
                            if let Some(seg) = type_path.path.segments.last() {
                                if seg.ident == "Context" {
                                    if let syn::PathArguments::AngleBracketed(gen_args) =
                                        &seg.arguments
                                    {
                                        // Collect *type* generic arguments (ignore lifetimes / consts).
                                        let type_args: Vec<&syn::Type> = gen_args
                                            .args
                                            .iter()
                                            .filter_map(|arg| match arg {
                                                syn::GenericArgument::Type(ty) => Some(ty),
                                                _ => None,
                                            })
                                            .collect();

                                        let acc_ty_opt: Option<Path> = if type_args.is_empty() {
                                            errors.push(Error::new_spanned(gen_args, "`Context` must have a generic parameter for the accounts struct").to_compile_error());
                                            None
                                        } else {
                                            // Start from the last type argument.
                                            let mut idx = type_args.len() - 1;

                                            // If the last argument is a `TxBuilderWrapper<..>` then use the one before it.
                                            if let syn::Type::Path(tp) = type_args[idx] {
                                                if tp
                                                    .path
                                                    .segments
                                                    .last()
                                                    .map(|s| s.ident == "TxBuilderWrapper")
                                                    .unwrap_or(false)
                                                    && idx > 0
                                                {
                                                    idx -= 1;
                                                }
                                            }

                                            match type_args[idx] {
                                                syn::Type::Path(acc_path) => {
                                                    Some(acc_path.path.clone())
                                                }
                                                other => {
                                                    errors.push(Error::new_spanned(other, "expected accounts type parameter to be a path").to_compile_error());
                                                    None
                                                }
                                            }
                                        };

                                        acc_ty_opt
                                    } else {
                                        errors.push(
                                            Error::new_spanned(
                                                seg,
                                                "Context must have generic parameters",
                                            )
                                            .to_compile_error(),
                                        );
                                        None
                                    }
                                } else {
                                    errors.push(
                                        Error::new_spanned(
                                            seg,
                                            "first argument must be &mut Context<'_, Accounts>",
                                        )
                                        .to_compile_error(),
                                    );
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            errors.push(
                                Error::new_spanned(
                                    ref_ty.elem.as_ref(),
                                    "first arg inner type must be Context",
                                )
                                .to_compile_error(),
                            );
                            None
                        }
                    }
                    _ => {
                        errors.push(
                            Error::new_spanned(ty, "first argument must be &mut Context")
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

            if let Some(acc_ty) = acc_ty_path_opt {
                fn_infos.push(FnInfo { fn_ident, acc_ty });
            }
        }
    }

    (fn_infos, errors)
}
