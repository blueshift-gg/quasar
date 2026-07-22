use crate::{Account, Instruction, Pubkey};

pub(crate) struct Backend {
    svm: quasar_svm::QuasarSvm,
}

impl Backend {
    pub(crate) fn new() -> Self {
        Self {
            svm: quasar_svm::QuasarSvm::new(),
        }
    }

    pub(crate) fn set_compute_unit_limit(&mut self, limit: u64) {
        self.svm.compute_budget.compute_unit_limit = limit;
    }

    pub(crate) fn load_program(&self, program_id: &Pubkey, elf: &[u8]) {
        self.svm
            .add_program(program_id, &quasar_svm::loader_keys::LOADER_V3, elf);
    }

    pub(crate) fn set_account(&mut self, account: Account) {
        self.svm.set_account(to_backend_account(account));
    }

    pub(crate) fn account(&self, address: &Pubkey) -> Option<Account> {
        self.svm.get_account(address).map(from_backend_account)
    }

    pub(crate) fn execute(
        &mut self,
        instructions: &[Instruction],
        accounts: &[Account],
        commit: bool,
    ) -> quasar_svm::ExecutionResult {
        let accounts = accounts
            .iter()
            .cloned()
            .map(to_backend_account)
            .collect::<Vec<_>>();
        if commit {
            self.svm.process_instruction_chain(instructions, &accounts)
        } else {
            self.svm.simulate_instruction_chain(instructions, &accounts)
        }
    }

    pub(crate) fn warp_to_timestamp(&mut self, timestamp: i64) {
        self.svm.warp_to_timestamp(timestamp);
    }
}

pub(crate) fn from_backend_account(account: quasar_svm::Account) -> Account {
    Account {
        address: account.address,
        lamports: account.lamports,
        data: account.data,
        owner: account.owner,
        executable: account.executable,
    }
}

fn to_backend_account(account: Account) -> quasar_svm::Account {
    quasar_svm::Account {
        address: account.address,
        lamports: account.lamports,
        data: account.data,
        owner: account.owner,
        executable: account.executable,
    }
}
