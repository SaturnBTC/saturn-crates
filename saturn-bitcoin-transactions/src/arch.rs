use arch_program::{
    account::{AccountInfo, MIN_ACCOUNT_LAMPORTS},
    program::{get_bitcoin_tx_output_value, get_runes_from_output, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rune::RuneAmount,
    system_instruction::create_account_with_anchor as create_account_instruction,
    utxo::UtxoMeta,
};
use bitcoin::Transaction;

use crate::{bytes::txid_to_bytes_big_endian, error::BitcoinTxError};

pub fn create_account<'a>(
    utxo: &UtxoMeta,
    account: &AccountInfo<'a>,
    system_program_id: &AccountInfo<'a>,
    fee_payer: &AccountInfo<'a>,
    program_id: &Pubkey,
    space: u64,
    signer_seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    let cpi_signer_seeds: &[&[&[u8]]] = &[signer_seeds];

    let instruction = create_account_instruction(
        fee_payer.key,
        account.key,
        MIN_ACCOUNT_LAMPORTS,
        space,
        program_id,
        utxo.txid_big_endian(),
        utxo.vout(),
    );

    invoke_signed(
        &instruction,
        &[
            account.clone(),
            fee_payer.clone(),
            system_program_id.clone(),
        ],
        cpi_signer_seeds,
    )?;

    Ok(())
}

pub(crate) fn get_amount_in_tx_inputs(tx: &Transaction) -> Result<u64, BitcoinTxError> {
    let mut amount = 0;

    for input in tx.input.iter() {
        let outpoint = input.previous_output;
        let value: u64 =
            get_bitcoin_tx_output_value(txid_to_bytes_big_endian(&outpoint.txid), outpoint.vout)
                .ok_or(BitcoinTxError::TransactionNotFound(
                    outpoint.txid.to_string(),
                ))?;

        amount += value;
    }

    Ok(amount)
}

pub fn get_rune(utxo: &UtxoMeta) -> Result<Option<RuneAmount>, ProgramError> {
    let txid = utxo.txid_big_endian();

    let runes = get_runes_from_output(txid, utxo.vout()).ok_or(ProgramError::Custom(
        BitcoinTxError::RuneOutputNotFound.into(),
    ))?;

    if runes.is_empty() {
        Ok(None)
    } else {
        if runes.len() > 1 {
            return Err(ProgramError::Custom(
                BitcoinTxError::MultipleRunesInUtxo.into(),
            ));
        }

        Ok(Some(runes[0]))
    }
}
