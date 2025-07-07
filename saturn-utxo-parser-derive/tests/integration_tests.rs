use arch_program::account::AccountInfo;
use arch_program::program_error::ProgramError;
use arch_program::utxo::UtxoMeta;
use saturn_account_parser::Accounts as AccountsTrait;
use saturn_bitcoin_transactions::utxo_info::UtxoInfo;
use saturn_utxo_parser::ErrorCode;
use saturn_utxo_parser::TryFromUtxos;
use saturn_utxo_parser_derive::UtxoParser;

/// Helper to construct a `UtxoInfo` with the given value and deterministic txid/vout.
fn create_utxo(
    value: u64,
    txid_byte: u8,
    vout: u32,
) -> saturn_bitcoin_transactions::utxo_info::UtxoInfo {
    let txid = [txid_byte; 32];
    UtxoInfo {
        meta: UtxoMeta::from(txid, vout),
        value,
        ..Default::default()
    }
}

// -----------------------------------------------------------------------------
// Basic happy-path behaviour
// -----------------------------------------------------------------------------
#[derive(Debug, UtxoParser)]
#[utxo_accounts(DummyAccounts<'a>)]
struct Basic<'a> {
    /// Mandatory fee UTXO with exact value expectation.
    #[utxo(value = 10_000)]
    fee: &'a saturn_bitcoin_transactions::utxo_info::UtxoInfo,

    /// Optional additional deposit UTXO (any predicate).
    deposit: Option<&'a saturn_bitcoin_transactions::utxo_info::UtxoInfo>,

    /// Catch-all for any remaining inputs.
    #[utxo(rest)]
    others: Vec<&'a saturn_bitcoin_transactions::utxo_info::UtxoInfo>,
}

#[test]
fn parses_expected_inputs() {
    // Prepare inputs.
    let fee_utxo = create_utxo(10_000, 1, 0);
    let deposit_utxo = create_utxo(55_000, 2, 0);
    let extra_utxo = create_utxo(99, 3, 1);

    // Order should not matter.
    let inputs = vec![deposit_utxo.clone(), fee_utxo.clone(), extra_utxo.clone()];

    let dummy = DummyAccounts::default();
    let parsed = Basic::try_utxos(&dummy, &inputs).expect("parsing should succeed");

    // Validate that fields were populated as expected.
    assert_eq!(parsed.fee.value, 10_000);
    assert!(parsed.deposit.is_some());
    assert_eq!(parsed.deposit.unwrap().value, 55_000);
    assert_eq!(parsed.others.len(), 1);
    assert_eq!(parsed.others[0].value, 99);
}

// -----------------------------------------------------------------------------
// Missing required UTXO should yield `MissingRequiredUtxo` error.
// -----------------------------------------------------------------------------
#[test]
fn missing_required_utxo() {
    // No fee UTXO with the required value is provided.
    let inputs = vec![create_utxo(500, 1, 0)];

    let dummy = DummyAccounts::default();
    let err = Basic::try_utxos(&dummy, &inputs).unwrap_err();
    assert_eq!(
        err,
        ProgramError::Custom(ErrorCode::InvalidUtxoValue.into())
    );
}

// -----------------------------------------------------------------------------
// Extra inputs without a `rest` collector should yield `UnexpectedExtraUtxos`.
// -----------------------------------------------------------------------------
#[derive(Debug, UtxoParser)]
#[utxo_accounts(DummyAccounts<'a>)]
struct NoRest<'a> {
    #[utxo(value = 1_000)]
    fee: &'a saturn_bitcoin_transactions::utxo_info::UtxoInfo,
}

#[test]
fn unexpected_extra_utxos() {
    let inputs = vec![create_utxo(1_000, 1, 0), create_utxo(2_000, 2, 0)];

    let dummy = DummyAccounts::default();
    let err = NoRest::try_utxos(&dummy, &inputs).unwrap_err();
    assert_eq!(
        err,
        ProgramError::Custom(ErrorCode::UnexpectedExtraUtxos.into())
    );
}

// -----------------------------------------------------------------------------
// Value predicate failure should yield `InvalidUtxoValue`.
// -----------------------------------------------------------------------------
#[derive(Debug, UtxoParser)]
#[utxo_accounts(DummyAccounts<'a>)]
struct ValueCheck<'a> {
    #[utxo(value = 42)]
    the_answer: &'a saturn_bitcoin_transactions::utxo_info::UtxoInfo,
}

#[test]
fn value_check_failure() {
    let inputs = vec![create_utxo(7, 1, 0)];

    let dummy = DummyAccounts::default();
    let err = ValueCheck::try_utxos(&dummy, &inputs).unwrap_err();
    assert_eq!(
        err,
        ProgramError::Custom(ErrorCode::InvalidUtxoValue.into())
    );
}

// -----------------------------------------------------------------------------
// Anchor attribute should be accepted and parsing should succeed.
// -----------------------------------------------------------------------------
#[derive(Debug, UtxoParser)]
#[utxo_accounts(DummyAccounts<'a>)]
struct AnchorAttr<'a> {
    /// UTXO that will be anchored to an account field (dummy for macro parsing).
    #[utxo(anchor = my_account)]
    anchor_utxo: &'a saturn_bitcoin_transactions::utxo_info::UtxoInfo,

    /// Capture any extras to satisfy the rest rule.
    #[utxo(rest)]
    others: Vec<&'a saturn_bitcoin_transactions::utxo_info::UtxoInfo>,
}

#[test]
fn anchor_attribute_parses() {
    let anchor = create_utxo(1_000, 10, 0);
    let extra = create_utxo(2_000, 11, 0);

    let inputs = vec![anchor.clone(), extra.clone()];

    let dummy = DummyAccounts::default();
    let parsed = AnchorAttr::try_utxos(&dummy, &inputs).expect("should parse with anchor attr");
    assert_eq!(parsed.anchor_utxo.value, 1_000);
    assert_eq!(parsed.others.len(), 1);
}

// -------------------------------------------------------------------------------------------------
// Minimal dummy Accounts type used in tests. It implements the `saturn_account_parser::Accounts`
// trait but doesn't perform any validation – good enough for unit testing the derive macro.
// -------------------------------------------------------------------------------------------------

#[derive(Debug)]
struct DummyAccounts<'info> {
    my_account: AccountInfo<'info>,
}

impl<'info> AccountsTrait<'info> for DummyAccounts<'info> {
    fn try_accounts(
        _accounts: &'info [AccountInfo<'info>],
    ) -> Result<Self, arch_program::program_error::ProgramError> {
        Ok(Self::default())
    }
}

impl<'info> Default for DummyAccounts<'info> {
    fn default() -> Self {
        use arch_program::{pubkey::Pubkey, utxo::UtxoMeta};

        // Leak boxed values to obtain references with 'static lifetime. This
        // is safe in test code because they live until the process exits and
        // we never mutate them concurrently.
        let key: &'static Pubkey = Box::leak(Box::new(Pubkey::default()));
        let lamports: &'static mut u64 = Box::leak(Box::new(0u64));
        let data: &'static mut [u8] = Box::leak(Box::new([0u8; 1]));
        let utxo_meta: &'static UtxoMeta = Box::leak(Box::new(UtxoMeta::from([0u8; 32], 0)));

        let acc_info = AccountInfo::new(
            key, lamports, data, key, // owner
            utxo_meta, false, // is_signer
            false, // is_writable
            false, // is_executable
        );

        Self {
            my_account: acc_info,
        }
    }
}
