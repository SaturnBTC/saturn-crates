//! Integration tests for the StateShard derive macro.
//!
//! These tests verify that the macro works correctly with actual types
//! and can generate working implementations for real-world use cases.

use saturn_account_shards::declare_shard_utxo_types;
use saturn_account_shards::{Result, StateShard};
use saturn_account_shards_derive::StateShard;
use saturn_bitcoin_transactions::utxo_info::{SingleRuneSet, UtxoInfo};

// Declare UTXO collection types for testing
declare_shard_utxo_types!(
    SingleRuneSet,
    TestBtcUtxos,
    TestRuneUtxo,
    10, // Can hold up to 10 BTC UTXOs
    15  // Padding for alignment
);

// Basic struct using default field names
#[derive(StateShard, Debug, Clone)]
#[repr(C)]
pub struct DefaultShard {
    pub pool_id: u64,
    pub liquidity: u128,
    pub btc_utxos: TestBtcUtxos,
    pub rune_utxo: TestRuneUtxo,
}

// Custom struct with non-default field names
#[derive(StateShard, Debug, Clone)]
#[shard(btc_utxos_attr = "bitcoin_utxos", rune_utxo_attr = "rune_utxo_data")]
#[repr(C)]
pub struct CustomFieldShard {
    pub pool_id: u64,
    pub bitcoin_utxos: TestBtcUtxos,
    pub rune_utxo_data: TestRuneUtxo,
}

// Complex struct with additional fields
#[derive(StateShard, Debug, Clone)]
#[repr(C)]
pub struct ComplexShard {
    pub pool_id: u64,
    pub liquidity: u128,
    pub fee_rate: u32,
    pub last_update: u64,
    pub btc_utxos: TestBtcUtxos,
    pub rune_utxo: TestRuneUtxo,
    pub metadata: [u8; 32],
    pub extra_data: u64,
}

// Helper function to create test UTXOs
fn create_test_utxo(value: u64, _txid_byte: u8, _vout: u32) -> UtxoInfo<SingleRuneSet> {
    UtxoInfo {
        value,
        ..Default::default()
    }
}

#[cfg(feature = "runes")]
fn create_test_rune_utxo(
    value: u64,
    _txid_byte: u8,
    _vout: u32,
    rune_amount: u128,
) -> UtxoInfo<SingleRuneSet> {
    let mut runes = SingleRuneSet::default();
    runes
        .insert(RuneAmount {
            amount: rune_amount,
            ..Default::default()
        })
        .unwrap();
    UtxoInfo {
        value,
        runes,
        ..Default::default()
    }
}

mod basic_functionality {
    use super::*;

    #[test]
    fn test_default_shard_btc_utxos() {
        let mut shard = DefaultShard {
            pool_id: 1,
            liquidity: 1000,
            btc_utxos: TestBtcUtxos::default(),
            rune_utxo: TestRuneUtxo::default(),
        };

        // Test btc_utxos_len and btc_utxos_max_len
        assert_eq!(shard.btc_utxos_len(), 0);
        assert_eq!(shard.btc_utxos_max_len(), 10);

        // Test add_btc_utxo
        let utxo1 = create_test_utxo(50000, 1, 0);
        let utxo2 = create_test_utxo(75000, 2, 0);

        assert_eq!(shard.add_btc_utxo(utxo1), Some(0));
        assert_eq!(shard.btc_utxos_len(), 1);
        assert_eq!(shard.add_btc_utxo(utxo2), Some(1));
        assert_eq!(shard.btc_utxos_len(), 2);

        // Test btc_utxos access
        let btc_utxos = shard.btc_utxos();
        assert_eq!(btc_utxos.len(), 2);
        assert_eq!(btc_utxos[0].value, 50000);
        assert_eq!(btc_utxos[1].value, 75000);

        // Test btc_utxos_mut
        let btc_utxos_mut = shard.btc_utxos_mut();
        btc_utxos_mut[0].value = 55000;
        assert_eq!(shard.btc_utxos()[0].value, 55000);
    }

    #[test]
    fn test_default_shard_rune_utxo() {
        let mut shard = DefaultShard {
            pool_id: 1,
            liquidity: 1000,
            btc_utxos: TestBtcUtxos::default(),
            rune_utxo: TestRuneUtxo::default(),
        };

        // Test rune_utxo initially None
        assert!(shard.rune_utxo().is_none());
        assert!(shard.rune_utxo_mut().is_none());

        // Test set_rune_utxo
        let rune_utxo = create_test_utxo(25000, 3, 0);
        shard.set_rune_utxo(rune_utxo);

        assert!(shard.rune_utxo().is_some());
        assert_eq!(shard.rune_utxo().unwrap().value, 25000);

        // Test rune_utxo_mut
        if let Some(rune_utxo_mut) = shard.rune_utxo_mut() {
            rune_utxo_mut.value = 30000;
        }
        assert_eq!(shard.rune_utxo().unwrap().value, 30000);

        // Test clear_rune_utxo
        shard.clear_rune_utxo();
        assert!(shard.rune_utxo().is_none());
    }

    #[test]
    fn test_btc_utxos_retain() {
        let mut shard = DefaultShard {
            pool_id: 1,
            liquidity: 1000,
            btc_utxos: TestBtcUtxos::default(),
            rune_utxo: TestRuneUtxo::default(),
        };

        // Add multiple UTXOs
        shard.add_btc_utxo(create_test_utxo(10000, 1, 0));
        shard.add_btc_utxo(create_test_utxo(50000, 2, 0));
        shard.add_btc_utxo(create_test_utxo(25000, 3, 0));
        shard.add_btc_utxo(create_test_utxo(75000, 4, 0));

        assert_eq!(shard.btc_utxos_len(), 4);

        // Retain only UTXOs with value > 30000
        shard.btc_utxos_retain(&mut |utxo| utxo.value > 30000);

        assert_eq!(shard.btc_utxos_len(), 2);
        assert_eq!(shard.btc_utxos()[0].value, 50000);
        assert_eq!(shard.btc_utxos()[1].value, 75000);
    }

    #[test]
    fn test_capacity_limits() {
        let mut shard = DefaultShard {
            pool_id: 1,
            liquidity: 1000,
            btc_utxos: TestBtcUtxos::default(),
            rune_utxo: TestRuneUtxo::default(),
        };

        // Fill to capacity
        for i in 0..10 {
            let result = shard.add_btc_utxo(create_test_utxo(1000, i as u8, i));
            assert_eq!(result, Some(i as usize));
        }

        assert_eq!(shard.btc_utxos_len(), 10);
        assert_eq!(shard.btc_utxos_max_len(), 10);

        // Try to add one more - should fail
        let result = shard.add_btc_utxo(create_test_utxo(1000, 11, 10));
        assert_eq!(result, None);
        assert_eq!(shard.btc_utxos_len(), 10);
    }

    #[test]
    fn test_total_btc_calculation() {
        let mut shard = DefaultShard {
            pool_id: 1,
            liquidity: 1000,
            btc_utxos: TestBtcUtxos::default(),
            rune_utxo: TestRuneUtxo::default(),
        };

        // Add UTXOs with known values
        shard.add_btc_utxo(create_test_utxo(25000, 1, 0));
        shard.add_btc_utxo(create_test_utxo(50000, 2, 0));
        shard.add_btc_utxo(create_test_utxo(75000, 3, 0));

        // total_btc() is provided by the StateShard trait
        let total = shard.total_btc();
        assert_eq!(total.to_sat(), 150000);
    }
}

mod custom_fields {
    use super::*;

    #[test]
    fn test_custom_field_names() {
        let mut shard = CustomFieldShard {
            pool_id: 1,
            bitcoin_utxos: TestBtcUtxos::default(),
            rune_utxo_data: TestRuneUtxo::default(),
        };

        // Test that the macro generated correct field access
        assert_eq!(shard.btc_utxos_len(), 0);
        assert!(shard.rune_utxo().is_none());

        // Add BTC UTXO using custom field
        let utxo = create_test_utxo(100000, 1, 0);
        assert_eq!(shard.add_btc_utxo(utxo), Some(0));
        assert_eq!(shard.btc_utxos_len(), 1);
        assert_eq!(shard.btc_utxos()[0].value, 100000);

        // Add rune UTXO using custom field
        let rune_utxo = create_test_utxo(50000, 2, 0);
        shard.set_rune_utxo(rune_utxo);
        assert!(shard.rune_utxo().is_some());
        assert_eq!(shard.rune_utxo().unwrap().value, 50000);
    }
}

mod complex_struct {
    use super::*;

    #[test]
    fn test_complex_struct_with_additional_fields() {
        let mut shard = ComplexShard {
            pool_id: 42,
            liquidity: 5000000,
            fee_rate: 100,
            last_update: 1234567890,
            btc_utxos: TestBtcUtxos::default(),
            rune_utxo: TestRuneUtxo::default(),
            metadata: [0u8; 32],
            extra_data: 999,
        };

        // Test that StateShard methods work correctly despite additional fields
        assert_eq!(shard.btc_utxos_len(), 0);
        assert_eq!(shard.btc_utxos_max_len(), 10);

        // Add UTXOs
        shard.add_btc_utxo(create_test_utxo(30000, 1, 0));
        shard.add_btc_utxo(create_test_utxo(70000, 2, 0));

        assert_eq!(shard.btc_utxos_len(), 2);
        assert_eq!(shard.total_btc().to_sat(), 100000);

        // Test that other fields are unchanged
        assert_eq!(shard.pool_id, 42);
        assert_eq!(shard.liquidity, 5000000);
        assert_eq!(shard.fee_rate, 100);
        assert_eq!(shard.last_update, 1234567890);
        assert_eq!(shard.extra_data, 999);
    }
}

#[cfg(feature = "runes")]
mod rune_functionality {
    use super::*;

    #[test]
    fn test_rune_utxo_operations() {
        let mut shard = DefaultShard {
            pool_id: 1,
            liquidity: 1000,
            btc_utxos: TestBtcUtxos::default(),
            rune_utxo: TestRuneUtxo::default(),
        };

        // Create a UTXO with runes
        let rune_utxo = create_test_rune_utxo(25000, 1, 0, 5000);

        // Test set_rune_utxo with runes
        shard.set_rune_utxo(rune_utxo);

        assert!(shard.rune_utxo().is_some());
        let stored_rune_utxo = shard.rune_utxo().unwrap();
        assert_eq!(stored_rune_utxo.value, 25000);

        // Check that runes are preserved
        assert_eq!(stored_rune_utxo.runes.len(), 1);
        let rune_amount = stored_rune_utxo.runes.as_slice()[0];
        assert_eq!(rune_amount.amount, 5000);
    }

    #[test]
    fn test_btc_utxos_with_runes() {
        let mut shard = DefaultShard {
            pool_id: 1,
            liquidity: 1000,
            btc_utxos: TestBtcUtxos::default(),
            rune_utxo: TestRuneUtxo::default(),
        };

        // Add BTC UTXOs with runes
        let utxo_with_runes = create_test_rune_utxo(50000, 1, 0, 2500);
        let utxo_without_runes = create_test_utxo(75000, 2, 0);

        shard.add_btc_utxo(utxo_with_runes);
        shard.add_btc_utxo(utxo_without_runes);

        assert_eq!(shard.btc_utxos_len(), 2);

        // Check first UTXO has runes
        let first_utxo = &shard.btc_utxos()[0];
        assert_eq!(first_utxo.value, 50000);
        assert_eq!(first_utxo.runes.len(), 1);

        // Check second UTXO has no runes
        let second_utxo = &shard.btc_utxos()[1];
        assert_eq!(second_utxo.value, 75000);
        assert_eq!(second_utxo.runes.len(), 0);
    }
}

mod edge_cases {
    use super::*;

    #[test]
    fn test_empty_operations() {
        let mut shard = DefaultShard {
            pool_id: 1,
            liquidity: 1000,
            btc_utxos: TestBtcUtxos::default(),
            rune_utxo: TestRuneUtxo::default(),
        };

        // Test operations on empty shard
        assert_eq!(shard.btc_utxos_len(), 0);
        assert_eq!(shard.btc_utxos().len(), 0);
        assert!(shard.rune_utxo().is_none());
        assert_eq!(shard.total_btc().to_sat(), 0);

        // Test retain on empty
        shard.btc_utxos_retain(&mut |_| true);
        assert_eq!(shard.btc_utxos_len(), 0);

        // Test clear on empty rune UTXO
        shard.clear_rune_utxo();
        assert!(shard.rune_utxo().is_none());
    }

    #[test]
    fn test_retain_all_filtered_out() {
        let mut shard = DefaultShard {
            pool_id: 1,
            liquidity: 1000,
            btc_utxos: TestBtcUtxos::default(),
            rune_utxo: TestRuneUtxo::default(),
        };

        // Add UTXOs
        shard.add_btc_utxo(create_test_utxo(10000, 1, 0));
        shard.add_btc_utxo(create_test_utxo(20000, 2, 0));
        shard.add_btc_utxo(create_test_utxo(30000, 3, 0));

        assert_eq!(shard.btc_utxos_len(), 3);

        // Filter out all UTXOs
        shard.btc_utxos_retain(&mut |_| false);

        assert_eq!(shard.btc_utxos_len(), 0);
        assert_eq!(shard.total_btc().to_sat(), 0);
    }

    #[test]
    fn test_retain_all_kept() {
        let mut shard = DefaultShard {
            pool_id: 1,
            liquidity: 1000,
            btc_utxos: TestBtcUtxos::default(),
            rune_utxo: TestRuneUtxo::default(),
        };

        // Add UTXOs
        shard.add_btc_utxo(create_test_utxo(10000, 1, 0));
        shard.add_btc_utxo(create_test_utxo(20000, 2, 0));
        shard.add_btc_utxo(create_test_utxo(30000, 3, 0));

        assert_eq!(shard.btc_utxos_len(), 3);

        // Keep all UTXOs
        shard.btc_utxos_retain(&mut |_| true);

        assert_eq!(shard.btc_utxos_len(), 3);
        assert_eq!(shard.total_btc().to_sat(), 60000);
    }

    #[test]
    fn test_multiple_rune_utxo_operations() {
        let mut shard = DefaultShard {
            pool_id: 1,
            liquidity: 1000,
            btc_utxos: TestBtcUtxos::default(),
            rune_utxo: TestRuneUtxo::default(),
        };

        // Set, clear, and set again
        let rune_utxo1 = create_test_utxo(25000, 1, 0);
        shard.set_rune_utxo(rune_utxo1);
        assert!(shard.rune_utxo().is_some());
        assert_eq!(shard.rune_utxo().unwrap().value, 25000);

        shard.clear_rune_utxo();
        assert!(shard.rune_utxo().is_none());

        let rune_utxo2 = create_test_utxo(50000, 2, 0);
        shard.set_rune_utxo(rune_utxo2);
        assert!(shard.rune_utxo().is_some());
        assert_eq!(shard.rune_utxo().unwrap().value, 50000);

        // Overwrite existing
        let rune_utxo3 = create_test_utxo(75000, 3, 0);
        shard.set_rune_utxo(rune_utxo3);
        assert!(shard.rune_utxo().is_some());
        assert_eq!(shard.rune_utxo().unwrap().value, 75000);
    }
}

mod memory_layout {
    use super::*;
    use std::mem;

    #[test]
    fn test_struct_memory_layout() {
        // Test that structs are aligned to at least 8 bytes (and thus safe for
        // zero-copy across common architectures). We avoid asserting an exact
        // value because the alignment of some primitive types (e.g. `u128`)
        // changed from 8 to 16 bytes on recent Rust versions / targets. Any
        // multiple of 8 ensures the layout remains ABI-stable for our use-case.
        assert_eq!(mem::align_of::<DefaultShard>() % 8, 0);
        assert_eq!(mem::align_of::<CustomFieldShard>() % 8, 0);
        assert_eq!(mem::align_of::<ComplexShard>() % 8, 0);

        // Test that the structs are of reasonable size
        let default_size = mem::size_of::<DefaultShard>();
        let custom_size = mem::size_of::<CustomFieldShard>();
        let complex_size = mem::size_of::<ComplexShard>();

        // They should be non-zero
        assert!(default_size > 0);
        assert!(custom_size > 0);
        assert!(complex_size > 0);

        // Complex struct should be larger due to additional fields
        assert!(complex_size > default_size);
    }
}

mod integration_with_shard_set {
    use super::*;
    use saturn_account_shards::ShardSet;

    #[test]
    fn test_shard_set_integration() {
        let mut shard1 = DefaultShard {
            pool_id: 1,
            liquidity: 1000,
            btc_utxos: TestBtcUtxos::default(),
            rune_utxo: TestRuneUtxo::default(),
        };

        let mut shard2 = DefaultShard {
            pool_id: 2,
            liquidity: 2000,
            btc_utxos: TestBtcUtxos::default(),
            rune_utxo: TestRuneUtxo::default(),
        };

        // Add different amounts to each shard
        shard1.add_btc_utxo(create_test_utxo(10000, 1, 0));
        shard2.add_btc_utxo(create_test_utxo(50000, 2, 0));

        // Create a shard set
        let mut shard_refs = vec![&mut shard1, &mut shard2];
        let shard_set = ShardSet::<SingleRuneSet, UtxoInfo<SingleRuneSet>, DefaultShard, 2>::new(
            &mut shard_refs,
        );

        // Test that we can work with the shard set
        assert_eq!(shard_set.len(), 2);

        // Test selecting minimum by total BTC
        let selected = shard_set
            .select_min_by(|shard| shard.total_btc().to_sat())
            .unwrap();

        // Should select the shard with less BTC (shard1)
        assert_eq!(selected.get_shard_by_index(0).pool_id, 1);
    }
}
