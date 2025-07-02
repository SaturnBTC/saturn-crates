#![deny(unused_must_use)]

use arch_program::{
    account::AccountInfo, program_error::ProgramError, pubkey::Pubkey, utxo::UtxoMeta,
};
use borsh::BorshDeserialize;
use saturn_bitcoin_transactions::utxo_info::UtxoInfo;
use saturn_bitcoin_transactions::TransactionBuilder;

#[cfg(not(test))]
use arch_program::entrypoint;

#[cfg(not(test))]
entrypoint!(process_instruction);

mod error;
mod instructions;

const MAX_MODIFIED_ACCOUNTS: usize = 8;
const MAX_INPUTS_TO_SIGN: usize = 4;

type EscrowTransactionBuilder<'a> =
    TransactionBuilder<'a, MAX_MODIFIED_ACCOUNTS, MAX_INPUTS_TO_SIGN>;

pub struct InstructionContext<'a> {
    pub program_id: Pubkey,
    pub accounts: &'a [AccountInfo<'static>],
    pub transaction_builder: EscrowTransactionBuilder<'a>,
}

#[derive(BorshDeserialize)]
pub enum EscrowInstruction {
    CreateEscrow { utxo: [UtxoMeta; 2] },
    Deposit { utxo: [UtxoMeta; 1] },
    Withdraw { amount: u64 },
}

pub fn process_instruction<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'static>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let instruction_params: EscrowInstruction = borsh::from_slice(instruction_data)
        .map_err(|err| ProgramError::BorshIoError(err.to_string()))?;

    let mut ctx = InstructionContext {
        program_id: *program_id,
        accounts,
        transaction_builder: TransactionBuilder::new(),
    };

    match instruction_params {
        EscrowInstruction::CreateEscrow { utxo } => {
            let utxo_infos: [UtxoInfo; 2] = to_utxo_info_array(&utxo)?;
        }
        EscrowInstruction::Deposit { utxo } => {
            let utxo_infos: [UtxoInfo; 1] = to_utxo_info_array(&utxo)?;
        }
        EscrowInstruction::Withdraw { amount } => {}
    }

    ctx.transaction_builder.finalize()?;

    Ok(())
}

pub fn to_utxo_info_array<const N: usize>(
    utxo_metas: &[UtxoMeta],
) -> Result<[UtxoInfo; N], ProgramError> {
    if utxo_metas.len() != N {
        return Err(ProgramError::InvalidInstructionData);
    }

    let mut utxo_infos = std::array::from_fn(|_| UtxoInfo::default());

    let res: Result<(), ProgramError> =
        utxo_metas
            .iter()
            .enumerate()
            .try_for_each(|(i, utxo_meta)| {
                let utxo_info = utxo_meta.try_into()?;
                utxo_infos[i] = utxo_info;

                Ok(())
            });

    res?;

    Ok(utxo_infos)
}
