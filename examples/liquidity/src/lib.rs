use arch_program::{pubkey::Pubkey, rune::RuneId, utxo::UtxoMeta};
use borsh::{BorshDeserialize, BorshSerialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::{Account, AccountLoader};
use saturn_bitcoin_transactions::utxo_info::UtxoInfo;
use saturn_program_macros::{declare_id, saturn_program};

mod state;

declare_id!("8YE2m8RGmFjyWkHfMV6aA1eeaoAj8ZqEXnoY6v1WKEwd");

// ===== Initialize Pool

#[derive(Accounts)]
pub struct InitializePoolAccounts<'info> {
    #[account(signer, mut)]
    pub caller: Account<'info, u64>,

    #[account(
        seeds = &[b"CONFIG"],
        program_id = Pubkey::default(),
        init,
        payer = caller,
        mut,
        zero_copy,
        of = LiquidityPoolConfig,
    )]
    pub config: AccountLoader<'info, LiquidityPoolConfig>,

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
        mut,
        // zero_copy,
        // of = LiquidityPoolShard
    )]
    pub shards: Vec<AccountLoader<'info, LiquidityPoolShard>>,
}

#[derive(UtxoParser)]
#[utxo_accounts(InitializePoolAccounts)]
pub struct InitializePoolUtxos {
    #[utxo(value = 10_000, runes = "none")]
    fee: UtxoInfo,

    #[utxo(anchor = config)]
    config: UtxoInfo,

    #[utxo(rest)]
    shards: Vec<UtxoInfo>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct InitializePoolParams {
    pub token_0: RuneId,
    pub token_1: RuneId,
    pub utxos: Vec<UtxoMeta>,
}

// ===== Add Pool Shards

#[derive(Accounts)]
pub struct AddPoolShardsAccounts<'info> {
    #[account(signer)]
    pub caller: Account<'info, u64>,

    #[account(
        seeds = &[b"CONFIG"],
        program_id = Pubkey::default(),
        zero_copy,
        of = LiquidityPoolConfig,
    )]
    pub config: AccountLoader<'info, LiquidityPoolConfig>,

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
    )]
    pub shards: Vec<AccountLoader<'info, LiquidityPoolShard>>,
}

// ===== Increase Liquidity

#[derive(Accounts)]
pub struct IncreaseLiquidityAccounts<'info> {
    #[account(signer, mut)]
    pub caller: Account<'info, u64>,

    #[account(
        seeds = &[b"CONFIG"],
        program_id = Pubkey::default(),
        zero_copy,
        of = LiquidityPoolConfig,
    )]
    pub config: AccountLoader<'info, LiquidityPoolConfig>,

    #[account(
        bump,
        seeds = &[b"CONFIG"],
        program_id = Pubkey::default(),
    )]
    pub config_bump: [u8; 1],

    #[account(
        seeds = &[b"POSITION", &caller.info().key.0],
        program_id = Pubkey::default(),
        init,
        payer = caller,
        mut,
        zero_copy,
        of = LiquidityPoolConfig,
    )]
    pub position: AccountLoader<'info, LiquidityPosition>,

    #[account(
        bump,
        seeds = &[b"POSITION", &caller.info().key.0],
        program_id = Pubkey::default(),
    )]
    pub position_bump: [u8; 1],

    #[account(
        shards,
        len = 50,
        seeds = &[b"SHARD"],
        program_id = Pubkey::default()
    )]
    pub shards: Vec<AccountLoader<'info, LiquidityPoolShard>>,
}

#[saturn_program(btc_tx_cfg(
    max_inputs_to_sign = 11,
    max_modified_accounts = 11,
    rune_capacity = 1
))]
mod handlers {
    use super::*;
    use arch_program::{program::get_bitcoin_block_height, program_error::ProgramError};
    use saturn_account_parser::Context;
    use saturn_account_shards::ShardSet;
    use saturn_utxo_parser::TryFromUtxos;

    pub fn initialize_pool(
        ctx: Context<'info, InitializePoolAccounts<'info>>,
        params: InitializePoolParams,
    ) -> Result<(), ProgramError> {
        let utxos = InitializePoolUtxos::try_utxos(ctx.accounts, &params.utxos).unwrap();

        let current_block_height = get_bitcoin_block_height();

        if params.token_0.block > current_block_height - 6 {
            panic!("This should be an error");
        }

        ctx.accounts
            .config
            .load_init()
            .expect("handle this error")
            .initialize(params.token_0, params.token_1, 10.);

        let shard_set: ShardSet<'_, LiquidityPoolShard, 10> =
            ShardSet::from_loaders(&ctx.accounts.shards);

        // for shard_acc in &ctx.accounts.shards {
        //     shard_acc
        //         .load_init()
        //         .expect("handle this error")
        //         .initialize();
        // }

        Ok(())
    }

    pub fn add_pool_shards(
        ctx: Context<'info, InitializePoolAccounts<'info>>,
        params: u8,
    ) -> Result<(), ProgramError> {
        Ok(())
    }

    pub fn increase_liquidity(
        ctx: Context<'info, InitializePoolAccounts<'info>>,
        params: u8,
    ) -> Result<(), ProgramError> {
        Ok(())
    }
}

pub use handlers::*;
use saturn_utxo_parser::UtxoParser;
pub use state::*;
