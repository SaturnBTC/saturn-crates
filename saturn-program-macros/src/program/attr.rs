use proc_macro2::TokenStream;
use syn::parse::Parser;
use syn::{punctuated::Punctuated, Error, Lit, Meta, Path};

/// Bitcoin-transaction specific configuration parsed from the attribute list.
#[derive(Default, Clone)]
pub struct BtcTxCfg {
    pub max_inputs_to_sign: Option<usize>,
    pub max_modified_accounts: Option<usize>,
    pub rune_set: Option<Path>,
}

/// Result of parsing the procedural macro attribute.
#[derive(Clone)]
pub struct AttrConfig {
    pub instruction_path: Path,
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
    let attr_meta: Punctuated<Meta, syn::Token![,]> = if attr.is_empty() {
        return Err(Error::new_spanned(
            TokenStream::new(),
            "expected attribute arguments",
        ));
    } else {
        parser.parse2(attr)?
    };

    let mut instruction_path: Option<Path> = None;
    let mut enable_bitcoin_tx: bool = false;
    let mut btc_tx_cfg: BtcTxCfg = BtcTxCfg::default();

    // First pass: detect `bitcoin_transaction = bool`
    let mut bitcoin_tx_flag_set = false;
    for meta in &attr_meta {
        if let Meta::NameValue(nv) = meta {
            if nv.path.is_ident("bitcoin_transaction") {
                if bitcoin_tx_flag_set {
                    return Err(Error::new_spanned(
                        &nv.path,
                        "duplicate `bitcoin_transaction` key",
                    ));
                }
                bitcoin_tx_flag_set = true;
                match &nv.value {
                    syn::Expr::Lit(expr_lit) => {
                        if let Lit::Bool(lit_bool) = &expr_lit.lit {
                            enable_bitcoin_tx = lit_bool.value;
                        } else {
                            return Err(Error::new_spanned(
                                &nv.value,
                                "bitcoin_transaction value must be a boolean literal",
                            ));
                        }
                    }
                    _ => {
                        return Err(Error::new_spanned(
                            &nv.value,
                            "bitcoin_transaction value must be boolean literal",
                        ));
                    }
                }
            }
        }
    }

    // Second pass: handle the remaining keys, including the nested `btc_tx_cfg` section.
    let mut btc_tx_cfg_seen = false;
    for meta in &attr_meta {
        match meta {
            // Skip: handled in first pass
            Meta::NameValue(nv) if nv.path.is_ident("bitcoin_transaction") => {}

            // ---------------------------
            // instruction = "path::ToInstr"
            // ---------------------------
            Meta::NameValue(nv) if nv.path.is_ident("instruction") => {
                if instruction_path.is_some() {
                    return Err(Error::new_spanned(
                        &nv.path,
                        "duplicate `instruction` key; specify it only once",
                    ));
                }
                match &nv.value {
                    syn::Expr::Lit(expr_lit) => {
                        if let Lit::Str(lit_str) = &expr_lit.lit {
                            let p: Path = syn::parse_str(&lit_str.value())?;
                            if p.segments.len() <= 1 {
                                return Err(Error::new_spanned(
                                    &nv.value,
                                    "instruction path must be namespaced (e.g. `crate::ix::MyInstr`) – add a module prefix",
                                ));
                            }
                            instruction_path = Some(p);
                        } else {
                            return Err(Error::new_spanned(
                                &nv.value,
                                "instruction value must be a string literal path",
                            ));
                        }
                    }
                    _ => {
                        return Err(Error::new_spanned(
                            &nv.value,
                            "instruction value must be string literal",
                        ));
                    }
                }
            }

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

                if !enable_bitcoin_tx {
                    return Err(Error::new_spanned(
                        &ml.path,
                        "`btc_tx_cfg` is only allowed when `bitcoin_transaction = true`",
                    ));
                }

                let inner_parser = Punctuated::<Meta, syn::Token![,]>::parse_terminated;
                let inner_meta = inner_parser.parse2(ml.tokens.clone())?;
                for nested in inner_meta {
                    match nested {
                        Meta::NameValue(nv) if nv.path.is_ident("max_inputs_to_sign") => {
                            if let syn::Expr::Lit(expr_lit) = &nv.value {
                                if let Lit::Int(int_lit) = &expr_lit.lit {
                                    btc_tx_cfg.max_inputs_to_sign =
                                        Some(int_lit.base10_parse::<usize>()?);
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
                            if let syn::Expr::Lit(expr_lit) = &nv.value {
                                if let Lit::Int(int_lit) = &expr_lit.lit {
                                    btc_tx_cfg.max_modified_accounts =
                                        Some(int_lit.base10_parse::<usize>()?);
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
                        other => {
                            return Err(Error::new_spanned(
                                other,
                                "unknown key inside btc_tx_cfg; expected `max_inputs_to_sign`, `max_modified_accounts`, or `rune_set`",
                            ));
                        }
                    }
                }
            }
            other => {
                return Err(Error::new_spanned(
                    other,
                    "unknown attribute key; expected `instruction`, `bitcoin_transaction`, or `btc_tx_cfg`",
                ));
            }
        }
    }

    // ---------------------------
    // Final validations + defaults
    // ---------------------------
    if enable_bitcoin_tx {
        if btc_tx_cfg.max_inputs_to_sign.is_none() {
            return Err(Error::new_spanned(
                TokenStream::new(),
                "`btc_tx_cfg` must specify `max_inputs_to_sign` when bitcoin_transaction is true",
            ));
        }
        if btc_tx_cfg.max_modified_accounts.is_none() {
            return Err(Error::new_spanned(
                TokenStream::new(),
                "`btc_tx_cfg` must specify `max_modified_accounts` when bitcoin_transaction is true",
            ));
        }
        if btc_tx_cfg.rune_set.is_none() {
            btc_tx_cfg.rune_set = Some(syn::parse_str(
                "saturn_bitcoin_transactions::utxo_info::SingleRuneSet",
            )?);
        }
    }

    let instruction_path = instruction_path.ok_or_else(|| {
        Error::new_spanned(
            TokenStream::new(),
            "missing `instruction = \"Path\"` in attribute",
        )
    })?;

    Ok(AttrConfig {
        instruction_path,
        enable_bitcoin_tx,
        btc_tx_cfg,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn parses_minimal_instruction() {
        let ts: proc_macro2::TokenStream = quote!(instruction = "crate::ix::Instr");
        let cfg = parse(ts).expect("should parse");
        assert_eq!(cfg.instruction_path.segments.last().unwrap().ident, "Instr");
        assert!(!cfg.enable_bitcoin_tx);
    }

    #[test]
    fn parses_full_bitcoin_tx_cfg() {
        let ts: proc_macro2::TokenStream = quote!(
            instruction = "crate::ix::Instr",
            bitcoin_transaction = true,
            btc_tx_cfg(max_inputs_to_sign = 8, max_modified_accounts = 16)
        );
        let cfg = parse(ts).expect("should parse");
        assert!(cfg.enable_bitcoin_tx);
        assert_eq!(cfg.btc_tx_cfg.max_inputs_to_sign, Some(8));
        assert_eq!(cfg.btc_tx_cfg.max_modified_accounts, Some(16));
    }

    #[test]
    fn error_on_missing_instruction() {
        let ts: proc_macro2::TokenStream = quote!(bitcoin_transaction = false);
        assert!(parse(ts).is_err());
    }

    #[test]
    fn error_on_btc_tx_cfg_without_flag() {
        let ts: proc_macro2::TokenStream = quote!(
            instruction = "crate::ix::Instr",
            btc_tx_cfg(max_inputs_to_sign = 1, max_modified_accounts = 1)
        );
        assert!(parse(ts).is_err());
    }
}
