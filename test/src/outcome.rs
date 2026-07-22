use {
    crate::{backend::from_backend_account, Account, AccountChange, ProgramError, Pubkey},
    base64::{engine::general_purpose::STANDARD, Engine as _},
};

pub(crate) struct TrackedAccount {
    pub(crate) address: Pubkey,
    pub(crate) writable: bool,
    pub(crate) before: Option<Account>,
    pub(crate) after: Option<Account>,
}

/// The structured result of executing one transaction.
///
/// `Outcome` owns the stable data tests normally need: the program error,
/// logs, return data, compute units, resulting accounts, and writable account
/// changes. The runtime's internal result type is intentionally private.
#[must_use = "assert the outcome with succeeds, fails, or fails_with"]
pub struct Outcome {
    error: Option<ProgramError>,
    compute_units: u64,
    logs: Vec<String>,
    return_data: Vec<u8>,
    accounts: Vec<Account>,
    changes: Vec<AccountChange>,
}

impl Outcome {
    pub(crate) fn from_backend(
        result: quasar_svm::ExecutionResult,
        tracked: Vec<TrackedAccount>,
    ) -> Self {
        let error = result
            .raw_result
            .err()
            .map(quasar_svm::ProgramError::from)
            .map(ProgramError::from);
        let mut accounts = tracked
            .iter()
            .filter_map(|account| account.after.clone())
            .collect::<Vec<_>>();
        accounts.sort_by_key(|account| account.address.to_bytes());
        accounts.dedup_by_key(|account| account.address);

        let changes = tracked
            .into_iter()
            .filter(|account| account.writable && account.before != account.after)
            .map(|account| AccountChange::new(account.address, account.before, account.after))
            .collect();

        Self {
            error,
            compute_units: result.compute_units_consumed,
            logs: result.logs,
            return_data: result.return_data,
            accounts,
            changes,
        }
    }

    pub(crate) fn simulated_account(
        result: &quasar_svm::ExecutionResult,
        address: &Pubkey,
    ) -> Option<Account> {
        result.account(address).cloned().map(from_backend_account)
    }

    /// Whether execution succeeded.
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }

    /// Whether execution failed.
    pub fn is_err(&self) -> bool {
        self.error.is_some()
    }

    /// The execution error, if any.
    pub fn error(&self) -> Option<&ProgramError> {
        self.error.as_ref()
    }

    /// Compute units consumed by the transaction.
    pub fn compute_units(&self) -> u64 {
        self.compute_units
    }

    /// Program logs in execution order.
    pub fn logs(&self) -> &[String] {
        &self.logs
    }

    /// Raw Solana return data.
    pub fn return_data(&self) -> &[u8] {
        &self.return_data
    }

    /// Decode return data with a generated or application-provided decoder.
    pub fn return_value<T>(&self, decode: impl FnOnce(&[u8]) -> Option<T>) -> Option<T> {
        decode(&self.return_data)
    }

    /// The resulting account state for an address involved in the transaction.
    pub fn account(&self, address: Pubkey) -> Option<&Account> {
        self.accounts
            .iter()
            .find(|account| account.address == address)
    }

    /// Decode a resulting account with a generated client decoder.
    pub fn account_as<T>(
        &self,
        address: Pubkey,
        decode: impl FnOnce(&[u8]) -> Option<T>,
    ) -> Option<T> {
        self.account(address)
            .and_then(|account| decode(&account.data))
    }

    /// Writable account changes, in first-appearance instruction order.
    pub fn account_changes(&self) -> &[AccountChange] {
        &self.changes
    }

    /// Decode every matching `sol_log_data` payload with a generated client
    /// event decoder. Unrelated program-data logs are ignored.
    pub fn events<T>(&self, decode: impl Fn(&[u8]) -> Option<T>) -> Vec<T> {
        self.logs
            .iter()
            .filter_map(|log| log.strip_prefix("Program data: "))
            .filter_map(|encoded| STANDARD.decode(encoded).ok())
            .filter_map(|bytes| decode(&bytes))
            .collect()
    }

    /// Assert success and keep the outcome available for chained assertions.
    pub fn succeeds(&self) -> &Self {
        if let Some(error) = &self.error {
            panic!("expected success, got {error}{}", self.formatted_logs());
        }
        self
    }

    /// Assert a typed custom program error.
    pub fn fails_with<E>(&self, expected: E) -> &Self
    where
        E: Into<u32>,
    {
        self.fails(ProgramError::Custom(expected.into()))
    }

    /// Assert a runtime or non-custom program error.
    pub fn fails(&self, expected: ProgramError) -> &Self {
        assert_eq!(
            self.error.as_ref(),
            Some(&expected),
            "unexpected execution outcome{}",
            self.formatted_logs()
        );
        self
    }

    /// Assert an inclusive compute-unit ceiling.
    pub fn cu_at_most(&self, limit: u64) -> &Self {
        assert!(
            self.compute_units <= limit,
            "expected at most {limit} compute units, consumed {}",
            self.compute_units
        );
        self
    }

    /// Assert a resulting lamport balance.
    pub fn has_lamports(&self, address: Pubkey, expected: u64) -> &Self {
        assert_eq!(
            self.required_account(address).lamports,
            expected,
            "unexpected lamport balance for {address}"
        );
        self
    }

    /// Assert a resulting Token or Token-2022 account balance.
    pub fn has_tokens(&self, address: Pubkey, expected: u64) -> &Self {
        assert_eq!(
            token_amount(self.required_account(address)),
            expected,
            "unexpected token balance for {address}"
        );
        self
    }

    /// Assert a resulting Token or Token-2022 mint supply.
    pub fn has_supply(&self, address: Pubkey, expected: u64) -> &Self {
        assert_eq!(
            mint_supply(self.required_account(address)),
            expected,
            "unexpected mint supply for {address}"
        );
        self
    }

    /// Assert Solana's closed-account state. A runtime may remove the account
    /// entirely or retain its empty system-owned representation.
    pub fn is_closed(&self, address: Pubkey) -> &Self {
        if let Some(account) = self.account(address) {
            assert_eq!(account.lamports, 0, "closed account still holds lamports");
            assert!(account.data.is_empty(), "closed account still holds data");
            assert_eq!(
                account.owner,
                crate::system_program::ID,
                "closed account is not system-owned"
            );
        }
        self
    }

    fn required_account(&self, address: Pubkey) -> &Account {
        self.account(address)
            .unwrap_or_else(|| panic!("outcome does not contain account {address}"))
    }

    fn formatted_logs(&self) -> String {
        if self.logs.is_empty() {
            return String::new();
        }
        format!("\nprogram logs:\n  {}", self.logs.join("\n  "))
    }
}

pub(crate) fn token_amount(account: &Account) -> u64 {
    use spl_token::{solana_program::program_pack::Pack, state::Account as TokenAccount};

    TokenAccount::unpack(&account.data)
        .unwrap_or_else(|error| {
            panic!(
                "could not decode {} as a token account: {error}",
                account.address
            )
        })
        .amount
}

pub(crate) fn mint_supply(account: &Account) -> u64 {
    use spl_token::{solana_program::program_pack::Pack, state::Mint};

    Mint::unpack(&account.data)
        .unwrap_or_else(|error| {
            panic!(
                "could not decode {} as a token mint: {error}",
                account.address
            )
        })
        .supply
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(logs: &[&str], compute_units: u64) -> Outcome {
        Outcome {
            error: None,
            compute_units,
            logs: logs.iter().map(ToString::to_string).collect(),
            return_data: vec![9, 8, 7],
            accounts: Vec::new(),
            changes: Vec::new(),
        }
    }

    #[test]
    fn event_decoding_ignores_unrelated_and_malformed_logs() {
        let outcome = outcome(
            &[
                "Program log: before",
                "Program data: AQID",
                "Program data: not-base64",
                "Program log: after",
            ],
            10,
        );

        assert_eq!(
            outcome.events(|bytes| (bytes.first() == Some(&1)).then(|| bytes.to_vec())),
            [vec![1, 2, 3]]
        );
        assert_eq!(outcome.return_value(|bytes| Some(bytes[1])), Some(8));
    }

    #[test]
    fn compute_unit_ceiling_is_inclusive() {
        outcome(&[], 10).cu_at_most(10);
    }
}
