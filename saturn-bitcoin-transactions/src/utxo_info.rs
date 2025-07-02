#[cfg(feature = "runes")]
use arch_program::rune::RuneAmount;

use arch_program::{
    program::get_bitcoin_tx_output_value, program_error::ProgramError, utxo::UtxoMeta,
};

use bytemuck::{Pod, Zeroable};
use saturn_collections::declare_fixed_option;

use crate::{arch::get_rune, bytes::txid_to_bytes_big_endian, error::BitcoinTxError};

#[cfg(feature = "utxo-consolidation")]
declare_fixed_option!(FixedOptionF64, f64, 7);

#[cfg(feature = "runes")]
declare_fixed_option!(FixedOptionRuneAmount, RuneAmount, 15);

#[repr(C, align(8))]
#[derive(Clone, Copy, Debug)]
pub struct UtxoInfo {
    pub meta: UtxoMeta,
    pub value: u64,

    #[cfg(feature = "runes")]
    pub runes: FixedOptionRuneAmount,

    #[cfg(feature = "utxo-consolidation")]
    pub needs_consolidation: FixedOptionF64,
}

unsafe impl Pod for UtxoInfo {}
unsafe impl Zeroable for UtxoInfo {}

impl PartialEq for UtxoInfo {
    fn eq(&self, other: &Self) -> bool {
        self.meta == other.meta
    }
}

impl Eq for UtxoInfo {}

impl std::fmt::Display for UtxoInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", hex::encode(&self.meta.txid()), self.meta.vout())
    }
}

impl AsRef<UtxoInfo> for UtxoInfo {
    fn as_ref(&self) -> &UtxoInfo {
        self
    }
}

impl AsRef<UtxoMeta> for UtxoInfo {
    fn as_ref(&self) -> &UtxoMeta {
        &self.meta
    }
}

impl Default for UtxoInfo {
    fn default() -> Self {
        Self {
            meta: UtxoMeta::from([0; 32], 0),
            value: u64::default(),
            #[cfg(feature = "runes")]
            runes: FixedOptionRuneAmount::none(),
            #[cfg(feature = "utxo-consolidation")]
            needs_consolidation: FixedOptionF64::default(),
        }
    }
}

impl TryFrom<&UtxoMeta> for UtxoInfo {
    type Error = ProgramError;

    fn try_from(value: &UtxoMeta) -> std::result::Result<Self, ProgramError> {
        // Prepare rune amount info (only when the "runes" feature is enabled)
        #[cfg(feature = "runes")]
        let runes_option: Option<RuneAmount> = {
            let rune = get_rune(value)?;
            rune.map(|x| x.into())
        };

        let outpoint = value.to_outpoint();

        let ui_value =
            get_bitcoin_tx_output_value(txid_to_bytes_big_endian(&outpoint.txid), outpoint.vout)
                .ok_or(ProgramError::Custom(
                    BitcoinTxError::TransactionNotFound(outpoint.txid.to_string()).into(),
                ))?;

        Ok(UtxoInfo {
            meta: value.clone(),
            value: ui_value,
            #[cfg(feature = "runes")]
            runes: runes_option.into(),
            #[cfg(feature = "utxo-consolidation")]
            needs_consolidation: FixedOptionF64::none(),
        })
    }
}
