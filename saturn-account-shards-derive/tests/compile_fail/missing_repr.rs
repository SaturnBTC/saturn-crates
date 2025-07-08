use saturn_account_shards_derive::ShardAccount;
use saturn_account_shards::{declare_fixed_array, declare_fixed_option};
use saturn_bitcoin_transactions::utxo_info::SingleRuneSet;

declare_fixed_array!(TestBtcUtxos, saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>, 4);
declare_fixed_option!(TestRuneUtxo, saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>, 15);

#[derive(ShardAccount, Copy, Clone)]
#[shard(rune_set_type = "SingleRuneSet")]
// NOTE: Intentionally omitting `#[repr(C)]` to trigger a compile-time error.
pub struct BadShard {
    pub btc_utxos: TestBtcUtxos,
    pub rune_utxo: TestRuneUtxo,
}

fn main() {} 