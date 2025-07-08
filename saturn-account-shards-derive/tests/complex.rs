mod common;
use common::*;
use saturn_account_shards::StateShard;

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
