use arch_program::account::AccountInfo;
use arch_program::program_error::ProgramError;
use arch_program::utxo::UtxoMeta;
use saturn_account_parser::Accounts as AccountsTrait;
use saturn_bitcoin_transactions::utxo_info::UtxoInfo;
use saturn_utxo_parser::{ErrorCode, TryFromUtxos};
use saturn_utxo_parser_derive::UtxoParser;

/// Helper to create a deterministic `UtxoInfo` for testing purposes.
fn create_meta(txid_byte: u8, vout: u32) -> UtxoMeta {
    let txid = [txid_byte; 32];
    UtxoMeta::from(txid, vout)
}

// -----------------------------------------------------------------------------
// Array field happy-path behaviour
// -----------------------------------------------------------------------------
#[derive(Debug, UtxoParser)]
#[utxo_accounts(DummyAccounts)]
struct ArrayParser {
    /// Exactly three UTXOs (no additional predicates).
    inputs: [UtxoInfo; 3],
}

#[test]
fn parses_exact_array() {
    // Prepare three matching UTXOs in arbitrary order.
    let m_a = create_meta(1, 0);
    let m_b = create_meta(2, 0);
    let m_c = create_meta(3, 0);
    let inputs = vec![m_b, m_c, m_a];

    let dummy = DummyAccounts::default();
    let parsed = ArrayParser::try_utxos(&dummy, &inputs).expect("parsing should succeed");

    // Ensure we captured all three UTXOs in any order.
    assert_eq!(parsed.inputs.len(), 3);
}

// -----------------------------------------------------------------------------
// Array field mismatch behaviour (too few / too many inputs)
// -----------------------------------------------------------------------------

#[test]
fn array_too_few_inputs() {
    let m_a = create_meta(1, 0);
    let m_b = create_meta(2, 0);
    // Only 2 inputs instead of required 3
    let inputs = vec![m_a, m_b];

    let dummy = DummyAccounts::default();
    let err = ArrayParser::try_utxos(&dummy, &inputs).unwrap_err();
    assert_eq!(
        err,
        ProgramError::Custom(ErrorCode::MissingRequiredUtxo.into())
    );
}

#[test]
fn array_too_many_inputs() {
    let m_a = create_meta(1, 0);
    let m_b = create_meta(2, 0);
    let m_c = create_meta(3, 0);
    let extra = create_meta(4, 0);
    let inputs = vec![m_a, m_b, m_c, extra];

    let dummy = DummyAccounts::default();
    let err = ArrayParser::try_utxos(&dummy, &inputs).unwrap_err();
    assert_eq!(
        err,
        ProgramError::Custom(ErrorCode::UnexpectedExtraUtxos.into())
    );
}

// -------------------------------------------------------------------------------------------------
// Minimal dummy Accounts type reused from other integration tests. It does not perform validation but
// satisfies the `saturn_account_parser::Accounts` trait.
// -------------------------------------------------------------------------------------------------

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

        // Leak boxed values to obtain references with 'static lifetime.
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

        Self { dummy: acc_info }
    }
}
