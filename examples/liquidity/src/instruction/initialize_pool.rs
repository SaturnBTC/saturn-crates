use arch_program::{pubkey::Pubkey, rune::RuneId};
use borsh::{BorshDeserialize, BorshSerialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::{
    codec::{BorshAccount, ZeroCopyAccount},
    AccountLoader,
};
use saturn_bitcoin_transactions::utxo_info::UtxoInfo;
use saturn_utxo_parser::UtxoParser;

use crate::state::{LiquidityPoolConfig, LiquidityPoolShard};

#[derive(Accounts)]
pub struct InitializePoolAccounts<'info> {
    #[account(signer)]
    pub caller: BorshAccount<'info, u64>,

    #[account(
        seeds = &[b"CONFIG"],
        program_id = Pubkey::default(),
        init,
        payer = caller,
    )]
    pub config: ZeroCopyAccount<'info, LiquidityPoolConfig>,

    #[account(
        bump,
        seeds = &[b"CONFIG"],
        program_id = Pubkey::default(),
    )]
    pub config_bump: [u8; 1],

    #[account(
        shards,
        len = 10,
        seeds = &[b"SHARD"],
        program_id = Pubkey::default(),
        init,
        payer = caller,
    )]
    pub shards: Vec<AccountLoader<'info, LiquidityPoolShard>>,
}

#[derive(UtxoParser)]
#[utxo_accounts(InitializePoolAccounts)]
pub struct InitializePoolUtxos<'a> {
    #[utxo(value = 10_000, runes = "none")]
    fee: &'a UtxoInfo,

    #[utxo(anchor = config)]
    config: &'a UtxoInfo,

    #[utxo(rest)]
    shards: Vec<&'a UtxoInfo>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct InitializePoolParams {
    pub token_0: RuneId,
    pub token_1: RuneId,
    pub utxos: Vec<UtxoInfo>,
}
