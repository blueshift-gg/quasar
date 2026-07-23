use {
    crate::{backend::from_backend_account, Account, AccountChange, ProgramError, Pubkey},
    base64::{engine::general_purpose::STANDARD, Engine as _},
    quasar_lang::{
        __zeropod::{ZcElem, ZcValidate},
        traits::{Discriminator, Owner},
    },
    std::ops::Deref,
};

pub(crate) struct TrackedAccount {
    pub(crate) address: Pubkey,
    pub(crate) writable: bool,
    pub(crate) signer: bool,
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

    /// Assert typed post-state at `address`, passing the decoded data to
    /// `check` for user assertions.
    ///
    /// The resulting account is read through `T`'s on-chain wrapper with the
    /// same ownership, discriminator, length, and zero-copy validation as
    /// [`Test::read`](crate::Test::read). Panics with the address and the
    /// specific failure when the account is absent or malformed. Chainable, so
    /// several accounts can be asserted in one expression.
    pub fn has_state<T>(&self, address: Pubkey, check: impl FnOnce(&T::Target)) -> &Self
    where
        T: Discriminator + Owner + Deref,
        T::Target: ZcElem + ZcValidate + Copy,
    {
        let name = core::any::type_name::<T>();
        let account = self.account(address).unwrap_or_else(|| {
            panic!("has_state {name}: outcome does not contain account {address}")
        });
        let state = crate::world::validate_typed::<T>("has_state", account);
        check(&state);
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

    // A minimal fixed-size Quasar account whose payload is a single address,
    // enough to exercise the typed `has_state` read path without a program.
    #[repr(transparent)]
    #[derive(Clone, Copy)]
    struct Marker(Pubkey);

    impl Deref for Marker {
        type Target = Pubkey;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl Discriminator for Marker {
        const DISCRIMINATOR: &'static [u8] = &[0xAB];
    }

    impl Owner for Marker {
        const OWNER: Pubkey = Pubkey::new_from_array([9; 32]);
    }

    fn state_outcome(account: Account) -> Outcome {
        Outcome {
            error: None,
            compute_units: 0,
            logs: Vec::new(),
            return_data: Vec::new(),
            accounts: vec![account],
            changes: Vec::new(),
        }
    }

    fn marker_account(address: Pubkey, owner: Pubkey, stored: Pubkey) -> Account {
        let mut data = Marker::DISCRIMINATOR.to_vec();
        data.extend_from_slice(stored.as_ref());
        Account::new(address, owner, 42, data)
    }

    #[test]
    fn has_state_reads_and_checks_typed_post_state() {
        let address = Pubkey::new_from_array([5; 32]);
        let stored = Pubkey::new_from_array([7; 32]);
        let outcome = state_outcome(marker_account(address, Marker::OWNER, stored));

        let mut checked = false;
        outcome
            .has_state::<Marker>(address, |value| {
                assert_eq!(*value, stored);
                checked = true;
            })
            .cu_at_most(0);
        assert!(checked, "the check closure should run against the state");
    }

    #[test]
    #[should_panic(expected = "has_state")]
    fn has_state_panics_when_ownership_is_wrong() {
        let address = Pubkey::new_from_array([5; 32]);
        let foreign = Pubkey::new_from_array([1; 32]);
        let outcome = state_outcome(marker_account(address, foreign, address));

        outcome.has_state::<Marker>(address, |_| {
            unreachable!("validation runs before the check")
        });
    }
}
