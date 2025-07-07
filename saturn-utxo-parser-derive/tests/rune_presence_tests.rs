use arch_program::account::AccountInfo;
use arch_program::program_error::ProgramError;
use arch_program::rune::{RuneAmount, RuneId};
use arch_program::utxo::UtxoMeta;
use saturn_account_parser::Accounts as AccountsTrait;
use saturn_bitcoin_transactions::utxo_info::UtxoInfo;
use saturn_bitcoin_transactions::utxo_info::UtxoInfoTrait;
use saturn_utxo_parser::{ErrorCode, TryFromUtxos};
use saturn_utxo_parser_derive::UtxoParser;

fn create_utxo(value: u64, txid_byte: u8, vout: u32) -> UtxoInfo {
    let txid = [txid_byte; 32];
    UtxoInfo {
        meta: UtxoMeta::from(txid, vout),
        value,
        ..Default::default()
    }
}

fn create_utxo_with_rune(value: u64, txid_byte: u8, vout: u32, amount: u128) -> UtxoInfo {
    let mut utxo = create_utxo(value, txid_byte, vout);
    let rune = RuneAmount {
        id: RuneId::new(777, 0),
        amount,
    };
    // Insert rune entry (capacity is 1 for SingleRuneSet)
    utxo.runes_mut().insert(rune).unwrap();
    utxo
}

// -----------------------------------------------------------------------------
// Structs for different rune presence predicates
// -----------------------------------------------------------------------------

#[derive(Debug, UtxoParser)]
#[utxo_accounts(DummyAccounts<'a>)]
struct RuneNone<'a> {
    #[utxo(runes = "none")]
    no_rune_utxo: &'a UtxoInfo,
}

#[derive(Debug, UtxoParser)]
#[utxo_accounts(DummyAccounts<'a>)]
struct RuneSome<'a> {
    #[utxo(runes = "some")]
    some_rune_utxo: &'a UtxoInfo,
}

#[derive(Debug, UtxoParser)]
#[utxo_accounts(DummyAccounts<'a>)]
struct RuneAny<'a> {
    #[utxo(runes = "any")]
    any_utxo: &'a UtxoInfo,
}

// -----------------------------------------------------------------------------
// Tests for "none" predicate
// -----------------------------------------------------------------------------
#[test]
fn rune_none_success() {
    let no_rune_utxo = create_utxo(1_000, 1, 0);
    let inputs = vec![no_rune_utxo];

    let dummy = DummyAccounts::default();
    let parsed = RuneNone::try_utxos(&dummy, &inputs).expect("should parse without runes");
    assert_eq!(parsed.no_rune_utxo.value, 1_000);
}

#[test]
fn rune_none_failure() {
    let utxo_with_rune = create_utxo_with_rune(1_000, 2, 0, 42);
    let inputs = vec![utxo_with_rune];

    let dummy = DummyAccounts::default();
    let err = RuneNone::try_utxos(&dummy, &inputs).unwrap_err();
    assert_eq!(
        err,
        ProgramError::Custom(ErrorCode::InvalidRunesPresence.into())
    );
}

// -----------------------------------------------------------------------------
// Tests for "some" predicate
// -----------------------------------------------------------------------------
#[test]
fn rune_some_success() {
    let utxo_with_rune = create_utxo_with_rune(5_000, 3, 0, 100);
    let inputs = vec![utxo_with_rune];

    let dummy = DummyAccounts::default();
    let parsed = RuneSome::try_utxos(&dummy, &inputs).expect("should parse with runes present");
    assert_eq!(parsed.some_rune_utxo.value, 5_000);
}

#[test]
fn rune_some_failure() {
    let no_rune_utxo = create_utxo(5_000, 4, 0);
    let inputs = vec![no_rune_utxo];

    let dummy = DummyAccounts::default();
    let err = RuneSome::try_utxos(&dummy, &inputs).unwrap_err();
    assert_eq!(
        err,
        ProgramError::Custom(ErrorCode::InvalidRunesPresence.into())
    );
}

// -----------------------------------------------------------------------------
// Tests for "any" predicate (should accept both cases)
// -----------------------------------------------------------------------------
#[test]
fn rune_any_accepts_no_runes() {
    let no_rune_utxo = create_utxo(9_000, 5, 0);
    let inputs = vec![no_rune_utxo];
    let dummy = DummyAccounts::default();
    RuneAny::try_utxos(&dummy, &inputs).expect("any predicate should accept no runes");
}

#[test]
fn rune_any_accepts_some_runes() {
    let utxo_with_rune = create_utxo_with_rune(9_000, 6, 0, 1);
    let inputs = vec![utxo_with_rune];
    let dummy = DummyAccounts::default();
    RuneAny::try_utxos(&dummy, &inputs).expect("any predicate should accept runes");
}

// -------------------------------------------------------------------------------------------------
// Dummy Accounts implementation
// -------------------------------------------------------------------------------------------------
#[derive(Debug)]
struct DummyAccounts<'info> {
    dummy: AccountInfo<'info>,
}

impl<'info> AccountsTrait<'info> for DummyAccounts<'info> {
    fn try_accounts(
        _accounts: &'info [AccountInfo<'info>],
    ) -> Result<Self, ProgramError> {
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

        let acc_info = AccountInfo::new(
            key,
            lamports,
            data,
            key,
            utxo_meta,
            false,
            false,
            false,
        );

        Self { dummy: acc_info }
    }
} 