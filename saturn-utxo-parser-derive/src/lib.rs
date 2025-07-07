use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;
use syn::{parse_macro_input, spanned::Spanned, Data, DeriveInput, Expr, Fields, Lit, Type};

/// Information extracted from a single field attribute
struct AttrInfo {
    value_check: Option<proc_macro2::TokenStream>,
    runes_check: Option<proc_macro2::TokenStream>,
    rest: bool,
    rune_id_expr: Option<proc_macro2::TokenStream>,
    rune_amount_expr: Option<proc_macro2::TokenStream>,
    anchor_ident: Option<syn::Ident>,
    anchor_span: Option<proc_macro2::Span>,
}

impl Default for AttrInfo {
    fn default() -> Self {
        Self {
            value_check: None,
            runes_check: None,
            rest: false,
            rune_id_expr: None,
            rune_amount_expr: None,
            anchor_ident: None,
            anchor_span: None,
        }
    }
}

/// Determine if the type is `&'a UtxoInfo`, `Vec<&'a UtxoInfo>` or an array `[&'a UtxoInfo; N]`.
#[derive(Debug, Clone)]
enum FieldKind {
    Single,
    Array(proc_macro2::TokenStream), // len expression tokenstream
    Vec,
    Optional, // Option<&'a UtxoInfo>
}

/// Derive macro that generates an implementation of [`TryFromUtxos`] for a
/// struct, providing declarative parsing and validation of
/// [`saturn_bitcoin_transactions::utxo_info::UtxoInfo`] inputs.
///
/// This macro enables you to define strongly-typed structures that automatically
/// parse and validate UTXO inputs according to your specification, eliminating
/// boilerplate code and reducing the chance of errors.
///
/// # How it works
///
/// Each field of the annotated struct is matched against the slice supplied to
/// `try_utxos` according to the field's *type* and optional `#[utxo(..)]`
/// *attribute*. Matched UTXOs are removed from consideration; if any inputs
/// remain unconsumed, or a validation check fails, the generated
/// method returns an appropriate [`ProgramError`]:
///
/// ```ignore
/// // Mandatory UTXO could not be found
/// ProgramError::Custom(ErrorCode::MissingRequiredUtxo.into())
///
/// // There are leftover inputs not matched by any field
/// ProgramError::Custom(ErrorCode::UnexpectedExtraUtxos.into())
///
/// // Predicate checks failed
/// ProgramError::Custom(ErrorCode::InvalidUtxoValue.into())
/// ProgramError::Custom(ErrorCode::InvalidRunesPresence.into())
/// ProgramError::Custom(ErrorCode::InvalidRuneId.into())
/// ProgramError::Custom(ErrorCode::InvalidRuneAmount.into())
/// ```
///
/// # Supported field types
///
/// | Rust type                               | Behaviour                                              |
/// | --------------------------------------- | ------------------------------------------------------ |
/// | `&'a UtxoInfo`                          | Exactly one matching UTXO **must** be present.         |
/// | `Option<&'a UtxoInfo>`                  | Zero or one matching UTXO may be present.              |
/// | `[&'a UtxoInfo; N]`                     | Exactly *N* matching UTXOs must be present.            |
/// | `Vec<&'a UtxoInfo>` **(see `rest`)**    | Variable-length list capturing remaining UTXOs.        |
///
/// A `Vec` field **must** be annotated with the `rest` flag, otherwise the
/// compilation will fail.
///
/// # `#[utxo(..)]` attribute
///
/// The attribute accepts a comma-separated list of *flags* and *key/value*
/// pairs that narrow the search predicate for the associated field:
///
/// ## Flags
///   * `rest` – mark a `Vec` field as the catch-all container receiving any
///     inputs not matched by earlier fields.
///
/// ## Key/Value Pairs
///   * `value = <expr>` – match only UTXOs whose `value` (in satoshis) is equal
///     to the given expression.
///   * `runes = "none" | "some" | "any"` – constrain presence of runes:
///       * `"none"` – assert that no runes are present.
///       * `"some"` – assert that at least one rune is present.
///       * `"any"` – do not check runes (default).
///   * `rune_id = <expr>` – match only UTXOs that contain the specified rune
///     id. May be combined with `rune_amount` for an exact match.
///   * `rune_amount = <expr>` – If `rune_id` is also provided, require the UTXO
///     to hold exactly this amount of the given rune. Otherwise require the
///     *total* rune amount inside the UTXO to equal the expression.
///   * `anchor = <ident>` – Expect identifier that refers to a field in the Accounts struct
///
/// The predicate generated from these parameters is applied to each candidate
/// UTXO until a match is found.
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust,ignore
/// use saturn_utxo_parser::{UtxoParser, TryFromUtxos};
/// use saturn_bitcoin_transactions::utxo_info::UtxoInfo;
///
/// #[derive(UtxoParser)]
/// struct SimpleSwap<'a> {
///     // UTXO paying the on-chain fee.
///     #[utxo(value = 10_000, runes = "none")]
///     fee: &'a UtxoInfo,
///
///     // Optional rune deposit, any amount.
///     #[utxo(runes = "some")]
///     deposit: Option<&'a UtxoInfo>,
///
///     // Capture all remaining inputs.
///     #[utxo(rest)]
///     others: Vec<&'a UtxoInfo>,
/// }
/// ```
///
/// ## Array Fields
///
/// ```rust,ignore
/// use saturn_utxo_parser::{UtxoParser, TryFromUtxos};
/// use saturn_bitcoin_transactions::utxo_info::UtxoInfo;
///
/// #[derive(UtxoParser)]
/// struct MultiInput<'a> {
///     // Exactly 3 UTXOs with specific value
///     #[utxo(value = 5_000)]
///     inputs: [&'a UtxoInfo; 3],
/// }
/// ```
///
/// ## Rune-specific Matching
///
/// ```rust,ignore
/// use saturn_utxo_parser::{UtxoParser, TryFromUtxos};
/// use saturn_bitcoin_transactions::utxo_info::UtxoInfo;
///
/// #[derive(UtxoParser)]
/// struct RuneTransfer<'a> {
///     // Exact rune ID and amount
///     #[utxo(rune_id = my_rune_id, rune_amount = 1000)]
///     specific_rune: &'a UtxoInfo,
///
///     // Any UTXO with exactly 500 total runes
///     #[utxo(rune_amount = 500)]
///     any_rune_500: &'a UtxoInfo,
/// }
/// ```
///
/// # Important Notes
///
/// - Field order matters: UTXOs are matched in the order fields appear in the struct
/// - Each UTXO can only be matched once
/// - The `rest` field (if present) should typically be the last field
/// - All expressions in attributes are evaluated in the context where the macro is used
///
/// [`TryFromUtxos`]: crate::TryFromUtxos
/// [`ProgramError`]: arch_program::program_error::ProgramError
#[proc_macro_derive(UtxoParser, attributes(utxo, utxo_accounts))]
pub fn derive_utxo_parser(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let struct_ident = &input.ident;
    let generics = &input.generics;
    let (_impl_generics, ty_generics, _where_clause) = generics.split_for_impl();

    // Storage for generated code pieces
    let mut field_initializers = Vec::new();
    let mut field_idents = Vec::new();

    // ------------------------------------------------------------------
    // Parse outer attribute specifying the Accounts type: #[utxo_accounts(TypePath)]
    // ------------------------------------------------------------------
    let mut accounts_ty_opt: Option<syn::Type> = None;
    for attr in &input.attrs {
        if attr.path().is_ident("utxo_accounts") {
            if accounts_ty_opt.is_some() {
                return syn::Error::new(attr.span(), "duplicate #[utxo_accounts] attribute")
                    .to_compile_error()
                    .into();
            }
            match attr.parse_args::<syn::Type>() {
                Ok(ty) => accounts_ty_opt = Some(ty),
                Err(e) => return e.to_compile_error().into(),
            }
        }
    }

    let accounts_ty = match accounts_ty_opt {
        Some(t) => t,
        None => {
            return syn::Error::new(
                input.ident.span(),
                "missing required #[utxo_accounts(<Type>)] attribute",
            )
            .to_compile_error()
            .into();
        }
    };

    // ------------------------------------------------------------------
    // Walk through struct fields and collect config
    // ------------------------------------------------------------------
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            other => {
                return syn::Error::new(
                    other.span(),
                    "UtxoParser only supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new(input.span(), "UtxoParser can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    // We'll build predicates and extraction logic that operates over a `remaining` Vec
    field_initializers.push(quote! {
        let mut remaining: Vec<&'a saturn_bitcoin_transactions::utxo_info::UtxoInfo> = utxos.iter().collect();
    });

    // Track whether we have already seen an `anchor = ...` attribute. Only one field may specify it.
    let mut anchor_attr_seen = false;

    for field in fields {
        let ident = field.ident.clone().expect("named field");
        let ty = &field.ty;
        field_idents.push(quote! { #ident });

        // --------------------------------------------------------------
        // Parse #[utxo(...)] attribute on this field
        // --------------------------------------------------------------
        let mut attr_info = AttrInfo::default();

        for attr in &field.attrs {
            if !attr.path().is_ident("utxo") {
                continue;
            }

            let args = match attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            ) {
                Ok(a) => a,
                Err(e) => return e.to_compile_error().into(),
            };

            for meta in args {
                match meta {
                    syn::Meta::Path(path) => {
                        if path.is_ident("rest") {
                            attr_info.rest = true;
                        } else {
                            return syn::Error::new(
                                path.span(),
                                "Unknown flag inside #[utxo(...)] attribute",
                            )
                            .to_compile_error()
                            .into();
                        }
                    }
                    syn::Meta::NameValue(nv) => {
                        let ident_str = {
                            if let Some(id) = nv.path.get_ident() {
                                id.to_string()
                            } else {
                                return syn::Error::new(nv.path.span(), "Expected ident")
                                    .to_compile_error()
                                    .into();
                            }
                        };
                        match ident_str.as_str() {
                            "value" => {
                                let expr = nv.value.clone();
                                attr_info.value_check = Some(quote! { utxo.value == (#expr) });
                            }
                            "runes" => {
                                if let Expr::Lit(expr_lit) = &nv.value {
                                    if let Lit::Str(lit_str) = &expr_lit.lit {
                                        match lit_str.value().as_str() {
                                            "none" => {
                                                attr_info.runes_check =
                                                    Some(quote! { utxo.runes.len() == 0 })
                                            }
                                            "some" => {
                                                attr_info.runes_check =
                                                    Some(quote! { utxo.runes.len() > 0 })
                                            }
                                            "any" => {}
                                            other => {
                                                return syn::Error::new(lit_str.span(), format!("unsupported runes value '{}'. expected 'none', 'some', or 'any'", other)).to_compile_error().into();
                                            }
                                        }
                                    }
                                }
                            }
                            "rune_id" => {
                                let expr = nv.value.clone();
                                attr_info.rune_id_expr = Some(quote! { (#expr) });
                            }
                            "rune_amount" => {
                                let expr = nv.value.clone();
                                attr_info.rune_amount_expr = Some(quote! { (#expr) as u128 });
                            }
                            "anchor" => {
                                // Expect identifier that refers to a field in the Accounts struct
                                attr_info.anchor_span = Some(nv.path.span());
                                match &nv.value {
                                    Expr::Path(expr_path) => {
                                        if let Some(id) = expr_path.path.get_ident() {
                                            attr_info.anchor_ident = Some(id.clone());
                                        } else {
                                            return syn::Error::new(
                                                expr_path.span(),
                                                "anchor expects an identifier",
                                            )
                                            .to_compile_error()
                                            .into();
                                        }
                                    }
                                    other => {
                                        return syn::Error::new(
                                            other.span(),
                                            "anchor expects an identifier path",
                                        )
                                        .to_compile_error()
                                        .into();
                                    }
                                }
                            }
                            other => {
                                return syn::Error::new(
                                    nv.path.span(),
                                    format!("Unknown key '{}' in #[utxo(...)] attribute", other),
                                )
                                .to_compile_error()
                                .into();
                            }
                        }
                    }
                    _ => {
                        return syn::Error::new(meta.span(), "Unsupported meta in attribute")
                            .to_compile_error()
                            .into();
                    }
                }
            }
        }

        // Detect duplicate `anchor` attributes across fields ---------------------------------------
        if let Some(anchor_ident) = &attr_info.anchor_ident {
            if anchor_attr_seen {
                let err_span = attr_info.anchor_span.unwrap_or_else(|| ident.span());
                return syn::Error::new(
                    err_span,
                    "Multiple fields specify `anchor` attribute; only one field is allowed",
                )
                .to_compile_error()
                .into();
            }
            anchor_attr_seen = true;
        }

        // Choose error variant based on which attribute checks are configured
        let err_variant: proc_macro2::TokenStream = if attr_info.rune_id_expr.is_some() {
            quote! { ErrorCode::InvalidRuneId }
        } else if attr_info.rune_amount_expr.is_some() {
            quote! { ErrorCode::InvalidRuneAmount }
        } else if attr_info.runes_check.is_some() {
            quote! { ErrorCode::InvalidRunesPresence }
        } else if attr_info.value_check.is_some() {
            quote! { ErrorCode::InvalidUtxoValue }
        } else {
            quote! { ErrorCode::MissingRequiredUtxo }
        };

        // Build predicate tokenstream for this field
        let predicate = {
            let mut parts = Vec::<proc_macro2::TokenStream>::new();
            if let Some(p) = &attr_info.value_check {
                parts.push(p.clone());
            }
            if let Some(p) = &attr_info.runes_check {
                parts.push(p.clone());
            }

            // Custom rune id / amount checks
            match (&attr_info.rune_id_expr, &attr_info.rune_amount_expr) {
                (Some(id_expr), Some(amount_expr)) => {
                    parts.push(quote! { utxo.contains_exact_rune(&#id_expr, #amount_expr) });
                }
                (Some(id_expr), None) => {
                    parts.push(quote! { utxo.rune_amount(&#id_expr).is_some() });
                }
                (None, Some(amount_expr)) => {
                    parts.push(quote! { utxo.total_rune_amount() == #amount_expr });
                }
                _ => {}
            }

            if parts.is_empty() {
                quote! { true }
            } else {
                let joined = parts.iter();
                quote! { #( #joined )&&* }
            }
        };

        // Determine field kind
        let kind = match ty {
            Type::Reference(_) => FieldKind::Single,
            Type::Array(arr) => {
                let len_tokens = arr.len.clone();
                FieldKind::Array(quote! { #len_tokens })
            }
            Type::Path(type_path) => {
                // crude check for Vec<...> or Option<...>
                if let Some(segment) = type_path.path.segments.last() {
                    if segment.ident == "Vec" {
                        FieldKind::Vec
                    } else if segment.ident == "Option" {
                        FieldKind::Optional
                    } else {
                        return syn::Error::new(type_path.span(), "Unsupported field type for UtxoParser derive. Expected Vec, Option, reference, or array of &'a UtxoInfo")
                            .to_compile_error()
                            .into();
                    }
                } else {
                    return syn::Error::new(type_path.span(), "Unexpected type path")
                        .to_compile_error()
                        .into();
                }
            }
            _ => {
                return syn::Error::new(ty.span(), "Unsupported field type for UtxoParser derive")
                    .to_compile_error()
                    .into();
            }
        };

        // Generate extraction code depending on kind
        let tokens = match kind {
            FieldKind::Single => {
                let anchor_handling = {
                    if let Some(anchor_ident) = &attr_info.anchor_ident {
                        let anchor_ident_tok = anchor_ident.clone();
                        quote! {
                            let _anchor_target = &accounts.#anchor_ident_tok;
                            let _anchor_ix = arch_program::system_instruction::anchor(
                                _anchor_target.key,
                                #ident.meta.txid_big_endian(),
                                #ident.meta.vout(),
                            );
                            #ident.set_anchor(*_anchor_target.key);
                        }
                    } else {
                        quote! {}
                    }
                };

                // When both rune_id and rune_amount predicates are present we want more specific diagnostics.
                let specialized_err_logic = if attr_info.rune_id_expr.is_some()
                    && attr_info.rune_amount_expr.is_some()
                {
                    // Build predicate parts used to detect ID-only matches.
                    let mut id_only_parts = Vec::<proc_macro2::TokenStream>::new();
                    if let Some(p) = &attr_info.value_check {
                        id_only_parts.push(p.clone());
                    }
                    if let Some(p) = &attr_info.runes_check {
                        id_only_parts.push(p.clone());
                    }
                    let id_expr_ts = attr_info.rune_id_expr.clone().unwrap();
                    id_only_parts.push(quote! { utxo.rune_amount(&#id_expr_ts).is_some() });

                    let joined_id_only = {
                        let iter = id_only_parts.iter();
                        quote! { #( #iter )&&* }
                    };

                    quote! {
                        let has_id_match = remaining.iter().any(|utxo| { #joined_id_only });
                        if has_id_match {
                            return Err(ProgramError::Custom(ErrorCode::InvalidRuneAmount.into()));
                        } else {
                            return Err(ProgramError::Custom(ErrorCode::InvalidRuneId.into()));
                        }
                    }
                } else {
                    // fallback to previously selected error variant
                    quote! { return Err(ProgramError::Custom(#err_variant.into())); }
                };

                quote! {
                    let pos_opt = remaining.iter().position(|utxo| { #predicate });
                    let #ident = if let Some(idx) = pos_opt {
                        remaining.remove(idx)
                    } else {
                        #specialized_err_logic
                    };
                    #anchor_handling
                }
            }
            FieldKind::Array(len_expr) => {
                let tmp_ident = format_ident!("{}_tmp", ident);
                // Generate anchor handling snippet executed within the loop (if `anchor` specified)
                let anchor_loop_handling = if let Some(anchor_ident) = &attr_info.anchor_ident {
                    let anchor_ident_tok = anchor_ident.clone();
                    quote! {
                        let _anchor_target = &accounts.#anchor_ident_tok[i];
                        let _anchor_ix = arch_program::system_instruction::anchor(
                            _anchor_target.key,
                            utxo_ref.meta.txid_big_endian(),
                            utxo_ref.meta.vout(),
                        );
                        utxo_ref.set_anchor(*_anchor_target.key);
                    }
                } else {
                    quote! {}
                };
                quote! {
                    let mut #tmp_ident: [Option<&'a saturn_bitcoin_transactions::utxo_info::UtxoInfo>; #len_expr] = [None; #len_expr];
                    for i in 0..#len_expr {
                        let pos_opt = remaining.iter().position(|utxo| { #predicate });
                        let utxo_ref = if let Some(idx) = pos_opt {
                            remaining.remove(idx)
                        } else {
                            return Err(ProgramError::Custom(#err_variant.into()));
                        };
                        #anchor_loop_handling
                        #tmp_ident[i] = Some(utxo_ref);
                    }
                    let #ident: [&'a saturn_bitcoin_transactions::utxo_info::UtxoInfo; #len_expr] = #tmp_ident.map(|o| o.unwrap());
                }
            }
            FieldKind::Vec => {
                if !attr_info.rest {
                    return syn::Error::new(
                        ty.span(),
                        "Vec field must be marked with rest flag: #[utxo(rest, ...)]",
                    )
                    .to_compile_error()
                    .into();
                }
                quote! {
                    let mut #ident: Vec<&'a saturn_bitcoin_transactions::utxo_info::UtxoInfo> = Vec::new();
                    remaining.retain(|utxo| {
                        if { #predicate } {
                            #ident.push(*utxo);
                            false // remove from remaining
                        } else { true }
                    });
                }
            }
            FieldKind::Optional => {
                let anchor_handling = {
                    if let Some(anchor_ident) = &attr_info.anchor_ident {
                        let anchor_ident_tok = anchor_ident.clone();
                        quote! {
                            if let Some(__opt_utxo) = #ident {
                                let _anchor_target = &accounts.#anchor_ident_tok;
                                let _anchor_ix = arch_program::system_instruction::anchor(
                                    _anchor_target.key,
                                    __opt_utxo.meta.txid_big_endian(),
                                    __opt_utxo.meta.vout(),
                                );
                                __opt_utxo.set_anchor(*_anchor_target.key);
                            }
                        }
                    } else {
                        quote! {}
                    }
                };

                quote! {
                    let pos_opt = remaining.iter().position(|utxo| { #predicate });
                    let #ident = if let Some(idx) = pos_opt {
                        Some(remaining.remove(idx))
                    } else {
                        None
                    };
                    #anchor_handling
                }
            }
        };

        field_initializers.push(tokens);
    }

    // After all fields processed, ensure no utxos remain unconsumed
    field_initializers.push(quote! {
        if !remaining.is_empty() {
            return Err(ProgramError::Custom(ErrorCode::UnexpectedExtraUtxos.into()));
        }
    });

    // ------------------------------------------------------------------
    // Generate final impl
    // ------------------------------------------------------------------
    let expanded = quote! {
        impl<'a> saturn_utxo_parser::TryFromUtxos<'a> for #struct_ident #ty_generics {
            type Accs = #accounts_ty;

            fn try_utxos(
                accounts: &'a Self::Accs,
                utxos: &'a [saturn_bitcoin_transactions::utxo_info::UtxoInfo]
            ) -> core::result::Result<Self, arch_program::program_error::ProgramError> {
                use arch_program::program_error::ProgramError;
                use saturn_utxo_parser::ErrorCode;

                #(#field_initializers)*

                Ok(Self { #(#field_idents),* })
            }
        }
    };

    TokenStream::from(expanded)
}
