use crate::model::{FieldCfg, FieldKind};
use syn::{spanned::Spanned, Expr, Field};

/// Parse the `fields` of a struct annotated with `#[derive(Accounts)]` and
/// return a vector with one [`FieldCfg`] per field.
///
/// This function performs *syntactic* parsing and returns [`syn::Error`] for
/// syntax / attribute misuse.  Semantic, cross-field rules are handled later
/// by the `validator` module.
pub fn parse_fields(
    fields: &syn::punctuated::Punctuated<Field, syn::token::Comma>,
) -> Result<Vec<FieldCfg>, syn::Error> {
    let mut parsed_fields = Vec::<FieldCfg>::with_capacity(fields.len());

    for field in fields.iter() {
        let ident = field
            .ident
            .clone()
            .ok_or_else(|| syn::Error::new(field.span(), "Unnamed fields are not supported"))?;

        // Initialise the configuration with defaults.
        let mut cfg = FieldCfg {
            ident,
            is_signer: None,
            is_writable: None,
            address: None,
            seeds: None,
            program_id: None,
            payer: None,
            is_shards: false,
            kind: FieldKind::Single,
            is_zero_copy: false,
            is_init: false,
            is_realloc: false,
            is_init_if_needed: false,
            base_ty: syn::parse_quote! { () },
            space: None,
            of_type: None,
            owner: None,
        };

        // Detect if the field is a reference slice `&[AccountInfo<'_>]`.
        let mut is_slice_type = false;
        if let syn::Type::Reference(ref ref_ty) = &field.ty {
            if let syn::Type::Slice(_) = &*ref_ty.elem {
                is_slice_type = true;
            }
        }

        // Determine the underlying base type (strip reference if present)
        cfg.base_ty = match &field.ty {
            syn::Type::Reference(ref ref_ty) => (*ref_ty.elem).clone(),
            _ => field.ty.clone(),
        };

        let mut slice_len_expr: Option<Expr> = None;
        let mut has_account_attr = false;

        // Parse #[account(...)] attribute, if present.
        for attr in &field.attrs {
            if attr.path().is_ident("account") {
                has_account_attr = true;
                let result = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("signer") {
                        // Reject duplicated `signer` flag
                        if cfg.is_signer.is_some() {
                            return Err(meta.error("duplicate `signer` flag"));
                        }
                        cfg.is_signer = Some(true);
                    } else if meta.path.is_ident("mut") || meta.path.is_ident("writable") {
                        // Reject duplicated `writable` flag
                        if cfg.is_writable.is_some() {
                            return Err(meta.error("duplicate `writable` flag"));
                        }
                        cfg.is_writable = Some(true);
                    } else if meta.path.is_ident("address") {
                        let expr: Expr = meta.value()?.parse()?;
                        cfg.address = Some(expr);
                    } else if meta.path.is_ident("len") {
                        // Detect duplicate `len` attribute.
                        if slice_len_expr.is_some() {
                            return Err(meta.error("duplicate `len` attribute"));
                        }
                        let expr: Expr = meta.value()?.parse()?;
                        slice_len_expr = Some(expr);
                    } else if meta.path.is_ident("seeds") {
                        let expr: Expr = meta.value()?.parse()?;
                        cfg.seeds = Some(expr);
                    } else if meta.path.is_ident("program_id") {
                        let expr: Expr = meta.value()?.parse()?;
                        cfg.program_id = Some(expr);
                    } else if meta.path.is_ident("payer") {
                        let expr: Expr = meta.value()?.parse()?;
                        cfg.payer = Some(expr);
                    } else if meta.path.is_ident("owner") {
                        let expr: Expr = meta.value()?.parse()?;
                        if cfg.owner.is_some() {
                            return Err(meta.error("duplicate `owner` attribute"));
                        }
                        cfg.owner = Some(expr);
                    } else if meta.path.is_ident("shards") {
                        cfg.is_shards = true;
                    } else if meta.path.is_ident("of") {
                        let ty: syn::Type = meta.value()?.parse()?;
                        if cfg.of_type.is_some() {
                            return Err(meta.error("duplicate `of` attribute"));
                        }
                        cfg.of_type = Some(ty);
                    } else if meta.path.is_ident("zero_copy") {
                        cfg.is_zero_copy = true;
                    } else if meta.path.is_ident("init_if_needed") {
                        // Reject mixing both init and init_if_needed flags.
                        if cfg.is_init {
                            return Err(
                                meta.error("`init_if_needed` cannot be combined with `init`")
                            );
                        }
                        if cfg.is_init_if_needed {
                            return Err(meta.error("duplicate `init_if_needed` flag"));
                        }
                        cfg.is_init_if_needed = true;
                    } else if meta.path.is_ident("init") {
                        if cfg.is_init_if_needed {
                            return Err(
                                meta.error("`init` cannot be combined with `init_if_needed`")
                            );
                        }
                        cfg.is_init = true;
                    } else if meta.path.is_ident("space") {
                        let expr: Expr = meta.value()?.parse()?;
                        cfg.space = Some(expr);
                    } else if meta.path.is_ident("realloc") {
                        // Reject mixing with init / init_if_needed.
                        if cfg.is_init || cfg.is_init_if_needed {
                            return Err(meta.error(
                                "`realloc` cannot be combined with `init` or `init_if_needed`",
                            ));
                        }
                        if cfg.is_realloc {
                            return Err(meta.error("duplicate `realloc` flag"));
                        }
                        cfg.is_realloc = true;
                    } else if meta.path.is_ident("bump") {
                        // Mark this field as a bump placeholder. Validation will follow after attrs.
                        // Reject duplicate bump flag
                        if matches!(cfg.kind, FieldKind::Bump) {
                            return Err(meta.error("duplicate `bump` flag"));
                        }
                        cfg.kind = FieldKind::Bump;
                    } else {
                        return Err(meta.error("Unknown flag in #[account] attribute"));
                    }
                    Ok(())
                });

                if let Err(e) = result {
                    return Err(e);
                }
            }
        }

        // If the field does not have an #[account] attribute we only skip it when it's clearly
        // a marker field such as `PhantomData`. Otherwise we treat it as an account field and
        // apply the usual validation so that unsupported bare types (e.g., `u64`) are caught.
        if !has_account_attr {
            let is_phantom = is_phantom_marker(&cfg.base_ty);

            if is_phantom {
                // Treat marker fields as Phantom kind so codegen can initialise them explicitly.
                cfg.kind = FieldKind::Phantom;
                // No further account-specific validation required.
                parsed_fields.push(cfg);
                continue;
            }
        }

        // Provide default `program_id = crate::ID` whenever it is required
        // (e.g. PDA seeds, init / realloc creation, or bump placeholder) but
        // the user did not specify it explicitly.  This mirrors Anchor’s
        // behaviour and improves ergonomics: users can simply rely on
        // `crate::ID` without boiler-plate.
        let program_id_is_required = cfg.seeds.is_some()
            || cfg.is_init
            || cfg.is_init_if_needed
            || cfg.is_realloc
            || matches!(cfg.kind, FieldKind::Bump);

        if program_id_is_required && cfg.program_id.is_none() {
            cfg.program_id = Some(syn::parse_quote! { crate::ID });
        }

        // Reject mutually exclusive attribute combinations early.
        if cfg.seeds.is_some() && cfg.address.is_some() {
            return Err(syn::Error::new(
                field.span(),
                "`address` cannot be combined with `seeds`; the PDA address is derived from the provided seeds – remove the redundant `address` attribute.",
            ));
        }

        // Ensure that when `seeds = ...` are provided we also know which program owns the derived PDA.
        if cfg.seeds.is_some() && cfg.program_id.is_none() {
            return Err(syn::Error::new(
                field.span(),
                "`program_id = <id>` attribute is required whenever `seeds = ...` is specified",
            ));
        }

        // Conversely, specifying `program_id` without `seeds` is usually meaningless, **except**
        // when the field is marked `init`. In that case the `program_id` indicates the owner of
        // the newly created account and must be allowed even without PDA seeds.
        if cfg.program_id.is_some() && cfg.seeds.is_none() && !(cfg.is_init || cfg.is_realloc) {
            return Err(syn::Error::new(
                field.span(),
                "`program_id` attribute requires `seeds = ...` to be specified unless the field is also marked `init`",
            ));
        }

        // Additional validation for `init` accounts at parse-time (syntax-level).
        if cfg.is_init || cfg.is_init_if_needed {
            // Specifying a constant `address` together with account creation flags is nonsensical
            // because the address is derived (PDA) or implicitly chosen at runtime.  Reject this
            // early to avoid silently ignoring a user mistake.
            if cfg.address.is_some() {
                return Err(syn::Error::new(
                    field.span(),
                    "`address` cannot be combined with `init` or `init_if_needed`; the account address is determined by the creation logic",
                ));
            }

            if cfg.payer.is_none() {
                return Err(syn::Error::new(
                    field.span(),
                    "`init` field requires `payer = <account>` in #[account] attribute",
                ));
            }
            if cfg.program_id.is_none() {
                return Err(syn::Error::new(
                    field.span(),
                    "`init` field requires `program_id = <id>` in #[account] attribute to set the owner",
                ));
            }
        }

        // Additional validation for `realloc` accounts.
        if cfg.is_realloc {
            if cfg.space.is_none() {
                return Err(syn::Error::new(
                    field.span(),
                    "`realloc` field requires `space = <expr>` in #[account] attribute to set the new data length",
                ));
            }
        }

        // A shard vector **derived from a PDA** (has `seeds`) is signed for via program
        // derived address, therefore the individual shard accounts themselves cannot be
        // transaction signers.  For *non-PDA* shard vectors we *do* allow `signer` so the
        // account owner can authorise CPI such as `allocate`.
        if cfg.is_shards && cfg.seeds.is_some() && cfg.is_signer.unwrap_or(false) {
            return Err(syn::Error::new(
                field.span(),
                "`signer` attribute cannot be used on shard vectors that are PDAs (`seeds = ...`)",
            ));
        }

        // Detect `Vec<...>` type regardless of `shards` attribute.
        let is_vec_type = matches!(&cfg.base_ty, syn::Type::Path(type_path) if type_path.path.segments.last().map_or(false, |seg| seg.ident == "Vec"));

        // If it's a vector but **not** marked as `shards` we interpret it as a fixed-length slice
        // (older syntax used `&[AccountInfo]`). The user must still provide `len = N`.
        if is_vec_type && !cfg.is_shards {
            // Must provide len expression
            let len_expr = slice_len_expr.ok_or_else(|| {
                syn::Error::new(field.span(), "vector field requires `len = N` attribute")
            })?;

            // Extract the element type inside `Vec<...>` so that subsequent codegen knows the base type.
            let element_ty = if let syn::Type::Path(type_path) = &field.ty {
                if let Some(last) = type_path.path.segments.last() {
                    if last.ident == "Vec" {
                        if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                            if let Some(first_arg) = args.args.first() {
                                if let syn::GenericArgument::Type(inner_ty) = first_arg {
                                    match inner_ty {
                                        syn::Type::Reference(ref ref_ty) => (*ref_ty.elem).clone(),
                                        _ => inner_ty.clone(),
                                    }
                                } else {
                                    return Err(syn::Error::new_spanned(
                                        first_arg,
                                        "Expected a type parameter inside Vec<...>",
                                    ));
                                }
                            } else {
                                return Err(syn::Error::new_spanned(
                                    last,
                                    "Vec requires one type parameter",
                                ));
                            }
                        } else {
                            return Err(syn::Error::new_spanned(
                                last,
                                "Expected generic parameter for Vec",
                            ));
                        }
                    } else {
                        return Err(syn::Error::new_spanned(&field.ty, "Malformed Vec type"));
                    }
                } else {
                    return Err(syn::Error::new_spanned(
                        &field.ty,
                        "Malformed Vec type path",
                    ));
                }
            } else {
                return Err(syn::Error::new_spanned(
                    &field.ty,
                    "`len` attribute with Vec requires generic parameter",
                ));
            };

            cfg.kind = FieldKind::FixedSlice(quote::quote! { #len_expr });
            // Set base type to the element type so later detection (e.g., AccountInfo) works.
            cfg.base_ty = element_ty;
        } else if cfg.is_shards {
            // Must provide len expression for shard vectors.
            let len_expr = slice_len_expr.ok_or_else(|| {
                syn::Error::new(field.span(), "`shards` field requires `len = N` attribute")
            })?;

            // Deduce the element type inside the Vec<...>
            let element_ty = if let syn::Type::Path(type_path) = &field.ty {
                if let Some(last) = type_path.path.segments.last() {
                    if last.ident == "Vec" {
                        if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                            if let Some(first_arg) = args.args.first() {
                                if let syn::GenericArgument::Type(inner_ty) = first_arg {
                                    match inner_ty {
                                        syn::Type::Reference(ref ref_ty) => (*ref_ty.elem).clone(),
                                        _ => inner_ty.clone(),
                                    }
                                } else {
                                    return Err(syn::Error::new_spanned(
                                        first_arg,
                                        "Expected a type parameter inside Vec<...>",
                                    ));
                                }
                            } else {
                                return Err(syn::Error::new_spanned(
                                    last,
                                    "Vec requires one type parameter",
                                ));
                            }
                        } else {
                            return Err(syn::Error::new_spanned(
                                last,
                                "Expected generic parameter for Vec",
                            ));
                        }
                    } else {
                        return Err(syn::Error::new_spanned(
                            &field.ty,
                            "`shards` attribute must be used on Vec<...> field",
                        ));
                    }
                } else {
                    return Err(syn::Error::new_spanned(&field.ty, "Malformed type path"));
                }
            } else {
                return Err(syn::Error::new_spanned(
                    &field.ty,
                    "`shards` attribute must be used on Vec<...> field",
                ));
            };

            // If the element itself is a known wrapper (e.g., AccountLoader/ShardHandle),
            // extract its inner generic type so the logical element type can be compared against `of = ...`.
            let wrapper_idents = ["AccountLoader", "ShardHandle"];
            let unwrapped_element_ty = if let syn::Type::Path(type_path) = &element_ty {
                if let Some(last) = type_path.path.segments.last() {
                    if wrapper_idents.contains(&last.ident.to_string().as_str()) {
                        if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                            // Find the first generic argument that is a Type (skip lifetimes)
                            let mut extracted: Option<syn::Type> = None;
                            for generic_arg in &args.args {
                                if let syn::GenericArgument::Type(inner_ty) = generic_arg {
                                    extracted = Some(match inner_ty {
                                        syn::Type::Reference(ref ref_ty) => (*ref_ty.elem).clone(),
                                        _ => inner_ty.clone(),
                                    });
                                    break;
                                }
                            }
                            if let Some(t) = extracted {
                                t
                            } else {
                                element_ty.clone()
                            }
                        } else {
                            element_ty.clone()
                        }
                    } else {
                        element_ty.clone()
                    }
                } else {
                    element_ty.clone()
                }
            } else {
                element_ty.clone()
            };

            cfg.kind = FieldKind::Shards(
                quote::quote! { #len_expr },
                quote::quote! { #unwrapped_element_ty },
            );
            // Ensure base_ty is set to the logical element type so downstream checks work.
            cfg.base_ty = unwrapped_element_ty.clone();

            // Validate that `of = Type` (if provided) matches the element type we deduced.
            if let Some(ref of_ty) = cfg.of_type {
                let wanted = quote::quote! { #of_ty }.to_string().replace(' ', "");
                let actual = quote::quote! { #unwrapped_element_ty }
                    .to_string()
                    .replace(' ', "");
                if wanted != actual {
                    return Err(syn::Error::new(
                        field.span(),
                        format!(
                            "`of` type `{}` does not match element type `{}` of the Vec",
                            wanted, actual
                        ),
                    ));
                }
            }
        } else if is_slice_type {
            let len_expr = slice_len_expr.ok_or_else(|| {
                syn::Error::new(
                    field.span(),
                    "slice field requires `len = N` in #[account] attribute",
                )
            })?;
            cfg.kind = FieldKind::FixedSlice(quote::quote! { #len_expr });
        } else {
            if slice_len_expr.is_some() {
                return Err(syn::Error::new(
                    field.span(),
                    "`len = N` only valid on slice fields",
                ));
            }
        }

        // Validate single-account fields: allow `AccountInfo`, the two wrapper types, or their aliases.
        if matches!(cfg.kind, FieldKind::Single) {
            let allowed_idents = ["Account", "AccountLoader", "AccountInfo"];
            let is_allowed = match &cfg.base_ty {
                syn::Type::Path(type_path) => type_path.path.segments.last().map_or(false, |seg| {
                    allowed_idents.contains(&seg.ident.to_string().as_str())
                }),
                _ => false,
            };

            if !is_allowed {
                return Err(syn::Error::new(
                    field.span(),
                    "unsupported base type for account field; expected AccountInfo, Account, or AccountLoader",
                ));
            }

            let wrapper_idents = ["Account", "AccountLoader"];
            // Extract inner type for wrapper variants.
            if let syn::Type::Path(type_path) = &cfg.base_ty {
                if let Some(last) = type_path.path.segments.last() {
                    if wrapper_idents.contains(&last.ident.to_string().as_str()) {
                        if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                            for arg in &args.args {
                                if let syn::GenericArgument::Type(inner_ty) = arg {
                                    cfg.base_ty = match inner_ty {
                                        syn::Type::Reference(ref ref_ty) => (*ref_ty.elem).clone(),
                                        _ => inner_ty.clone(),
                                    };
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Additional checks specific to bump placeholder fields ---------------------------------

        if matches!(cfg.kind, FieldKind::Bump) {
            // Field must be either `u8`, a `[u8; 1]` array, or a reference to such array (e.g. `&'info [u8; 1]`).
            let is_u8_primitive = matches!(&cfg.base_ty, syn::Type::Path(tp)
                if tp.path.segments.last().map_or(false, |seg| seg.ident == "u8"));

            // Detect `[u8; 1]` array type.
            let is_u8_array1 = matches!(&cfg.base_ty, syn::Type::Array(arr)
                if matches!(&*arr.elem, syn::Type::Path(tp)
                    if tp.path.segments.last().map_or(false, |seg| seg.ident == "u8"))
                    && matches!(&arr.len, syn::Expr::Lit(lit)
                        if matches!(&lit.lit, syn::Lit::Int(int_lit) if int_lit.base10_parse::<u64>().map_or(false, |v| v == 1))));

            // Detect reference to `[u8; 1]` – any lifetime.
            let is_ref_u8_array1 = matches!(&cfg.base_ty, syn::Type::Reference(ref_ty)
                if matches!(&*ref_ty.elem, syn::Type::Array(arr)
                    if matches!(&*arr.elem, syn::Type::Path(tp)
                        if tp.path.segments.last().map_or(false, |seg| seg.ident == "u8"))
                        && matches!(&arr.len, syn::Expr::Lit(lit)
                            if matches!(&lit.lit, syn::Lit::Int(int_lit) if int_lit.base10_parse::<u64>().map_or(false, |v| v == 1)) )));

            if !(is_u8_primitive || is_u8_array1 || is_ref_u8_array1) {
                return Err(syn::Error::new(
                    field.span(),
                    "`bump` field must have type `u8`, `[u8; 1]`, or reference to `[u8; 1]`",
                ));
            }

            // Must provide seeds and program_id so we can derive the PDA and its bump.
            if cfg.seeds.is_none() {
                return Err(syn::Error::new(
                    field.span(),
                    "`bump` field requires `seeds = ...` attribute",
                ));
            }
            if cfg.program_id.is_none() {
                return Err(syn::Error::new(
                    field.span(),
                    "`bump` field requires `program_id = <id>` attribute",
                ));
            }

            // Bump fields cannot combine other account-specific flags.
            if cfg.is_signer.is_some()
                || cfg.is_writable.is_some()
                || cfg.is_init
                || cfg.is_zero_copy
                || cfg.is_shards
            {
                return Err(syn::Error::new(
                    field.span(),
                    "`bump` field cannot combine other account-specific flags",
                ));
            }

            // It is ok – push and continue (no account consumed).
            parsed_fields.push(cfg);
            continue;
        }

        parsed_fields.push(cfg);
    }

    Ok(parsed_fields)
}

/// Returns true if `ty` is exactly `core::marker::PhantomData<_>` or `std::marker::PhantomData<_>`.
/// This mirrors Anchor's behaviour where only those two canonical paths are treated as zero-sized
/// marker fields and therefore ignored by the macro. Any other type that merely ends with
/// `PhantomData` will be treated as a regular field and validated accordingly.
fn is_phantom_marker(ty: &syn::Type) -> bool {
    use syn::Type;

    let segments_equal = |segments: &[String]| -> bool {
        segments == ["core", "marker", "PhantomData"]
            || segments == ["std", "marker", "PhantomData"]
    };

    match ty {
        Type::Path(type_path) => {
            let segs: Vec<String> = type_path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            segments_equal(&segs)
        }
        _ => false,
    }
}
