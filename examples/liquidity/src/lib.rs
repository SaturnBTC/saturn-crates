use arch_program::pubkey::Pubkey;
use saturn_account_macros::Accounts;
use saturn_account_parser::{Account, AccountLoader};
use saturn_program_macros::{declare_id, saturn_program};

mod instruction;
mod state;

declare_id!("8YE2m8RGmFjyWkHfMV6aA1eeaoAj8ZqEXnoY6v1WKEwd");

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
    use saturn_utxo_parser::TryFromUtxos;

    use crate::{
        initialize_pool::InitializePoolUtxos, instruction::initialize_pool::InitializePoolParams,
    };

    pub fn initialize_pool<'info>(
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

        for shard_acc in &ctx.accounts.shards {
            shard_acc
                .load_init()
                .expect("handle this error")
                .initialize();
        }

        Ok(())
    }

    pub fn add_pool_shards<'info>(
        ctx: Context<'info, InitializePoolAccounts<'info>>,
        params: u8,
    ) -> Result<(), ProgramError> {
        Ok(())
    }

    pub fn increase_liquidity<'info>(
        ctx: Context<'info, InitializePoolAccounts<'info>>,
        params: u8,
    ) -> Result<(), ProgramError> {
        Ok(())
    }
}

pub use handlers::*;
pub use instruction::*;
pub use state::*;
