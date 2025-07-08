use saturn_account_shards::{declare_fixed_array, declare_fixed_option, StateShard};
use saturn_account_shards_derive::ShardAccount;
use saturn_bitcoin_transactions::utxo_info::{SingleRuneSet, UtxoInfo};

// Declare minimal UTXO collection types for the test
declare_fixed_array!(
    TestBtcUtxos,
    saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
    4
);
declare_fixed_option!(
    TestRuneUtxo,
    saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
    15
);

// The struct derives ShardAccount which in turn relies on StateShard implementation.
#[derive(ShardAccount, Copy, Clone)]
#[shard(rune_set_type = "SingleRuneSet")]
#[repr(C)]
pub struct MyShardAccount {
    pub btc_utxos: TestBtcUtxos,
    pub rune_utxo: TestRuneUtxo,
}

fn main() {
    // Ensure that the generated impls are usable in code.
    let mut shard = MyShardAccount {
        btc_utxos: TestBtcUtxos::default(),
        rune_utxo: TestRuneUtxo::default(),
    };

    // The ShardAccount derive delegates to StateShard trait, so we can call its methods.
    assert_eq!(shard.btc_utxos_len(), 0);

    // Use a dummy UtxoInfo to exercise add_btc_utxo.
    let utxo: UtxoInfo<SingleRuneSet> = Default::default();
    let _ = shard.add_btc_utxo(utxo);
}
