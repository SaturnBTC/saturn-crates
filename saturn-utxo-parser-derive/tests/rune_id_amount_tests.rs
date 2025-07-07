#![cfg(feature = "runes")]

use arch_program::account::AccountInfo;
use arch_program::program_error::ProgramError;
use arch_program::rune::{RuneAmount, RuneId};
use arch_program::utxo::UtxoMeta;
use saturn_account_parser::Accounts as AccountsTrait;
use saturn_bitcoin_transactions::utxo_info::{UtxoInfo, UtxoInfoTrait};
use saturn_utxo_parser::{ErrorCode, TryFromUtxos};
use saturn_utxo_parser_derive::UtxoParser;

// Helper to create a plain UTXO
fn create_utxo(value: u64, txid_byte: u8, vout: u32) -> UtxoInfo {
    let txid = [txid_byte; 32];
    UtxoInfo {
        meta: UtxoMeta::from(txid, vout),
        value,
        ..Default::default()
    }
}

// Helper to create utxo with given rune id and amount
fn create_utxo_with_rune(
    value: u64,
    txid_byte: u8,
    vout: u32,
    rune_id: RuneId,
    amount: u128,
) -> UtxoInfo {
    let mut utxo = create_utxo(value, txid_byte, vout);
    let rune = RuneAmount {
        id: rune_id,
        amount,
    };
    utxo.runes_mut().insert(rune).unwrap();
    utxo
}

// Helper function to avoid const privacy limitations
fn target_rune_id() -> RuneId {
    RuneId::new(777, 0)
}

#[derive(Debug, UtxoParser)]
#[utxo_accounts(DummyAccounts<'a>)]
struct ExactRune<'a> {
    #[utxo(rune_id = RuneId::new(777, 0), rune_amount = 500)]
    exact: &'a UtxoInfo,
}

// Success path
#[test]
fn exact_rune_success() {
    let matching_utxo = create_utxo_with_rune(1_000, 1, 0, target_rune_id(), 500);
    let dummy = DummyAccounts::default();
    let inputs = vec![matching_utxo];
    let parsed = ExactRune::try_utxos(&dummy, &inputs).expect("should parse");
    assert_eq!(parsed.exact.value, 1_000);
}

// Wrong rune id should error
#[test]
fn rune_id_mismatch_error() {
    let wrong_id = RuneId::new(999, 0);
    let utxo = create_utxo_with_rune(1_000, 2, 0, wrong_id, 500);
    let dummy = DummyAccounts::default();
    let inputs = vec![utxo];
    let err = ExactRune::try_utxos(&dummy, &inputs).unwrap_err();
    assert_eq!(err, ProgramError::Custom(ErrorCode::InvalidRuneId.into()));
}

// Wrong amount should error
#[test]
fn rune_amount_mismatch_error() {
    let utxo = create_utxo_with_rune(1_000, 3, 0, target_rune_id(), 499);
    let dummy = DummyAccounts::default();
    let inputs = vec![utxo];
    let err = ExactRune::try_utxos(&dummy, &inputs).unwrap_err();
    assert_eq!(
        err,
        ProgramError::Custom(ErrorCode::InvalidRuneAmount.into())
    );
}

// ---------------------------------- Dummy Accounts ----------------------------------
#[derive(Debug)]
struct DummyAccounts<'info> {
    dummy: AccountInfo<'info>,
}

impl<'info> AccountsTrait<'info> for DummyAccounts<'info> {
    fn try_accounts(_accounts: &'info [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        Ok(Self::default())
    }
}

impl<'info> Default for DummyAccounts<'info> {
    fn default() -> Self {
        use arch_program::pubkey::Pubkey;

        let key: &'static Pubkey = Box::leak(Box::new(Pubkey::default()));
        let lamports: &'static mut u64 = Box::leak(Box::new(0u64));
        let data: &'static mut [u8] = Box::leak(Box::new([0u8; 1]));
        let utxo_meta: &'static UtxoMeta = Box::leak(Box::new(UtxoMeta::from([0u8; 32], 0)));

        let acc_info = AccountInfo::new(key, lamports, data, key, utxo_meta, false, false, false);

        Self { dummy: acc_info }
    }
}
