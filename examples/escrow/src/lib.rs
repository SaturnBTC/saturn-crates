use arch_program::utxo::UtxoMeta;
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::{Account, AccountLoader};
use saturn_bitcoin_transactions::utxo_info::UtxoInfo;
use saturn_program_macros::{declare_id, saturn_program};
use saturn_utxo_parser::UtxoParser;

declare_id!("8YE2m8RGmFjyWkHfMV6aA1eeaoAj8ZqEXnoY6v1WKEwd");

#[derive(Accounts)]
struct DepositAccounts<'info> {
    #[account(signer, mut)]
    caller: Account<'info, u64>,

    #[account(
        init_if_needed,
        mut,
        payer = caller,
        seeds = &[b""], 
        program_id = arch_program::pubkey::Pubkey::default()
    )]
    escrow_utxo: Account<'info, UtxoMeta>,

    #[account(
        bump,
        seeds = &[b""], 
        program_id = arch_program::pubkey::Pubkey::default()
    )]
    escrow_utxo_bump: [u8; 1],
}

#[derive(Accounts)]
struct WithdrawAccounts<'info> {
    #[account(signer, mut)]
    caller: Account<'info, u64>,

    #[account(
        seeds = &[b""], 
        program_id = arch_program::pubkey::Pubkey::default()
    )]
    escrow_utxo: Account<'info, UtxoMeta>,

    #[account(
        bump,
        seeds = &[b""], 
        program_id = arch_program::pubkey::Pubkey::default()
    )]
    escrow_utxo_bump: [u8; 1],
}

#[derive(Debug, UtxoParser)]
#[utxo_accounts(WithdrawAccounts)]
struct WithdrawUtxos {
    #[utxo(value = 10_000, runes = "none")]
    fee_utxo: UtxoInfo,

    #[utxo(anchor = escrow_utxo)]
    escrow_utxo_account_utxo: UtxoInfo,
}

#[saturn_program(btc_tx_cfg(max_inputs_to_sign = 4, max_modified_accounts = 4, rune_capacity = 1))]
mod handlers {
    use arch_program::{
        bitcoin::{Amount, ScriptBuf, TxOut},
        program::get_bitcoin_tx_output_value,
        program_error::ProgramError,
        utxo::UtxoMeta,
    };
    use mempool_oracle_sdk::TxStatus;
    use saturn_bitcoin_transactions::utxo_info::{SingleRuneSet, UtxoInfo};
    use saturn_utxo_parser::TryFromUtxos;

    use super::*;

    pub fn deposit<'info>(
        mut ctx: Context<'info, DepositAccounts<'info>>,
        params: UtxoMeta,
    ) -> Result<(), ProgramError> {
        // ctx.accounts.escrow_utxo = params;

        // ctx.btc_tx.0.add_state_transition(ctx.accounts.escrow_utxo.info()).unwrap();

        Ok(())
    }

    pub fn withdraw<'info>(
        mut ctx: Context<'info, WithdrawAccounts<'info>>,
        params: String,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        let funds_utxo_txid = ctx.accounts.escrow_utxo.txid_big_endian();
        let funds_utxo_vout = ctx.accounts.escrow_utxo.vout();

        let funds_utxo_val = get_bitcoin_tx_output_value(funds_utxo_txid, funds_utxo_vout).unwrap();

        let utxo_info = UtxoInfo {
            value: funds_utxo_val,
            meta: *ctx.accounts.escrow_utxo,
            runes: SingleRuneSet::default(),
            ..Default::default()
        };

        let utxos: Vec<UtxoMeta> = vec![*ctx.accounts.escrow_utxo];

        let parsed_utxos = WithdrawUtxos::try_utxos(&ctx.accounts, &utxos).unwrap();

        ctx.btc_tx
            .add_tx_input(&utxo_info, &TxStatus::Confirmed, ctx.program_id)
            .unwrap();

        let fee = 1_000;

        ctx.btc_tx.transaction.output.push(TxOut {
            script_pubkey: ScriptBuf::new(),
            value: Amount::from_sat(utxo_info.value - fee),
        });

        Ok(())
    }
}
