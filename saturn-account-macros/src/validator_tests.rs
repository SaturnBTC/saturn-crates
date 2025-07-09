// validator_tests.rs – unit tests for cross-field semantic validation
#![cfg(test)]

use crate::{parser, validator};
use arch_program::account::AccountInfo;
use saturn_account_parser::codec::Account;
use syn::{parse_quote, Data, DeriveInput, Fields};

/// Extract named fields helper reused across tests.
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

// ──────────────────────────────────────────────────────────────────────────
// Negative cases – validator must emit an error
// ──────────────────────────────────────────────────────────────────────────

/// 2.2 – Duplicate field identifiers are rejected.
#[test]
fn validator_rejects_duplicate_identifiers() {
    let di: DeriveInput = parse_quote! {
        struct Dup<'info> {
            #[account(signer)]
            dup: Account<'info, u64>,
            #[account(len = 1)]
            dup: Vec<AccountInfo<'static>>,
        }
    };

    let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
    let err = validator::validate(&parsed).unwrap_err();
    assert!(err.to_string().contains("duplicate field identifier"));
}

/// 2.7 – `payer` refers to a non-existent field.
#[test]
fn validator_rejects_unknown_payer_field() {
    let di: DeriveInput = parse_quote! {
        struct Accs<'info> {
            #[account(signer)]
            payer: Account<'info, u64>,
            #[account(init, payer = ghost, program_id = arch_program::pubkey::Pubkey::default())]
            new_acc: Account<'info, u64>,
        }
    };

    let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
    let err = validator::validate(&parsed).unwrap_err();
    assert!(err.to_string().contains("unknown field `ghost`"));
}

/// 2.6 – `payer` expression must be a simple identifier, not an arbitrary expression.
#[test]
fn validator_rejects_non_identifier_payer_expression() {
    let di: DeriveInput = parse_quote! {
        struct Weird<'info> {
            #[account(signer)]
            payer: Account<'info, u64>,
            #[account(init, payer = 42, program_id = arch_program::pubkey::Pubkey::default())]
            new_acc: Account<'info, u64>,
        }
    };

    let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
    let err = validator::validate(&parsed).unwrap_err();
    assert!(err.to_string().contains("must be a single identifier"));
}

// ──────────────────────────────────────────────────────────────────────────
// Positive cases – validator should pass without errors
// ──────────────────────────────────────────────────────────────────────────

/// 2.2 (positive) – multiple init accounts each with a valid signer payer.
#[test]
fn validator_allows_multiple_inits_with_distinct_payers() {
    let di: DeriveInput = parse_quote! {
        struct Accs<'info> {
            #[account(mut, signer)]
            alice: Account<'info, u64>,
            #[account(mut, signer)]
            bob: Account<'info, u64>,
            #[account(mut, init, payer = alice, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
            pool: Account<'info, u64>,
            #[account(mut, init, payer = bob, signer, program_id = arch_program::pubkey::Pubkey::default())]
            vault: Account<'info, u64>,
        }
    };

    let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
    validator::validate(&parsed).expect("validator should accept multiple valid init accounts");
}

/// 2.5 – payer **must** appear before the account it funds; forward reference should error.
#[test]
fn validator_rejects_payer_declared_after_init_field() {
    let di: DeriveInput = parse_quote! {
        struct Accs<'info> {
            #[account(mut, init, payer = signer, program_id = arch_program::pubkey::Pubkey::default())]
            new_acc: Account<'info, u64>,
            #[account(mut, signer)]
            signer: Account<'info, u64>,
        }
    };

    let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
    let err = validator::validate(&parsed).unwrap_err();
    assert!(err.to_string().contains("must be declared before"));
}

// ──────────────────────────────────────────────────────────────────────────
// Additional negative cases – newly covered branches
// ──────────────────────────────────────────────────────────────────────────

/// 2.1 – `payer` points to an existing field that is *not* marked `signer`.
#[test]
fn validator_rejects_payer_not_signer() {
    let di: DeriveInput = parse_quote! {
        struct Accs<'info> {
            #[account()] // plain account, *not* signer
            not_signer: Account<'info, u64>,
            #[account(init, payer = not_signer, program_id = arch_program::pubkey::Pubkey::default())]
            new_acc: Account<'info, u64>,
        }
    };

    let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
    let err = validator::validate(&parsed).unwrap_err();
    assert!(err.to_string().contains("must be marked `signer`"));
}

/// 2.1 (variant) – init field refers to *itself* as payer which is obviously not `signer`.
#[test]
fn validator_rejects_self_payer_reference() {
    let di: DeriveInput = parse_quote! {
        struct Accs<'info> {
            #[account(init, payer = self_acc, program_id = arch_program::pubkey::Pubkey::default())]
            self_acc: Account<'info, u64>,
        }
    };

    let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
    let err = validator::validate(&parsed).unwrap_err();
    assert!(err.to_string().contains("must be marked `signer`"));
}

/// 2.2 (positive) – several init accounts can share *one* signer payer.
#[test]
fn validator_allows_multiple_inits_with_same_payer() {
    let di: DeriveInput = parse_quote! {
        struct Accs<'info> {
            #[account(mut, signer)]
            payer: Account<'info, u64>,
            #[account(mut, init, payer = payer, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
            acc_one: Account<'info, u64>,
            #[account(mut, init, payer = payer, seeds = &[b"seed2"], program_id = arch_program::pubkey::Pubkey::default())]
            acc_two: Account<'info, u64>,
        }
    };

    let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
    validator::validate(&parsed).expect("validator should allow shared payer");
}

/// 0.0 (positive) – struct without any `init` fields should pass trivially.
#[test]
fn validator_allows_struct_with_no_init_fields() {
    let di: DeriveInput = parse_quote! {
        struct Accs<'info> {
            #[account(mut, signer)]
            user: Account<'info, u64>,
            #[account(mut)]
            data: Account<'info, u64>,
        }
    };

    let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
    validator::validate(&parsed).expect("validator should allow structs without init fields");
}

/// 2.6 (negative) – `payer` path with extra segments (e.g. `crate::user`) isn't a simple identifier.
#[test]
fn validator_rejects_multi_segment_payer_path() {
    let di: DeriveInput = parse_quote! {
        struct Accs<'info> {
            #[account(mut, signer)]
            user: Account<'info, u64>,
            #[account(init, payer = crate::user, program_id = arch_program::pubkey::Pubkey::default())]
            new_acc: Account<'info, u64>,
        }
    };

    let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
    let err = validator::validate(&parsed).unwrap_err();
    assert!(err.to_string().contains("unknown field `crate`"));
}

// ──────────────────────────────────────────────────────────────────────────
// Stress / mixed scenario – should still succeed
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn validator_allows_complex_mixed_struct() {
    let di: DeriveInput = parse_quote! {
        struct Accs<'info> {
            // signers
            #[account(mut, signer)]
            alice: Account<'info, u64>,
            #[account(mut, signer)]
            bob: Account<'info, u64>,

            // init accounts (different ordering)
            #[account(mut, init, payer = bob, seeds = &[b"seed2"], program_id = arch_program::pubkey::Pubkey::default())]
            early_vault: Account<'info, u64>,

            // plain accounts
            #[account(mut)]
            cfg: Account<'info, u64>,

            // more init accounts referencing previous signers
            #[account(mut, init, payer = alice, signer, program_id = arch_program::pubkey::Pubkey::default())]
            pool: Account<'info, u64>,
            #[account(mut, init, payer = bob, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
            vault_b: Account<'info, u64>,
        }
    };

    let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
    validator::validate(&parsed).expect("validator should accept complex mixed structs");
}

/// 2.x – validator allows init_if_needed referencing a signer payer.
#[test]
fn validator_allows_init_if_needed() {
    let di: DeriveInput = parse_quote! {
        struct Accs<'info> {
            #[account(mut, signer)]
            payer: Account<'info, u64>,
            #[account(mut, init_if_needed, payer = payer, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
            maybe_new: Account<'info, u64>,
        }
    };

    let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
    validator::validate(&parsed).expect("validator should accept init_if_needed");
}

/// 2.x – validator allows realloc field with valid signer payer.
#[test]
fn validator_allows_realloc_with_valid_payer() {
    let di: DeriveInput = parse_quote! {
        struct Accs<'info> {
            #[account(mut, signer)]
            payer: Account<'info, u64>,
            #[account(mut, realloc, payer = payer, space = 32)]
            data: Account<'info, u64>,
        }
    };

    let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
    validator::validate(&parsed).expect("validator should allow realloc with signer payer");
}

/// 2.x – validator rejects realloc when payer field is not signer.
#[test]
fn validator_rejects_realloc_payer_not_signer() {
    let di: DeriveInput = parse_quote! {
        struct Accs<'info> {
            payer: Account<'info, u64>,
            #[account(realloc, payer = payer, space = 64)]
            data: Account<'info, u64>,
        }
    };

    let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
    let err = validator::validate(&parsed).unwrap_err();
    assert!(err.to_string().contains("must be marked `signer`"));
}
