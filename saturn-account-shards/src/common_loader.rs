//! Test helpers and mock types shared between `split_loader` and `update_loader` unit tests.
//!
//! This file is **included** as `crate::split_loader::common_loader` via
//! `#[path = "common_loader.rs"]` inside `split_loader.rs`.  Keeping the
//! implementation here avoids code duplication while maintaining the original
//! module path used throughout the existing tests.

// NOTE: The entire module is only compiled when running tests.
#![cfg(test)]

use super::*;
use arch_program::{account::AccountInfo, pubkey::Pubkey, utxo::UtxoMeta};
use bytemuck::{Pod, Zeroable};
use saturn_account_discriminator_derive::Discriminator;
use saturn_account_parser::codec::zero_copy::AccountLoader;
use saturn_bitcoin_transactions::utxo_info::{SingleRuneSet, UtxoInfo};

// Increased capacity to comfortably cover edge-case tests that require up to
// 5 UTXOs per shard × 10 shards. 64 is a convenient power-of-two that leaves
// plenty of head-room for future tests.
pub const MAX_BTC_UTXOS: usize = 64;

/// Zero-copy mock shard used exclusively in unit tests.
#[repr(C)]
#[derive(Clone, Copy, Zeroable, Discriminator)]
pub struct MockShardZc {
    /// Fixed-capacity array of BTC UTXOs.
    btc_utxos: [UtxoInfo<SingleRuneSet>; MAX_BTC_UTXOS],
    /// Rune-bearing UTXO slot.
    rune_utxo: UtxoInfo<SingleRuneSet>,
    /// Current number of valid BTC UTXOs (0..=MAX_BTC_UTXOS).
    btc_utxo_len: u8,
    /// `1` = `rune_utxo` occupied, `0` = empty.
    has_rune: u8,
    /// Padding to keep alignment multiple of 8 (Pod-safe).
    _padding: [u8; 5],
}

// SAFETY: All fields are Pod and the struct is #[repr(C)]
// with no hidden padding (explicit _padding), so the type is Pod-safe.
unsafe impl Pod for MockShardZc {}

impl Default for MockShardZc {
    fn default() -> Self {
        Self::zeroed()
    }
}

// ---------------------------------------------------------------------
// `StateShard` implementation so the mock can be used with library helpers.
// ---------------------------------------------------------------------
impl StateShard<UtxoInfo<SingleRuneSet>, SingleRuneSet> for MockShardZc {
    fn btc_utxos(&self) -> &[UtxoInfo<SingleRuneSet>] {
        let len = self.btc_utxo_len as usize;
        &self.btc_utxos[..len]
    }

    fn btc_utxos_mut(&mut self) -> &mut [UtxoInfo<SingleRuneSet>] {
        let len = self.btc_utxo_len as usize;
        &mut self.btc_utxos[..len]
    }

    fn btc_utxos_retain(&mut self, f: &mut dyn FnMut(&UtxoInfo<SingleRuneSet>) -> bool) {
        let len = self.btc_utxo_len as usize;
        let mut write_idx = 0usize;
        for read_idx in 0..len {
            let keep = f(&self.btc_utxos[read_idx]);
            if keep {
                if write_idx != read_idx {
                    self.btc_utxos[write_idx] = self.btc_utxos[read_idx];
                }
                write_idx += 1;
            }
        }
        self.btc_utxo_len = write_idx as u8;
    }

    fn add_btc_utxo(&mut self, utxo: UtxoInfo<SingleRuneSet>) -> Option<usize> {
        let len = self.btc_utxo_len as usize;
        if len >= MAX_BTC_UTXOS {
            return None;
        }
        self.btc_utxos[len] = utxo;
        self.btc_utxo_len += 1;
        Some(len)
    }

    fn btc_utxos_len(&self) -> usize {
        self.btc_utxo_len as usize
    }

    fn btc_utxos_max_len(&self) -> usize {
        MAX_BTC_UTXOS
    }

    fn rune_utxo(&self) -> Option<&UtxoInfo<SingleRuneSet>> {
        if self.has_rune == 1 {
            Some(&self.rune_utxo)
        } else {
            None
        }
    }

    fn rune_utxo_mut(&mut self) -> Option<&mut UtxoInfo<SingleRuneSet>> {
        if self.has_rune == 1 {
            Some(&mut self.rune_utxo)
        } else {
            None
        }
    }

    fn clear_rune_utxo(&mut self) {
        self.has_rune = 0;
    }

    fn set_rune_utxo(&mut self, utxo: UtxoInfo<SingleRuneSet>) {
        self.rune_utxo = utxo;
        self.has_rune = 1;
    }
}

// ---------------------------------------------------------------------
// Account-loader factory helpers
// ---------------------------------------------------------------------

/// Builds an in-memory [`AccountLoader<MockShardZc>`].
pub fn create_loader() -> AccountLoader<'static, MockShardZc> {
    // Leak all heap allocations → `'static` lifetime suitable for tests.
    let key = Box::leak(Box::new(Pubkey::default()));
    let owner = Box::leak(Box::new(Pubkey::default()));
    let utxo = Box::leak(Box::new(UtxoMeta::default()));
    let lamports = Box::leak(Box::new(0u64));

    // Data buffer sized exactly for the zero-copy struct.
    let data_len = core::mem::size_of::<MockShardZc>();
    let data: &'static mut [u8] = Box::leak(vec![0u8; data_len].into_boxed_slice());

    // Assemble `AccountInfo`.
    let account_info = AccountInfo::new(
        key, lamports, data, owner, utxo, /* is_signer   = */ false,
        /* is_writable = */ true, /* is_executable = */ false,
    );

    let account_ref: &'static AccountInfo<'static> = Box::leak(Box::new(account_info));

    AccountLoader::new(account_ref)
}

// ------------------------------------------------------------------
// Helper functions mirroring `split::common` for test convenience
// ------------------------------------------------------------------

/// Creates a deterministic `UtxoMeta` (txid pattern) for tests.
pub fn random_utxo_meta(vout: u32) -> UtxoMeta {
    UtxoMeta::from([vout as u8; 32], vout)
}

/// Builds a BTC `UtxoInfo` with the given value and vout.
pub fn create_btc_utxo(value: u64, vout: u32) -> UtxoInfo<SingleRuneSet> {
    UtxoInfo::<SingleRuneSet> {
        meta: random_utxo_meta(vout),
        value,
        ..Default::default()
    }
}

/// Constructs a `MockShardZc` pre-populated with one BTC-UTXO of `initial_btc`.
pub fn create_shard(initial_btc: u64) -> MockShardZc {
    let mut shard = MockShardZc::default();
    if initial_btc > 0 {
        shard.add_btc_utxo(create_btc_utxo(initial_btc, 0));
    }
    shard
}

/// Creates a loader that is pre-initialised with the provided `shard` data.
pub fn create_loader_from(shard: &MockShardZc) -> AccountLoader<'static, MockShardZc> {
    let loader = create_loader();
    {
        let mut mut_ref = loader.load_mut().expect("zero-copy borrow");
        *mut_ref = *shard;
    }
    loader
}

/// Utility: leak an array of loaders built from a `Vec<MockShardZc>` and return a `'static` slice.
pub fn leak_loaders_from_vec(
    shards: Vec<MockShardZc>,
) -> &'static [&'static AccountLoader<'static, MockShardZc>] {
    let mut boxed_vec: Vec<&'static AccountLoader<'static, MockShardZc>> =
        Vec::with_capacity(shards.len());
    for shard in shards {
        let loader: &'static _ = Box::leak(Box::new(create_loader_from(&shard)));
        boxed_vec.push(loader);
    }
    Box::leak(boxed_vec.into_boxed_slice())
}

pub fn create_loaders(shards: Vec<MockShardZc>) -> Vec<AccountLoader<'static, MockShardZc>> {
    shards
        .into_iter()
        .map(|shard| create_loader_from(&shard))
        .collect()
}

/// Adds multiple BTC-UTXOs to the provided `shard`.
/// Each new UTXO will use a sequential `vout` value starting after the current last index.
/// The helper silently stops once the shard reaches its maximum capacity.
pub fn add_btc_utxos_bulk(shard: &mut MockShardZc, sats_values: &[u64]) {
    let mut next_vout = shard.btc_utxos_len() as u32;
    for &value in sats_values {
        // Respect shard capacity – abort once full.
        if shard
            .add_btc_utxo(create_btc_utxo(value, next_vout))
            .is_none()
        {
            break;
        }
        next_vout = next_vout.saturating_add(1);
    }
}

/// Convenience wrapper to obtain a **mutable** reference to the underlying
/// `MockShardZc` inside an `AccountLoader` and run an arbitrary closure against it.
pub fn with_loader_mut<R, F: FnOnce(&mut MockShardZc) -> R>(
    loader: &'static AccountLoader<'static, MockShardZc>,
    f: F,
) -> R {
    let mut borrow = loader.load_mut().expect("zero-copy borrow");
    f(&mut borrow)
}

/// Builds a Rune-bearing `UtxoInfo` with the specified `amount` (whole runes) and `vout`.
#[cfg(feature = "runes")]
pub fn create_rune_utxo(amount: u128, vout: u32) -> UtxoInfo<SingleRuneSet> {
    use arch_program::rune::{RuneAmount, RuneId};
    use saturn_bitcoin_transactions::constants::DUST_LIMIT;

    // Single-entry rune set (capacity = 1)
    let mut runes = SingleRuneSet::default();
    // Safe: capacity is 1, so insertion cannot overflow.
    let _ = runes.insert(RuneAmount {
        id: RuneId::new(1, 1),
        amount,
    });

    UtxoInfo::<SingleRuneSet> {
        meta: random_utxo_meta(vout),
        value: DUST_LIMIT, // minimal on-chain value for a valid output
        runes,
        #[cfg(feature = "utxo-consolidation")]
        needs_consolidation: Default::default(),
    }
}
