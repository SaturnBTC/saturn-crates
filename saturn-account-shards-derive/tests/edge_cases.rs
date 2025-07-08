mod common;
use common::*;
use saturn_account_shards::StateShard;

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
