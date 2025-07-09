use borsh::{BorshDeserialize, BorshSerialize};

use crate::instruction::initialize_pool::InitializePoolParams;

pub mod add_pool_shards;
pub mod increase_liquidity;
pub mod initialize_pool;

#[derive(BorshSerialize, BorshDeserialize)]
pub enum LiquidityInstruction {
    InitializePool(InitializePoolParams),
    AddPoolShards(u8),
    IncreaseLiquidity(u8),
}
