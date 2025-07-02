use arch_program::{account::AccountInfo, program_error::ProgramError};
use saturn_bitcoin_transactions::{
    constants::DUST_LIMIT, utxo_info::UtxoInfo, Accounts, InstructionUtxos,
};

use crate::{error::EscrowError, InstructionContext};

pub struct CreateEscrowUtxos {
    pub account_utxo: UtxoInfo,
    pub escrow_utxo: UtxoInfo,
}

impl<'a> InstructionUtxos<'a> for CreateEscrowUtxos {
    fn try_utxos(utxos: &'a [UtxoInfo]) -> Result<Self, ProgramError> {
        if utxos.len() != 2 {
            return Err(EscrowError::InvalidUtxoCount.into());
        }

        let mut account_utxo = None;
        let mut escrow_utxo = None;

        for utxo in utxos {
            if utxo.runes.is_some() {
                return Err(EscrowError::InvalidUtxoContainingRunes.into());
            }

            if utxo.value == DUST_LIMIT {
                account_utxo = Some(*utxo);
            } else {
                escrow_utxo = Some(*utxo);
            }
        }

        if account_utxo.is_none() || escrow_utxo.is_none() {
            return Err(EscrowError::NoAccountUtxoFound.into());
        }

        Ok(CreateEscrowUtxos {
            account_utxo: account_utxo.unwrap(),
            escrow_utxo: escrow_utxo.unwrap(),
        })
    }
}

pub struct CreateEscrowAccounts<'a> {
    pub caller: &'a AccountInfo<'static>,
    pub system_program: &'a AccountInfo<'static>,
    pub fee_payer: &'a AccountInfo<'static>,
    pub escrow_account: &'a AccountInfo<'static>,
}

impl<'a> Accounts<'a> for CreateEscrowAccounts<'a> {
    fn try_accounts(accounts: &'a [AccountInfo<'static>]) -> Result<Self, ProgramError> {
        if accounts.len() != 4 {
            return Err(EscrowError::InvalidAccountCount.into());
        }

        Ok(CreateEscrowAccounts {
            caller: &accounts[0],
            system_program: &accounts[1],
            fee_payer: &accounts[2],
            escrow_account: &accounts[3],
        })
    }
}

pub fn create_escrow(
    ctx: &mut InstructionContext,
    utxo: [UtxoInfo; 2],
) -> Result<(), ProgramError> {
    let utxos = CreateEscrowUtxos::try_utxos(&utxo)?;
    let acc = CreateEscrowAccounts::try_accounts(&ctx.accounts)?;

    let space = 1000;

    ctx.transaction_builder.create_state_account(
        &utxos.account_utxo,
        acc.system_program,
        acc.fee_payer,
        acc.escrow_account,
        &ctx.program_id,
        space,
        &[],
    )?;

    Ok(())
}
