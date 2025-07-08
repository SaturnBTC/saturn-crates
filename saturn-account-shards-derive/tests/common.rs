//! Common types, declarations and helper functions shared by the StateShard
//! integration test suite. Keeping them in a single file avoids repeating
//! ~200 lines of boiler-plate in every test crate and therefore speeds up
//! incremental compilations.

use arch_program::utxo::UtxoMeta;
use saturn_account_shards::{declare_fixed_array, declare_fixed_option, Result};
use saturn_account_shards_derive::ShardAccount;
use saturn_bitcoin_transactions::utxo_info::{SingleRuneSet, UtxoInfo};

#[cfg(feature = "runes")]
use arch_program::rune::{RuneAmount, RuneId};

// -----------------------------------------------------------------------------
//  UTXO collection types used throughout the tests
// -----------------------------------------------------------------------------

declare_fixed_array!(
    TestBtcUtxos,
    saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
    10
);
declare_fixed_option!(
    TestRuneUtxo,
    saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
    15
);

// -----------------------------------------------------------------------------
//  Shard variants used by the different test groups
// -----------------------------------------------------------------------------

/// Basic struct using default field names.
#[derive(ShardAccount, Debug, Clone, Copy)]
#[shard(rune_set_type = "SingleRuneSet")]
#[repr(C)]
pub struct DefaultShard {
    pub pool_id: u64,
    pub liquidity: u128,
    pub btc_utxos: TestBtcUtxos,
    pub rune_utxo: TestRuneUtxo,
}

/// Custom struct with non-default field names.
#[derive(ShardAccount, Debug, Clone, Copy)]
#[shard(
    btc_utxos_attr = "bitcoin_utxos",
    rune_utxo_attr = "rune_utxo_data",
    rune_set_type = "SingleRuneSet"
)]
#[repr(C)]
pub struct CustomFieldShard {
    pub pool_id: u64,
    pub bitcoin_utxos: TestBtcUtxos,
    pub rune_utxo_data: TestRuneUtxo,
}

/// Complex struct with additional miscellaneous fields.
#[derive(ShardAccount, Debug, Clone, Copy)]
#[shard(rune_set_type = "SingleRuneSet")]
#[repr(C)]
pub struct ComplexShard {
    pub pool_id: u64,
    pub liquidity: u128,
    pub fee_rate: u32,
    pub last_update: u64,
    pub btc_utxos: TestBtcUtxos,
    pub rune_utxo: TestRuneUtxo,
    pub metadata: [u8; 32],
    pub extra_data: u64,
}

// -----------------------------------------------------------------------------
//  Helper functions
// -----------------------------------------------------------------------------

/// Create a BTC-only `UtxoInfo` with a deterministic txid byte pattern.
pub fn create_test_utxo(value: u64, txid_byte: u8, vout: u32) -> UtxoInfo<SingleRuneSet> {
    let txid = [txid_byte; 32];
    UtxoInfo {
        meta: UtxoMeta::from(txid, vout),
        value,
        ..Default::default()
    }
}

/// Create a UTXO that optionally carries rune data (compiled only when the
/// `runes` feature is enabled).
#[cfg(feature = "runes")]
pub fn create_test_rune_utxo(
    value: u64,
    txid_byte: u8,
    vout: u32,
    rune_amount: u128,
) -> UtxoInfo<SingleRuneSet> {
    let txid = [txid_byte; 32];
    let mut runes = SingleRuneSet::default();
    runes
        .insert(RuneAmount {
            id: RuneId::new(1, 1),
            amount: rune_amount,
        })
        .unwrap();

    UtxoInfo {
        meta: UtxoMeta::from(txid, vout),
        value,
        runes,
        ..Default::default()
    }
}
