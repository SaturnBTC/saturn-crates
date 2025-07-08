mod common;
use common::*;
use saturn_account_shards::StateShard;

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
