mod common;
use common::*;
// use saturn_account_shards::{ShardSet, StateShard};
// use saturn_bitcoin_transactions::utxo_info::{SingleRuneSet, UtxoInfo};
use saturn_account_parser::codec::zero_copy::AccountLoader;
use saturn_account_shards::ShardSet;

// `ShardAccount` derive already provides `Zeroable` and `Pod` for `DefaultShard`.

// #[test]
// fn test_shard_set_integration() {
//     let mut shard1 = DefaultShard {
//         pool_id: 1,
//         liquidity: 1000,
//         btc_utxos: TestBtcUtxos::default(),
//         rune_utxo: TestRuneUtxo::default(),
//     };
//
//     let mut shard2 = DefaultShard {
//         pool_id: 2,
//         liquidity: 2000,
//         btc_utxos: TestBtcUtxos::default(),
//         rune_utxo: TestRuneUtxo::default(),
//     };
//
//     // Add different amounts to each shard
//     shard1.add_btc_utxo(create_test_utxo(10000, 1, 0));
//     shard2.add_btc_utxo(create_test_utxo(50000, 2, 0));
//
//     // Create a shard set (legacy API)
//     let mut shard_refs = vec![&mut shard1, &mut shard2];
//     let shard_set =
//         ShardSet::<SingleRuneSet, UtxoInfo<SingleRuneSet>, DefaultShard, 2>::new(&mut shard_refs);
//
//     // Basic properties
//     assert_eq!(shard_set.len(), 2);
//
//     // Select minimum by total BTC (should pick shard1)
//     let selected = shard_set
//         .select_min_by(|shard| shard.total_btc().to_sat())
//         .unwrap();
//
//     assert_eq!(selected.get_shard_by_index(0).pool_id, 1);
// }

#[test]
fn shard_set_compile_check() {
    // Create an empty slice of AccountLoaders for compile-time checking.
    let loaders: &[&AccountLoader<'_, DefaultShard>] = &[];
    let shard_set = ShardSet::<DefaultShard, 2>::from_loaders(loaders);

    // Sanity checks on the basic API.
    assert_eq!(shard_set.len(), 0);
    assert!(shard_set.is_empty());
}
