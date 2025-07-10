use arch_program::rune::RuneId;
use bytemuck::{Pod, Zeroable};
use saturn_account_discriminator_derive::Discriminator;
use saturn_account_shards_derive::ShardAccount;
use saturn_bitcoin_transactions::utxo_info::{FixedArrayUtxoInfo, FixedOptionUtxoInfo, UtxoInfo};
use saturn_collections::generic::fixed_list::FixedList;

#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, Discriminator)]
#[repr(C)]
pub struct LiquidityPoolConfig {
    pub token_0: RuneId,

    pub token_1: RuneId,

    pub fee_rate: f64,
}

impl LiquidityPoolConfig {
    pub fn initialize(&mut self, token_0: RuneId, token_1: RuneId, fee_rate: f64) {
        self.token_0 = token_0;
        self.token_1 = token_1;
        self.fee_rate = fee_rate;
    }
}

#[derive(Clone, Copy, Debug, Default, ShardAccount)]
#[repr(C)]
pub struct LiquidityPoolShard {
    liquidity: u128,

    btc_utxos: FixedArrayUtxoInfo,
    rune_utxo: FixedOptionUtxoInfo,
}

impl LiquidityPoolShard {
    pub fn initialize(&mut self) {
        self.liquidity = 0;
    }
}

#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, Discriminator)]
#[repr(C)]
pub struct LiquidityPosition {
    liquidity: u128,
}

impl LiquidityPosition {
    pub fn initialize(&mut self) {
        self.liquidity = 0;
    }
}
