use saturn_collections::generic::fixed_set::FixedSetError;
use saturn_error::saturn_error;
use saturn_safe_math::MathError;

#[saturn_error(offset = 800)]
pub enum BitcoinTxError {
    #[error("Transaction input amount is not enough to cover network fees")]
    NotEnoughAmountToCoverFees,

    #[error("The resulting transaction exceeds the maximum size allowed")]
    TransactionTooLarge,

    #[error("An arithmetic error ocurred")]
    CalcOverflow,

    #[error("The transaction inputs don't cover the amount to be spent in the transaction")]
    InsufficientInputAmount,

    #[error("The configured fee rate is too low")]
    InvalidFeeRateTooLow,

    #[error("The utxo was not found in the user utxos")]
    UtxoNotFoundInUserUtxos,

    #[error("The transaction input length must match the user utxos length")]
    TransactionInputLengthMustMatchUserUtxosLength,

    #[error("The transaction was not found")]
    TransactionNotFound,

    #[error("The utxo does not contain runes")]
    RuneOutputNotFound,

    #[error("The utxo contains more runes than the maximum allowed")]
    MoreRunesInUtxoThanMax,

    #[error("Not enough BTC in pool")]
    NotEnoughBtcInPool,

    #[error("The runestone is not valid")]
    RunestoneDecipherError,

    #[error("Rune input list is full")]
    RuneInputListFull,

    #[error("Rune addition overflow")]
    RuneAdditionOverflow,

    #[error("Input to sign list is full")]
    InputToSignListFull,

    #[error("Modified account list is full")]
    ModifiedAccountListFull,
}

impl From<FixedSetError> for BitcoinTxError {
    fn from(error: FixedSetError) -> Self {
        match error {
            FixedSetError::Full => BitcoinTxError::RuneInputListFull,
            FixedSetError::Duplicate => panic!("Duplicate rune input"),
        }
    }
}

impl From<MathError> for BitcoinTxError {
    fn from(error: MathError) -> Self {
        match error {
            MathError::AdditionOverflow => BitcoinTxError::CalcOverflow,
            MathError::SubtractionOverflow => BitcoinTxError::CalcOverflow,
            MathError::MultiplicationOverflow => BitcoinTxError::CalcOverflow,
            MathError::DivisionOverflow => BitcoinTxError::CalcOverflow,
            MathError::ConversionError => BitcoinTxError::CalcOverflow,
        }
    }
}
