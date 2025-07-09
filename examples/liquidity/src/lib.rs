use saturn_program_macros::saturn_program;

mod instruction;
mod state;

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
        initialize_pool::{InitializePoolAccounts, InitializePoolUtxos},
        instruction::initialize_pool::InitializePoolParams,
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
