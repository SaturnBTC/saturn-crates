#![cfg(feature = "runes")]

mod common;
use arch_program::rune::RuneId;
use common::*;
use saturn_account_shards::StateShard;

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
    assert_eq!(rune_amount.id, RuneId::new(1, 1));
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
