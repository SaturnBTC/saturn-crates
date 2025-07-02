use arch_program::program_error::ProgramError;
use bitcoin::OutPoint;
use saturn_safe_math::MathError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum BitcoinTxError {
    #[error("Transaction input amount is not enough to cover network fees")]
    NotEnoughAmountToCoverFees,

    #[error("An arithmetic error ocurred")]
    MathError(#[from] MathError),

    #[error("The resulting transaction exceeds the maximum size allowed")]
    TransactionTooLarge,

    #[error("The transaction inputs don't cover the amount to be spent in the transaction")]
    InsufficientInputAmount,

    #[error("The configured fee rate is too low")]
    InvalidFeeRateTooLow,

    #[error("The utxo was not found in the user utxos: {0}")]
    UtxoNotFound(OutPoint),

    #[error("The transaction was not found: {0}")]
    TransactionNotFound(String),

    #[error("The utxo does not contain runes")]
    RuneOutputNotFound,

    #[error("The utxo contains multiple runes")]
    MultipleRunesInUtxo,

    #[error("Not enough BTC in pool")]
    NotEnoughBtcInPool,
}

impl From<BitcoinTxError> for u32 {
    fn from(error: BitcoinTxError) -> u32 {
        match error {
            BitcoinTxError::NotEnoughAmountToCoverFees => 800,
            BitcoinTxError::MathError(_) => 801,
            BitcoinTxError::TransactionTooLarge => 802,
            BitcoinTxError::InsufficientInputAmount => 803,
            BitcoinTxError::InvalidFeeRateTooLow => 804,
            BitcoinTxError::UtxoNotFound(_) => 805,
            BitcoinTxError::TransactionNotFound(_) => 806,
            BitcoinTxError::RuneOutputNotFound => 807,
            BitcoinTxError::MultipleRunesInUtxo => 808,
            BitcoinTxError::NotEnoughBtcInPool => 809,
        }
    }
}

impl From<BitcoinTxError> for ProgramError {
    fn from(error: BitcoinTxError) -> ProgramError {
        ProgramError::Custom(error.into())
    }
}
