#[cfg(test)]
mod parser_tests {
    use crate::{model, parser};

    use super::*;
    use arch_program::account::AccountInfo;
    use saturn_account_parser::codec::Account;
    use syn::{parse_quote, Data, DeriveInput, Fields};

    /// Local helper to extract named fields from a `DeriveInput`.
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

    /// 1.4 – parser accepts a `Vec<...>` field marked with `shards` and `len = N`.
    #[test]
    fn parser_accepts_shards_vector() {
        let di: DeriveInput = parse_quote! {
            struct ShardAccs<'info> {
                #[account(shards, len = 4)]
                shards: Vec<AccountInfo<'static>>,
            }
        };

        let cfgs = parser::parse_fields(extract_named_fields(&di)).expect("parse should succeed");
        assert_eq!(cfgs.len(), 1);
        let shards_cfg = &cfgs[0];
        assert!(shards_cfg.is_shards, "is_shards flag should be true");
        assert!(matches!(shards_cfg.kind, model::FieldKind::Shards(..)));
    }

    /// 1.5 – mixing multiple attributes on a single field should parse correctly.
    #[test]
    fn parser_accepts_combination_of_attributes() {
        let di: DeriveInput = parse_quote! {
            struct Combo<'info> {
                #[account(signer, mut)]
                user: Account<'info, u64>,
                #[account(init, payer = user, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default(), space = 8)]
                new_acc: Account<'info, u64>,
            }
        };

        // Parsing should succeed without errors.
        let cfgs = parser::parse_fields(extract_named_fields(&di)).expect("parse should succeed");
        assert_eq!(cfgs.len(), 2);
        // Basic sanity: first is signer, second is init.
        assert_eq!(cfgs[0].is_signer, Some(true));
        assert!(cfgs[1].is_init);
    }

    /// 1.6 – struct with additional lifetimes / generics parses.
    #[test]
    fn parser_accepts_lifetimes_and_generics() {
        let di: DeriveInput = parse_quote! {
            struct Generic<'info, 'other, T: Copy> {
                #[account(signer)]
                owner: Account<'info, u64>,
                #[account(len = 3)]
                others: Vec<AccountInfo<'static>>,
                _pd: core::marker::PhantomData<&'other T>,
            }
        };

        parser::parse_fields(extract_named_fields(&di))
            .expect("parse should succeed with generics");
    }

    /// 1.7 – attribute order independence.
    #[test]
    fn parser_attribute_order_independent() {
        let di: DeriveInput = parse_quote! {
            struct Order<'info> {
                #[account(mut, signer)]
                acc: Account<'info, u64>,
            }
        };
        let cfgs = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let cfg = &cfgs[0];
        assert_eq!(cfg.is_signer, Some(true));
        assert_eq!(cfg.is_writable, Some(true));
    }

    /// 1.13 – unknown attribute triggers an error.
    #[test]
    fn parser_rejects_unknown_attribute() {
        let di: DeriveInput = parse_quote! {
            struct Unknown<'info> {
                #[account(foo)]
                acc: Account<'info, u64>,
            }
        };
        let err = parser::parse_fields(extract_named_fields(&di)).unwrap_err();
        assert!(err.to_string().contains("Unknown flag"));
    }

    /// 1.3 – slice without `len` should be rejected until dynamic slices are supported.
    #[test]
    fn parser_rejects_slice_without_len() {
        let di: DeriveInput = parse_quote! {
            struct Slice<'info> {
                pdas: Vec<AccountInfo<'static>>,
            }
        };
        let err = parser::parse_fields(extract_named_fields(&di)).unwrap_err();
        assert!(err.to_string().contains("vector field requires"));
    }

    /// 1.8 / 1.14 – duplicate attribute keys should yield an error.
    #[test]
    fn parser_rejects_duplicate_attributes() {
        let di: DeriveInput = parse_quote! {
            struct Dup<'info> {
                #[account(len = 2, len = 3)]
                pdas: Vec<AccountInfo<'static>>,
            }
        };
        let err = parser::parse_fields(extract_named_fields(&di)).unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    /// 1.9 – unsupported base type (e.g., u64) should be rejected.
    #[test]
    fn parser_rejects_unsupported_base_type() {
        let di: DeriveInput = parse_quote! {
            struct Bad<'info> {
                value: u64,
            }
        };
        let err = parser::parse_fields(extract_named_fields(&di)).unwrap_err();
        assert!(err.to_string().contains("unsupported"));
    }

    /// 1.15 – tuple struct should be rejected as parser expects named fields.
    #[test]
    #[should_panic]
    fn parser_rejects_tuple_struct() {
        let di: DeriveInput = parse_quote! {
            struct Tup<'info>(#[account(signer)] Account<'info, u64>);
        };
        // Attempting to extract named fields will panic;
        let _ = extract_named_fields(&di);
    }

    /// 1.16 – enum input should be rejected.
    #[test]
    #[should_panic]
    fn parser_rejects_enum() {
        let di: DeriveInput = parse_quote! {
            enum E<'info> {
                Variant {
                    #[account(signer)]
                    acc: Account<'info, u64>,
                }
            }
        };
        let err = parser::parse_fields(extract_named_fields(&di)).unwrap_err();
        assert!(err.to_string().contains("structs"));
    }

    /// 1.x – parser accepts `init_if_needed` attribute.
    #[test]
    fn parser_accepts_init_if_needed() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(signer)]
                payer: Account<'info, u64>,
                #[account(init_if_needed, payer = payer, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
                maybe_new: Account<'info, u64>,
            }
        };
        let cfgs = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        assert_eq!(cfgs.len(), 2);
        assert!(cfgs[1].is_init_if_needed);
    }

    /// 1.x – bump placeholder field should parse as FieldKind::Bump and not consume accounts.
    #[test]
    fn parser_accepts_bump_placeholder_field() {
        let di: DeriveInput = parse_quote! {
            struct BumpTest<'info> {
                #[account(seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
                pda: Account<'info, u64>,
                #[account(bump, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
                pda_bump: u8,
            }
        };

        let cfgs = parser::parse_fields(extract_named_fields(&di)).expect("parse should succeed");
        assert_eq!(cfgs.len(), 2);
        assert!(cfgs
            .iter()
            .any(|c| matches!(c.kind, model::FieldKind::Bump)));
    }

    /// 1.x – parser accepts a field marked `realloc` with required attributes.
    #[test]
    fn parser_accepts_realloc() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(signer)]
                payer: Account<'info, u64>,
                #[account(realloc, payer = payer, space = 16)]
                data: Account<'info, u64>,
            }
        };

        let cfgs = parser::parse_fields(extract_named_fields(&di)).expect("parse should succeed");
        assert_eq!(cfgs.len(), 2);
        let data_cfg = &cfgs[1];
        assert!(data_cfg.is_realloc, "realloc flag should be set");
        assert!(
            data_cfg.space.is_some(),
            "space attribute should be captured"
        );
    }

    /// 1.y – parser accepts bump placeholder declared as `[u8; 1]` array.
    #[test]
    fn parser_accepts_bump_array_placeholder() {
        let di: DeriveInput = parse_quote! {
            struct BumpArr<'info> {
                #[account(bump, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
                pda_bump: [u8; 1],
            }
        };
        let cfgs = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        assert!(cfgs
            .iter()
            .any(|c| matches!(c.kind, model::FieldKind::Bump)));
    }

    /// 1.z – parser accepts bump placeholder declared as reference to `[u8; 1]`.
    #[test]
    fn parser_accepts_bump_ref_array_placeholder() {
        let di: DeriveInput = parse_quote! {
            struct BumpRef<'info> {
                #[account(bump, seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default())]
                pda_bump: &'info [u8; 1],
            }
        };
        let cfgs = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        assert!(cfgs
            .iter()
            .any(|c| matches!(c.kind, model::FieldKind::Bump)));
    }

    /// parser accepts `of = MyShard` and validates match
    #[test]
    fn parser_accepts_of_param_and_matches_type() {
        let di: DeriveInput = parse_quote! {
            struct Test<'info> {
                #[account(shards, of = MyShard, len = 2)]
                shards: Vec<saturn_account_shards::ShardHandle<'info, MyShard>>,
            }
        };
        let cfgs = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        assert!(matches!(cfgs[0].kind, model::FieldKind::Shards(..)));
        assert!(cfgs[0].of_type.is_some());
    }

    /// parser rejects when `of` mismatches element type
    #[test]
    fn parser_rejects_of_param_mismatch() {
        let di: DeriveInput = parse_quote! {
            struct Test<'info> {
                #[account(shards, of = WrongShard, len = 1)]
                shards: Vec<saturn_account_shards::ShardHandle<'info, RightShard>>,
            }
        };
        let err = parser::parse_fields(extract_named_fields(&di)).unwrap_err();
        assert!(err.to_string().contains("does not match element type"));
    }
}
