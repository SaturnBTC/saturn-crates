use proc_macro2::TokenStream;
use syn::parse::Parser;
use syn::{punctuated::Punctuated, Error, Lit, Meta, Path};

/// Upper bounds used to validate `btc_tx_cfg` numeric parameters.  These are
/// intentionally conservative to avoid generating gigantic const-generic
/// instantiations that blow up compile-time memory usage or LLVM IR size.
/// Tune them as the runtime implementation evolves.
const MAX_INPUTS_TO_SIGN_LIMIT: usize = 64;
const MAX_MODIFIED_ACCOUNTS_LIMIT: usize = 64;

/// Bitcoin-transaction specific configuration parsed from the attribute list.
#[derive(Default, Clone)]
pub struct BtcTxCfg {
    pub max_inputs_to_sign: Option<usize>,
    pub max_modified_accounts: Option<usize>,
    pub rune_set: Option<Path>,
    pub rune_capacity: Option<usize>,
}

/// Result of parsing the procedural macro attribute.
#[derive(Clone)]
pub struct AttrConfig {
    /// If `true`, the generated program will include Bitcoin transaction builder logic.
    /// This is enabled when a `btc_tx_cfg(..)` section is present in the attribute list.
    pub enable_bitcoin_tx: bool,
    pub btc_tx_cfg: BtcTxCfg,
}

/// Parse the attribute list provided to `#[saturn_program(..)]`.
///
/// This function is **pure** (no side-effects) so it can be unit-tested easily.
pub fn parse(attr: TokenStream) -> Result<AttrConfig, Error> {
    // ------------------------------------------------------------
    // 1. Parse attribute list into `Meta` items
    // ------------------------------------------------------------
    let parser = Punctuated::<Meta, syn::Token![,]>::parse_terminated;
    // When the attribute is used without parentheses (e.g. `#[saturn_program]`),
    // the incoming token stream is empty.  Treat this as the *default* configuration
    // rather than an immediate compile-error so that the ergonomic, argument-less
    // form continues to work.
    let attr_meta: Punctuated<Meta, syn::Token![,]> = if attr.is_empty() {
        Punctuated::new()
    } else {
        parser.parse2(attr)?
    };

    let mut btc_tx_cfg: BtcTxCfg = BtcTxCfg::default();

    // Flags used during the second pass
    let mut btc_tx_cfg_seen = false;

    // ------------------------------------------------------------
    // 2. Handle each top-level attribute key/section
    // ------------------------------------------------------------
    for meta in &attr_meta {
        match meta {
            // ---------------------------
            // btc_tx_cfg = ( ... )
            // ---------------------------
            Meta::List(ml) if ml.path.is_ident("btc_tx_cfg") => {
                if btc_tx_cfg_seen {
                    return Err(Error::new_spanned(
                        &ml.path,
                        "duplicate `btc_tx_cfg` section",
                    ));
                }
                btc_tx_cfg_seen = true;

                let inner_parser = Punctuated::<Meta, syn::Token![,]>::parse_terminated;
                let inner_meta = inner_parser.parse2(ml.tokens.clone())?;
                for nested in inner_meta {
                    match nested {
                        Meta::NameValue(nv) if nv.path.is_ident("max_inputs_to_sign") => {
                            // Reject duplicates – should only appear once
                            if btc_tx_cfg.max_inputs_to_sign.is_some() {
                                return Err(Error::new_spanned(
                                    &nv.path,
                                    "duplicate `max_inputs_to_sign` key inside btc_tx_cfg",
                                ));
                            }
                            if let syn::Expr::Lit(expr_lit) = &nv.value {
                                if let Lit::Int(int_lit) = &expr_lit.lit {
                                    let value = int_lit.base10_parse::<usize>()?;
                                    if value > MAX_INPUTS_TO_SIGN_LIMIT {
                                        return Err(Error::new_spanned(
                                            &nv.value,
                                            format!(
                                                "max_inputs_to_sign exceeds allowed maximum ({MAX_INPUTS_TO_SIGN_LIMIT})",
                                            ),
                                        ));
                                    }
                                    btc_tx_cfg.max_inputs_to_sign = Some(value);
                                } else {
                                    return Err(Error::new_spanned(
                                        &nv.value,
                                        "max_inputs_to_sign must be an integer literal",
                                    ));
                                }
                            } else {
                                return Err(Error::new_spanned(
                                    &nv.value,
                                    "max_inputs_to_sign must be an integer literal",
                                ));
                            }
                        }
                        Meta::NameValue(nv) if nv.path.is_ident("max_modified_accounts") => {
                            // Reject duplicates – should only appear once
                            if btc_tx_cfg.max_modified_accounts.is_some() {
                                return Err(Error::new_spanned(
                                    &nv.path,
                                    "duplicate `max_modified_accounts` key inside btc_tx_cfg",
                                ));
                            }
                            if let syn::Expr::Lit(expr_lit) = &nv.value {
                                if let Lit::Int(int_lit) = &expr_lit.lit {
                                    let value = int_lit.base10_parse::<usize>()?;
                                    if value > MAX_MODIFIED_ACCOUNTS_LIMIT {
                                        return Err(Error::new_spanned(
                                            &nv.value,
                                            format!(
                                                "max_modified_accounts exceeds allowed maximum ({MAX_MODIFIED_ACCOUNTS_LIMIT})",
                                            ),
                                        ));
                                    }
                                    btc_tx_cfg.max_modified_accounts = Some(value);
                                } else {
                                    return Err(Error::new_spanned(
                                        &nv.value,
                                        "max_modified_accounts must be an integer literal",
                                    ));
                                }
                            } else {
                                return Err(Error::new_spanned(
                                    &nv.value,
                                    "max_modified_accounts must be an integer literal",
                                ));
                            }
                        }
                        Meta::NameValue(nv) if nv.path.is_ident("rune_set") => {
                            // Reject duplicates – should only appear once
                            if btc_tx_cfg.rune_set.is_some() {
                                return Err(Error::new_spanned(
                                    &nv.path,
                                    "duplicate `rune_set` key inside btc_tx_cfg",
                                ));
                            }
                            if let syn::Expr::Lit(expr_lit) = &nv.value {
                                if let Lit::Str(lit_str) = &expr_lit.lit {
                                    btc_tx_cfg.rune_set = Some(syn::parse_str(&lit_str.value())?);
                                } else {
                                    return Err(Error::new_spanned(
                                        &nv.value,
                                        "rune_set must be a string literal path",
                                    ));
                                }
                            } else {
                                return Err(Error::new_spanned(
                                    &nv.value,
                                    "rune_set must be a string literal",
                                ));
                            }
                        }
                        Meta::NameValue(nv) if nv.path.is_ident("rune_capacity") => {
                            // Reject duplicates – should only appear once
                            if btc_tx_cfg.rune_capacity.is_some() {
                                return Err(Error::new_spanned(
                                    &nv.path,
                                    "duplicate `rune_capacity` key inside btc_tx_cfg",
                                ));
                            }
                            if let syn::Expr::Lit(expr_lit) = &nv.value {
                                if let Lit::Int(int_lit) = &expr_lit.lit {
                                    btc_tx_cfg.rune_capacity =
                                        Some(int_lit.base10_parse::<usize>()?);
                                } else {
                                    return Err(Error::new_spanned(
                                        &nv.value,
                                        "rune_capacity must be an integer literal",
                                    ));
                                }
                            } else {
                                return Err(Error::new_spanned(
                                    &nv.value,
                                    "rune_capacity must be an integer literal",
                                ));
                            }
                        }
                        other => {
                            return Err(Error::new_spanned(
                                other,
                                "unknown key inside btc_tx_cfg; expected `max_inputs_to_sign`, `max_modified_accounts`, `rune_set`, or `rune_capacity`",
                            ));
                        }
                    }
                }
            }

            other => {
                return Err(Error::new_spanned(
                    other,
                    "unknown attribute key; expected `btc_tx_cfg`",
                ));
            }
        }
    }

    // ---------------------------
    // 3. Final validations + defaults
    // ---------------------------
    let enable_bitcoin_tx = btc_tx_cfg_seen;

    if enable_bitcoin_tx {
        if btc_tx_cfg.max_inputs_to_sign.is_none() {
            return Err(Error::new_spanned(
                TokenStream::new(),
                "`btc_tx_cfg` must specify `max_inputs_to_sign`",
            ));
        }
        if btc_tx_cfg.max_modified_accounts.is_none() {
            return Err(Error::new_spanned(
                TokenStream::new(),
                "`btc_tx_cfg` must specify `max_modified_accounts`",
            ));
        }

        // Enforce that **exactly one** of `rune_set` or `rune_capacity` is supplied.
        match (
            btc_tx_cfg.rune_set.is_some(),
            btc_tx_cfg.rune_capacity.is_some(),
        ) {
            // User provided an explicit rune set path – nothing else to do.
            (true, false) => {}
            // User opted for `rune_capacity` – inject a placeholder path so later macro
            // phases can safely `unwrap()` it. A concrete alias will be generated in the
            // dispatcher phase.
            (false, true) => {
                btc_tx_cfg.rune_set = Some(syn::parse_str("RuneSet")?);
            }
            // Both keys at once – reject.
            (true, true) => {
                return Err(Error::new_spanned(
                    TokenStream::new(),
                    "btc_tx_cfg keys rune_set and rune_capacity are mutually exclusive",
                ));
            }
            // Neither key – reject and ask the user to pick one.
            (false, false) => {
                return Err(Error::new_spanned(
                    TokenStream::new(),
                    "`btc_tx_cfg` must specify exactly one of `rune_set` or `rune_capacity`",
                ));
            }
        }
    }

    Ok(AttrConfig {
        enable_bitcoin_tx,
        btc_tx_cfg,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn parses_full_bitcoin_tx_cfg() {
        let ts: proc_macro2::TokenStream = quote!(btc_tx_cfg(
            max_inputs_to_sign = 8,
            max_modified_accounts = 16,
            rune_capacity = 32
        ));
        let cfg = parse(ts).expect("should parse");
        assert!(cfg.enable_bitcoin_tx);
        assert_eq!(cfg.btc_tx_cfg.max_inputs_to_sign, Some(8));
        assert_eq!(cfg.btc_tx_cfg.max_modified_accounts, Some(16));
        assert_eq!(cfg.btc_tx_cfg.rune_capacity, Some(32));
    }
}
