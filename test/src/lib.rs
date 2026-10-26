//! Runs a program's SBF build under the pinned runtime profile: Agave 4.3.2
//! through Mollusk, with every feature of that release enabled.

use std::{cell::RefCell, path::PathBuf, rc::Rc};

use agave_feature_set::FeatureSet;
use base64::{Engine, engine::general_purpose::STANDARD};
use mollusk_svm::{
    Mollusk,
    program::{ProgramCache, create_program_account_loader_v3, keyed_account_for_system_program},
    result::{InstructionResult, ProgramResult},
};
use quasar_lang::account::Kind;
use solana_account::Account;
use solana_address::Address;
use solana_instruction::Instruction;
pub use solana_instruction::error::InstructionError;
use solana_program_error::ProgramError;
use solana_sdk_ids::bpf_loader_upgradeable;
use solana_svm_log_collector::LogCollector;

/// One program and the accounts it runs against. Accounts keep the state left
/// by every successful instruction.
pub struct Test {
    runtime: Mollusk,
    accounts: Vec<(Address, Account)>,
    /// Each program loaded, with the path it came from under the target
    /// directory if it has one: its `debug` build is beside it.
    programs: Vec<(Address, Option<String>, Vec<u8>)>,
}

impl Test {
    /// Loads `target/deploy/<name>.so`. The System Program and the Clock and
    /// Rent sysvar accounts are always available.
    pub fn new(program: Address, name: &str) -> Self {
        let mut test = Self::empty();
        test.add_program(program, name);
        test
    }

    /// Loads a program built from a pinned revision by `bench/rivals.sh`,
    /// from `target/rivals/<name>.so`.
    pub fn rival(program: Address, name: &str) -> Self {
        let mut test = Self::empty();
        test.load(program, &format!("rivals/{name}.so"));
        test
    }

    /// Loads the program as `make sbf` builds it with `quasar-lang/debug`:
    /// making every check itself, none left to a call, and logging which
    /// fails. A test runs it beside the usual build and compares what they
    /// decide.
    pub fn debug(program: Address, name: &str) -> Self {
        let mut test = Self::empty();
        test.load(program, &format!("debug-checks/deploy/{name}.so"));
        test
    }

    /// Loads the program as built under `target/<dir>/deploy`.
    pub fn built_in(dir: &str, program: Address, name: &str) -> Self {
        let mut test = Self::empty();
        test.load(program, &format!("{dir}/deploy/{name}.so"));
        test
    }

    /// Classic SPL Token, pinned independently of the runtime; see `programs/README.md`.
    pub fn spl_token_elf() -> &'static [u8] {
        include_bytes!("../programs/spl_token.so")
    }

    /// p-token, built from a pinned revision by `bench/rivals.sh`.
    pub fn p_token_elf() -> Vec<u8> {
        let elf = target_dir().join("rivals/pinocchio_token_program.so");
        std::fs::read(&elf).unwrap_or_else(|error| panic!("{}: {error}", elf.display()))
    }

    /// Loads another program, so the one under test can call it.
    pub fn add_program(&mut self, program: Address, name: &str) {
        self.load(program, &format!("deploy/{name}.so"));
    }

    fn empty() -> Self {
        let mut runtime = Mollusk::default();
        runtime.feature_set = FeatureSet::all_enabled().runtime_features();
        runtime.program_cache =
            ProgramCache::new(&runtime.feature_set, &runtime.compute_budget, tracing());
        let sysvars = &runtime.sysvars;
        let accounts = vec![
            keyed_account_for_system_program(),
            sysvars.keyed_account_for_clock_sysvar(),
            sysvars.keyed_account_for_rent_sysvar(),
        ];
        Self { runtime, accounts, programs: Vec::new() }
    }

    /// Loads a program from its ELF, such as SPL Token's.
    pub fn add_elf(&mut self, program: Address, elf: &[u8]) {
        self.add(program, None, elf);
    }

    fn add(&mut self, program: Address, path: Option<&str>, elf: &[u8]) {
        self.runtime.add_program_with_loader_and_elf(&program, &bpf_loader_upgradeable::id(), elf);
        self.accounts.push((program, create_program_account_loader_v3(&program)));
        self.programs.push((program, path.map(str::to_owned), elf.to_vec()));
    }

    fn load(&mut self, program: Address, path: &str) {
        let elf = target_dir().join(path);
        let elf = std::fs::read(&elf).unwrap_or_else(|error| {
            panic!("{}: {error}; run `make sbf` and `bench/rivals.sh` first", elf.display())
        });
        self.add(program, Some(path), &elf);
    }

    /// Why `instruction` fails: what the programs log when it runs against
    /// the same accounts with each program's `debug` build in place of its
    /// usual one. Empty if no program here has one built.
    fn explain(&self, instruction: &Instruction) -> Vec<String> {
        let mut debug = Self::empty();
        let mut any = false;
        for (program, path, elf) in &self.programs {
            let built =
                path.as_ref().and_then(|path| path.strip_prefix("deploy/")).and_then(|name| {
                    std::fs::read(target_dir().join("debug-checks/deploy").join(name)).ok()
                });
            any |= built.is_some();
            debug.add(*program, None, built.as_deref().unwrap_or(elf));
        }
        if !any {
            return Vec::new();
        }
        debug.accounts = self.accounts.clone();
        // ponytail: the sysvars a test warped are not carried over (Mollusk's
        // are not `Clone`); copy them field by field if an explanation of a
        // time-dependent failure is ever wrong.
        let logs = LogCollector::new_ref();
        debug.runtime.logger = Some(Rc::clone(&logs));
        let result = debug.runtime.process_instruction(instruction, &debug.accounts);
        let mut said: Vec<String> = logs
            .borrow()
            .get_recorded_content()
            .iter()
            .filter_map(|line| line.strip_prefix("Program log: ").map(|line| format!("  {line}")))
            .collect();
        if result.program_result.is_ok() {
            said.push("  (its `debug` build succeeds on this instruction)".to_owned());
        }
        said
    }

    /// A new system-owned account holding `lamports`.
    pub fn wallet(&mut self, lamports: u64) -> Address {
        let key = Address::new_unique();
        self.accounts.push((key, Account { lamports, ..Account::default() }));
        key
    }

    /// A new account owned by `K::OWNER`, holding `value`, with the rent-exempt balance.
    pub fn account<K: Kind>(&mut self, value: &K) -> Address {
        self.account_at(Address::new_unique(), value)
    }

    /// Like `account`, at a chosen address such as a PDA.
    pub fn account_at<K: Kind>(&mut self, key: Address, value: &K) -> Address {
        let data =
            [K::DISCRIMINATOR, &quasar_lang::zeropod::to_vec(value).expect("a storable value")]
                .concat();
        self.set(key, K::OWNER, self.rent(data.len()), data)
    }

    /// A new account with this owner, balance and data.
    pub fn account_with(&mut self, owner: Address, lamports: u64, data: Vec<u8>) -> Address {
        self.set(Address::new_unique(), owner, lamports, data)
    }

    pub fn owner(&self, key: &Address) -> Address {
        self.stored(key).owner
    }

    /// Puts an account with this owner, balance and data at `key`.
    pub fn set(&mut self, key: Address, owner: Address, lamports: u64, data: Vec<u8>) -> Address {
        self.put(key, Account { lamports, data, owner, ..Account::default() });
        key
    }

    fn put(&mut self, key: Address, account: Account) {
        match self.accounts.iter_mut().find(|(known, _)| *known == key) {
            Some((_, known)) => *known = account,
            None => self.accounts.push((key, account)),
        }
    }

    /// The rent-exempt balance for `len` bytes of data.
    pub fn rent(&self, len: usize) -> u64 {
        self.runtime.sysvars.rent.minimum_balance(len)
    }

    /// The runtime itself: its sysvars, to read, warp or pass as accounts,
    /// and programs loaded from elsewhere, such as SPL Token.
    pub fn runtime(&mut self) -> &mut Mollusk {
        &mut self.runtime
    }

    pub fn run(&mut self, instruction: Instruction) -> Outcome {
        let logs = LogCollector::new_ref();
        self.runtime.logger = Some(Rc::clone(&logs));
        let result = self.runtime.process_instruction(&instruction, &self.accounts);
        if result.program_result.is_ok() {
            for (key, account) in &result.resulting_accounts {
                self.put(*key, account.clone());
            }
        }
        let explained = match result.program_result.is_ok() {
            true => Vec::new(),
            false => self.explain(&instruction),
        };
        Outcome { result, logs, explained }
    }

    pub fn lamports(&self, key: &Address) -> u64 {
        self.stored(key).lamports
    }

    pub fn data(&self, key: &Address) -> &[u8] {
        &self.stored(key).data
    }

    /// The `K` an account holds, after checking it holds one, and nothing else.
    pub fn read<K: Kind>(&self, key: &Address) -> K {
        let body = self.data(key).strip_prefix(K::DISCRIMINATOR);
        body.and_then(|body| quasar_lang::zeropod::from_slice(body).ok())
            .unwrap_or_else(|| panic!("not a {}", std::any::type_name::<K>()))
    }

    /// Every account's key, owner, lamports and data, in key order.
    pub fn state(&self) -> Vec<(Address, Address, u64, Vec<u8>)> {
        let mut state: Vec<_> = self
            .accounts
            .iter()
            .map(|(key, account)| (*key, account.owner, account.lamports, account.data.clone()))
            .collect();
        state.sort();
        state
    }

    fn stored(&self, key: &Address) -> &Account {
        let (_, account) =
            self.accounts.iter().find(|(known, _)| known == key).expect("unknown account");
        account
    }
}

/// Whether programs record their instructions as they run: with the `trace`
/// feature, when `SBF_TRACE_DIR` says where to.
fn tracing() -> bool {
    cfg!(feature = "trace") && std::env::var_os("SBF_TRACE_DIR").is_some()
}

/// Where builds go: `CARGO_TARGET_DIR`, or the nearest `target` above the
/// directory the tests run in, which is the package under test.
pub fn target_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CARGO_TARGET_DIR") {
        return dir.into();
    }
    let here = std::env::current_dir().expect("a working directory");
    let root = here.ancestors().find(|dir| dir.join("target").is_dir()).unwrap_or(&here);
    root.join("target")
}

/// Fails unless `actual` is the committed file at `path`. With
/// `QUASAR_BLESS=1` (`make idl`), writes it instead.
pub fn snapshot(path: &str, actual: &str) {
    if std::env::var_os("QUASAR_BLESS").is_some() {
        std::fs::write(path, actual).expect("writable");
    } else {
        let committed = std::fs::read_to_string(path).unwrap_or_default();
        assert!(committed == actual, "{path} is out of date; run `make idl` and review the diff");
    }
}

pub struct Outcome {
    result: InstructionResult,
    logs: Rc<RefCell<LogCollector>>,
    explained: Vec<String>,
}

impl Outcome {
    pub fn succeeds(self) -> Self {
        assert!(
            self.result.program_result.is_ok(),
            "expected success, got {:?}{}",
            self.result.program_result,
            self.why()
        );
        self
    }

    /// What the programs' `debug` builds say about the failure, each line as
    /// logged; empty for a success, or when none is built (`make sbf`).
    pub fn explanation(&self) -> &[String] {
        &self.explained
    }

    /// The explanation, for a failed assertion's message.
    fn why(&self) -> String {
        match self.explained.as_slice() {
            [] => self.missing_reload().to_owned(),
            said => format!("\nits `debug` build says:\n{}", said.join("\n")),
        }
    }

    /// What an access violation at an address with the top bit set means:
    /// the program used an account after a call that may have changed it,
    /// without `reload()` (`Entry::poison`). The runtime only reports the
    /// address.
    fn missing_reload(&self) -> &'static str {
        let logs = self.logs.borrow();
        let poisoned = logs.get_recorded_content().iter().any(|line| {
            line.split_once("Access violation in unknown section at address 0x").is_some_and(
                |(_, address)| {
                    address.len() > 16
                        && address.starts_with(['8', '9', 'a', 'b', 'c', 'd', 'e', 'f'])
                        && address.as_bytes()[16] == b' '
                },
            )
        });
        match poisoned {
            true => {
                ": an account was used after a call that may have changed it; call `reload()` on it first"
            }
            false => "",
        }
    }

    pub fn fails(self, expected: impl Into<ProgramError>) -> Self {
        let expected = ProgramResult::Failure(expected.into());
        assert!(
            self.result.program_result == expected,
            "expected {expected:?}, got {:?}{}",
            self.result.program_result,
            self.why()
        );
        self
    }

    /// The runtime rejected the instruction, not the program.
    pub fn rejected(self, expected: InstructionError) -> Self {
        assert_eq!(self.result.program_result, ProgramResult::UnknownError(expected));
        self
    }

    pub fn is_ok(&self) -> bool {
        self.result.program_result.is_ok()
    }

    pub fn return_data(&self) -> &[u8] {
        &self.result.return_data
    }

    /// Every line the runtime and the program logged.
    pub fn logs(&self) -> Vec<String> {
        self.logs.borrow().get_recorded_content().to_vec()
    }

    /// Every `Program data:` entry the program logged, each decoded and joined
    /// into one byte string.
    pub fn events(&self) -> Vec<Vec<u8>> {
        let logs = self.logs.borrow();
        logs.get_recorded_content()
            .iter()
            .filter_map(|line| line.strip_prefix("Program data: "))
            .map(|parts| {
                parts.split(' ').flat_map(|part| STANDARD.decode(part).expect("base64")).collect()
            })
            .collect()
    }

    pub fn compute_units(&self) -> u64 {
        self.result.compute_units_consumed
    }
}

#[cfg(test)]
mod token_fixture_tests {
    #[test]
    fn classic_token_keeps_spl_instruction_logs() {
        let elf = super::Test::spl_token_elf();
        let marker = b"Instruction: Transfer";
        assert!(elf.windows(marker.len()).any(|bytes| bytes == marker));
        assert!(!elf.windows(b"pinocchio".len()).any(|bytes| bytes == b"pinocchio"));
    }
}
