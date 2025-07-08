mod common;
use common::*;
use saturn_account_shards::StateShard;

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
