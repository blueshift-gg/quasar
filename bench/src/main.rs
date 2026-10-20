//! Runs the same vault instructions through each implementation, all under
//! the pinned runtime profile, and prints the results as Markdown.
//! `make bench` writes them to `bench/RESULTS.md`; `make check` fails if a
//! fresh run differs from that file.

mod escrow_bench;

use quasar_lang::sysvar::{Rent, SysvarData};
use quasar_lang::{Address, client::Instruction};
use quasar_test::Test;
use vault::ID;

/// The same program built with `quasar-lang/debug`: making every check
/// itself, none left to a call that makes it, and saying which fails.
const EVERY_CHECK: &str = "the same, built with `debug`";
const SYSTEM: Address = Address::new_from_array([0; 32]);
/// The same user everywhere: how many bumps a PDA search tries depends on it.
const USER: Address = Address::new_from_array([7; 32]);

/// Name in the table, and the program's file under `target/rivals`, or
/// `None` for this repository's vault.
const IMPLEMENTATIONS: [(&str, Option<&str>); 4] = [
    ("Quasar, this repository", None),
    ("Quasar master (b0de7db)", Some("blueshift_vault")),
    ("anchor-next v2 (dbb5bae)", Some("anchor_next_vault")),
    ("Pinocchio 0.11, by hand", Some("pinocchio_vault")),
];

fn main() {
    let (_, bump) = Address::find_program_address(&[b"vault", USER.as_ref()], &ID);
    print!(
        r#"# Vault benchmark

A system-owned vault, written in each framework with the same account checks.
The vault is a system-owned PDA, which anyone can fund at the address of any
bump, so the frameworks that check it prove its bump canonical: each candidate
from 255 down is hashed and curve-checked until one is off the curve. Quasar,
Quasar master and the Pinocchio version, which calls `find_program_address`,
do; anchor-next accepts any bump that gives the address.
Agave 3.1.14 through Mollusk 0.10.3, every feature enabled; `bench/rivals.sh` pins
the other revisions. Compute units include the System Program call. Every run
uses the same user, whose vault's canonical bump is {bump}, so a search from 255
tries {tries} bumps.

| Implementation | deposit (CU) | withdraw (CU) | binary (bytes) |
|---|---:|---:|---:|
"#,
        tries = 256 - u32::from(bump),
    );
    for (name, rival) in IMPLEMENTATIONS {
        let (t, path) = match rival {
            None => (Test::new(ID, "vault"), "deploy/vault".to_owned()),
            Some(file) => (Test::rival(ID, file), format!("rivals/{file}")),
        };
        let accounts = (USER, vault(), SYSTEM);
        let deposit = vault::deposit::instruction(accounts, vault::Amount { amount: 1_000_000 });
        let withdraw = vault::withdraw::instruction(accounts, vault::Amount { amount: 400_000 });
        row(name, t, SYSTEM, [deposit, withdraw], &path);
    }

    print!(
        r#"
## A vault the program owns

Quasar master's vault example, unmodified, and the same vault written here
(`examples/lamport-vault`): deposits go through the System Program into a vault
the program owns, and withdrawals take lamports straight out, with no call.

| Implementation | deposit (CU) | withdraw (CU) | binary (bytes) |
|---|---:|---:|---:|
"#
    );
    for (name, t, path) in [
        ("Quasar, this repository", Test::new(ID, "lamport_vault"), "deploy/lamport_vault"),
        ("Quasar master (b0de7db)", Test::rival(ID, "quasar_vault"), "rivals/quasar_vault"),
    ] {
        let deposit = lamport_vault::deposit::instruction(
            (USER, vault(), SYSTEM),
            lamport_vault::Amount { amount: 1_000_000 },
        );
        let withdraw = lamport_vault::withdraw::instruction(
            (USER, vault()),
            lamport_vault::Amount { amount: 400_000 },
        );
        row(name, t, ID, [deposit, withdraw], path);
    }

    print!(
        r#"
## Escrow

Quasar master's escrow example, unmodified, and the same escrow written here
(`examples/escrow`), on the SPL Token program: make, then take; and make, then
refund. Both log the same events; the rewrite also accepts Token-2022 accounts.
The maker's token account
for mint B and the vault are created by `make`; the taker's account for mint
A by `take`.

| Implementation | make (CU) | take (CU) | refund (CU) | binary (bytes) |
|---|---:|---:|---:|---:|
"#
    );
    for (name, rival) in [
        ("Quasar, this repository", None),
        (EVERY_CHECK, Some("debug")),
        ("Quasar master (b0de7db)", Some("quasar_escrow")),
    ] {
        let program = |rival: Option<&str>| match rival {
            None => Test::new(escrow::ID, "escrow"),
            Some("debug") => Test::debug(escrow::ID, "escrow"),
            Some(file) => Test::rival(escrow::ID, file),
        };
        let path = match rival {
            None => "deploy/escrow".to_owned(),
            Some("debug") => "debug-checks/deploy/escrow".to_owned(),
            Some(file) => format!("rivals/{file}"),
        };
        let mut t = escrow_bench::prepare(program(rival));
        let make = t.run(escrow_bench::make()).succeeds().compute_units();
        let take = t.run(escrow_bench::take()).succeeds().compute_units();
        let mut t = escrow_bench::prepare(program(rival));
        t.run(escrow_bench::make()).succeeds();
        let refund = t.run(escrow_bench::refund()).succeeds().compute_units();
        println!("| {name} | {make} | {take} | {refund} | {} |", size(&path));
    }

    multisig_rows();
    amm_rows();
    p_token_rows();
}

/// What an instruction's compute units went to: the programs it called, the
/// calls themselves, and what is left, the program's own code and syscalls.
struct Ledger {
    total: u64,
    callees: u64,
    calls: u64,
    system: u64,
}

impl Ledger {
    /// A call costs this much before the callee runs (SIMD-0339).
    const CALL: u64 = 946;
    /// What the System Program charges for an instruction.
    const SYSTEM: u64 = 150;

    fn of(outcome: &quasar_test::Outcome, program: Address) -> Self {
        let (logs, own) = (outcome.logs(), program.to_string());
        let called = |line: &&String| line.contains("invoke [2]");
        let consumed = logs.iter().filter(|line| !line.contains(&own)).filter_map(|line| {
            line.split("consumed ").nth(1)?.split(' ').next()?.parse::<u64>().ok()
        });
        let system = logs.iter().filter(called).filter(|line| line.contains(&SYSTEM.to_string()));
        Self {
            total: outcome.compute_units(),
            callees: consumed.sum(),
            calls: logs.iter().filter(called).count() as u64,
            system: system.count() as u64 * Self::SYSTEM,
        }
    }

    fn row(&self, name: &str) {
        let Self { total, callees, calls, system } = self;
        let ours = total - callees - system - calls * Self::CALL;
        println!(
            "| {name} | {total} | {callees} | {calls} | {} | {system} | {ours} |",
            calls * Self::CALL
        );
    }
}

/// The escrow on p-token, which replaces the Token program (SIMD-0266), and
/// where its compute units go there.
fn p_token_rows() {
    print!(
        r#"
## On p-token

The escrow above with p-token, built from a pinned revision by
`bench/rivals.sh`, as the Token program. Initializing the accounts an
instruction creates in one call (p-token's `Batch`) was measured at 620
compute units less for `make` and declined: Token-2022 has no `Batch`, nor
do associated token accounts go through it.

| Implementation | make (CU) | take (CU) | refund (CU) | binary (bytes) |
|---|---:|---:|---:|---:|
"#
    );
    let p_token = Test::p_token_elf();
    type Build = (&'static str, fn() -> Test, &'static str);
    let builds: [Build; 2] = [
        ("Quasar, this repository", || Test::new(escrow::ID, "escrow"), "deploy/escrow"),
        (
            "Quasar master (b0de7db)",
            || Test::rival(escrow::ID, "quasar_escrow"),
            "rivals/quasar_escrow",
        ),
    ];
    let mut ledgers = Vec::new();
    for (name, build, path) in builds {
        let mut t = escrow_bench::prepare_on(build(), &p_token);
        let make = t.run(escrow_bench::make()).succeeds();
        let take = t.run(escrow_bench::take()).succeeds();
        let mut t = escrow_bench::prepare_on(build(), &p_token);
        t.run(escrow_bench::make()).succeeds();
        let refund = t.run(escrow_bench::refund()).succeeds();
        let [a, b, c] = [&make, &take, &refund].map(|outcome| outcome.compute_units());
        println!("| {name} | {a} | {b} | {c} | {} |", size(path));
        for (instruction, outcome) in [("make", &make), ("take", &take), ("refund", &refund)] {
            ledgers.push((format!("{name}, {instruction}"), Ledger::of(outcome, escrow::ID)));
        }
    }
    print!(
        r#"
What each instruction's compute units go to: the programs it calls, the
calls themselves at {call} each, the System Program at {system} an
instruction, and what is left: the program's own code and its syscalls.

| Instruction | total | called programs | calls | calls (CU) | System | left |
|---|---:|---:|---:|---:|---:|---:|
"#,
        call = Ledger::CALL,
        system = Ledger::SYSTEM,
    );
    for (name, ledger) in ledgers {
        ledger.row(&name);
    }
}

/// Initialize a pool with a 0.3% fee, deposit, swap, withdraw.
fn amm_rows() {
    use amm::{Deposit, Fee, ID, Trade, Withdraw};
    use quasar_lang::wrapper::ProgramId;
    use quasar_spl::token::Token;
    use solana_program_pack::Pack;
    use spl_token_interface::state::{Account as TokenAccount, AccountState, Mint};
    print!(
        r#"
## AMM

`examples/amm`, a constant-product pool on the SPL Token program; no other
framework here has one. Initialize creates the pool, its two vaults and its
LP mint; the deposit is the first.

| Implementation | initialize (CU) | deposit (CU) | swap (CU) | withdraw (CU) | binary (bytes) |
|---|---:|---:|---:|---:|---:|
"#
    );
    for (name, mut t, path) in [
        ("Quasar, this repository", Test::new(ID, "amm"), "deploy/amm"),
        (EVERY_CHECK, Test::debug(ID, "amm"), "debug-checks/deploy/amm"),
    ] {
        t.add_elf(Token::ID, mollusk_svm_programs_token::token::ELF);
        let [owner, mint_a, mint_b, vault_a, vault_b, lp_mint, owner_a, owner_b, owner_lp] =
            [1, 4, 5, 6, 7, 8, 9, 10, 11].map(|byte| Address::new_from_array([byte; 32]));
        let pool =
            Address::find_program_address(&[b"pool", mint_a.as_ref(), mint_b.as_ref()], &ID).0;
        let token_account = |mint, amount| {
            let mut data = vec![0; TokenAccount::LEN];
            let state = AccountState::Initialized;
            TokenAccount { mint, owner, amount, state, ..TokenAccount::default() }
                .pack_into_slice(&mut data);
            data
        };
        t.set(owner, SYSTEM, 10_000_000_000, vec![]);
        for (mint, owner_account) in [(mint_a, owner_a), (mint_b, owner_b)] {
            let mut data = vec![0; Mint::LEN];
            Mint { supply: 1_000_000, decimals: 6, is_initialized: true, ..Mint::default() }
                .pack_into_slice(&mut data);
            t.set(mint, Token::ID, t.rent(Mint::LEN), data);
            t.set(
                owner_account,
                Token::ID,
                t.rent(TokenAccount::LEN),
                token_account(mint, 1_000_000),
            );
        }
        for account in [pool, vault_a, vault_b, lp_mint] {
            t.set(account, SYSTEM, 0, vec![]);
        }
        let accounts = (owner, mint_a, mint_b, pool, vault_a, vault_b, lp_mint, Token::ID, SYSTEM);
        let initialize = t
            .run(amm::initialize::instruction(accounts, Fee { fee_bps: 30 }))
            .succeeds()
            .compute_units();
        t.set(owner_lp, Token::ID, t.rent(TokenAccount::LEN), token_account(lp_mint, 0));
        let liquidity =
            (owner, pool, lp_mint, owner_a, owner_b, owner_lp, vault_a, vault_b, Token::ID);
        let swap_accounts = (owner, pool, owner_a, owner_b, vault_a, vault_b, Token::ID);
        let [deposit, swap, withdraw] = [
            amm::deposit::instruction(
                liquidity,
                Deposit { max_a: 10_000, max_b: 10_000, min_shares: 0 },
            ),
            amm::swap::instruction(
                swap_accounts,
                Trade { amount_in: 1_000, min_out: 0, a_for_b: true },
            ),
            amm::withdraw::instruction(liquidity, Withdraw { shares: 5_000, min_a: 0, min_b: 0 }),
        ]
        .map(|ix| t.run(ix).succeeds().compute_units());
        let size = size(path);
        println!("| {name} | {initialize} | {deposit} | {swap} | {withdraw} | {size} |");
    }
}

/// Create a multisig of three members needing two, deposit, then pay out
/// with two approvals.
fn multisig_rows() {
    use multisig::{Amount, ID, Threshold};
    print!(
        r#"
## Multisig

Quasar master's multisig example, unmodified, and the same multisig written
here (`examples/multisig`): three members, a threshold of two. The rewrite
sizes the configuration to its content.

| Implementation | create (CU) | deposit (CU) | execute (CU) | binary (bytes) |
|---|---:|---:|---:|---:|
"#
    );
    let creator = USER;
    let config = Address::find_program_address(&[b"multisig", creator.as_ref()], &ID).0;
    let vault = Address::find_program_address(&[b"vault", config.as_ref()], &ID).0;
    let members: Vec<Address> =
        (0..3).map(|index| Address::new_from_array([10 + index; 32])).collect();
    let recipient = Address::new_from_array([2; 32]);
    for (name, mut t, path) in [
        ("Quasar, this repository", Test::new(ID, "multisig"), "deploy/multisig".to_owned()),
        (
            "Quasar master (b0de7db)",
            Test::rival(ID, "quasar_multisig"),
            "rivals/quasar_multisig".to_owned(),
        ),
    ] {
        for account in [creator, recipient].iter().chain(&members) {
            t.set(*account, SYSTEM, 10_000_000_000, vec![]);
        }
        for account in [config, vault] {
            t.set(account, SYSTEM, 0, vec![]);
        }
        let create = multisig::create::instruction(
            (creator, config, Rent::ID, SYSTEM, members.clone()),
            Threshold { threshold: 2 },
        );
        let deposit = multisig::deposit::instruction(
            (creator, config, vault, SYSTEM),
            Amount { amount: 5_000 },
        );
        let approvals = vec![members[0], members[2]];
        let execute = multisig::execute_transfer::instruction(
            (config, creator, vault, recipient, SYSTEM, approvals),
            Amount { amount: 3_000 },
        );
        let [create, deposit, execute] =
            [create, deposit, execute].map(|ix| t.run(ix).succeeds().compute_units());
        println!("| {name} | {create} | {deposit} | {execute} | {} |", size(&path));
    }
}

fn size(path: &str) -> u64 {
    let elf = quasar_test::target_dir().join(format!("{path}.so"));
    std::fs::metadata(elf).expect("built").len()
}

/// The vault of `USER`.
fn vault() -> Address {
    Address::find_program_address(&[b"vault", USER.as_ref()], &ID).0
}

/// Deposits then withdraws, with the vault owned by `vault_owner`, and prints
/// the compute units of each and the binary's size.
fn row(name: &str, mut t: Test, vault_owner: Address, instructions: [Instruction; 2], path: &str) {
    t.set(USER, SYSTEM, 10_000_000_000, vec![]);
    t.set(vault(), vault_owner, 0, vec![]);
    let [deposit, withdraw] = instructions.map(|ix| t.run(ix).succeeds().compute_units());
    println!("| {name} | {deposit} | {withdraw} | {} |", size(path));
}
