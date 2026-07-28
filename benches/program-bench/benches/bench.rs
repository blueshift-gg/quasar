//! CU benchmark driver.
//!
//! Runs every instruction in `programs/quasar-bench` through `quasar-test`,
//! records `compute_units`, and writes `BENCHMARK.md`. Build the program first:
//!
//! ```bash
//! cargo build-sbf --manifest-path programs/quasar-bench/Cargo.toml
//! cargo bench
//! ```
//!
//! Instructions are built with the quasar-generated Rust client (`client/`), so
//! account order, signer/mut flags, discriminators, arg encoding, and PDA seeds
//! come from the IDL instead of being restated here. Regenerate it with
//! `quasar idl programs/quasar-bench` — see `README.md`. Only account *state*
//! setup (fixtures, raw dynamic-account bytes) is local.
use {
    quasar_bench::{
        ClockSnapshot, ClockSnapshotData, Data, DataPdaDyn, SimpleAccount, SimpleAccountData,
        TwoDynArgsAccountData, ID,
    },
    quasar_bench_client as client,
    quasar_lang::client::{wincode, DynString, DynVec},
    quasar_test::{prelude::*, PROGRAM_PATH_ENV},
    std::{
        collections::HashMap,
        fs,
        io::IsTerminal,
        path::{Path, PathBuf},
        process::Command,
    },
};

/// A measured instruction: its CU, or why it never reached the success path.
type Row = (&'static str, Result<u64, String>);

/// The build this driver measures. Also the source of the reported binary size.
/// `cargo build-sbf` writes to the workspace target dir, which is this
/// manifest's now that the program is a member of this workspace.
const PROGRAM_SO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/target/deploy/quasar_bench.so");

fn mk() -> Test {
    Test::new(ID)
}

fn note(rows: &mut Vec<Row>, name: &'static str, out: &Outcome) {
    // The reason travels with the row rather than printing here: the report
    // shows it beside the FAIL cell instead of scrolled off above the table.
    rows.push((
        name,
        match out.is_ok() {
            true => Ok(out.compute_units()),
            false => Err(format!("{:?}", out.error())),
        },
    ));
}

/// Short commit the numbers came from, so an archived table stays attributable.
///
/// Shells out rather than taking a build-time dep like `vergen`: two commands
/// beat a proc-macro crate in the graph. Returns `None` outside a repo or with
/// no git on PATH — the table is still valid, just unattributed. The `-dirty`
/// suffix covers only this benchmark's own tree, since that is what determines
/// the numbers.
fn git_revision() -> Option<String> {
    let dir = env!("CARGO_MANIFEST_DIR");
    let git = |args: &[&str]| Command::new("git").args(args).current_dir(dir).output().ok();

    let head = git(&["rev-parse", "--short", "HEAD"])?;
    if !head.status.success() {
        return None;
    }
    let sha = String::from_utf8(head.stdout).ok()?.trim().to_string();

    let dirty = git(&["status", "--porcelain", "--", dir, ":(exclude)BENCHMARK.md"])
        .is_some_and(|out| !out.stdout.is_empty());
    Some(if dirty { format!("{sha}-dirty") } else { sha })
}

/// The `SimpleAccount` PDA for `authority`, planted with `value`.
///
/// Returns its address: several instructions derive it from the authority, but
/// `realloc_check` and `cpi_mut_readback` take it as an explicit account.
fn seed_simple_account(t: &mut Test, authority: Pubkey, value: u64) -> Pubkey {
    let (account, bump) = t.derive_pda_with_bump(SimpleAccount::seeds(&authority));
    t.write(
        account,
        SimpleAccountData {
            authority,
            value: value.into(),
            bump,
        },
    );
    account
}

/// Serialize an account body with the generated client, then plant it.
///
/// The client's `SchemaWrite` impls emit the discriminator, the length
/// prefixes, and the payload — the whole on-chain layout — so nothing about the
/// encoding is restated here. Regenerating the client after a state change
/// turns a layout mismatch into a compile error rather than a `FAIL` row.
fn seed<T>(t: &mut Test, addr: Pubkey, account: &T)
where
    T: wincode::SchemaWrite<wincode::config::DefaultConfig, Src = T>,
{
    let data = wincode::serialize(account).expect("serialize account fixture");
    t.set_account(Account::new(addr, ID, 1_000_000, data));
}

fn main() {
    std::env::set_var(PROGRAM_PATH_ENV, PROGRAM_SO);

    let spl = SPL_TOKEN_PROGRAM_ID;
    let mut rows: Vec<Row> = Vec::new();

    // ---------------- 0..=9 canonical ----------------
    {
        let mut t = mk();
        let out = t.send(client::PingInstruction {});
        note(&mut rows, "ping", &out);
    }
    {
        let mut t = mk();
        let out = t.send(client::LogInstruction {});
        note(&mut rows, "log", &out);
    }
    {
        let mut t = mk();
        let admin = t.add(Wallet::new());
        let account = t.fresh_address();
        let out = t.send(client::CreateAccountInstruction { admin, account });
        note(&mut rows, "create_account", &out);
    }
    {
        let mut t = mk();
        let payer = t.add(Wallet::new());
        let account = t.add(Wallet::new());
        let out = t.send(client::TransferInstruction {
            payer,
            account,
            amount: 100,
        });
        note(&mut rows, "transfer", &out);
    }
    {
        let mut t = mk();
        let a: [Pubkey; 10] = core::array::from_fn(|_| t.fresh_address());
        let out = t.send(client::UncheckedAccountsInstruction {
            account1: a[0],
            account2: a[1],
            account3: a[2],
            account4: a[3],
            account5: a[4],
            account6: a[5],
            account7: a[6],
            account8: a[7],
            account9: a[8],
            account10: a[9],
        });
        note(&mut rows, "unchecked_accounts", &out);
    }
    {
        let mut t = mk();
        let d = t.fresh_address();
        seed(&mut t, d, &client::Data { byte: 0, bump: 0 });
        let out = t.send(client::AccountsInstruction {
            account1: d,
            account2: d,
            account3: d,
            account4: d,
            account5: d,
            account6: d,
            account7: d,
            account8: d,
            account9: d,
            account10: d,
        });
        note(&mut rows, "accounts", &out);
    }
    {
        let mut t = mk();
        let admin = t.add(Wallet::new());
        let out = t.send(client::CreatePdaStaticInstruction { admin });
        note(&mut rows, "create_pda_static", &out);
    }
    {
        let mut t = mk();
        let (pda, bump) = t.derive_pda_with_bump(Data::seeds());
        seed(&mut t, pda, &client::Data { byte: 0, bump });
        let out = t.send(client::VerifyPdaStaticInstruction {});
        note(&mut rows, "verify_pda_static", &out);
    }
    {
        let mut t = mk();
        let admin = t.add(Wallet::new());
        let authority = t.fresh_address();
        let out = t.send(client::CreatePdaDynamicInstruction { admin, authority });
        note(&mut rows, "create_pda_dynamic", &out);
    }
    {
        let mut t = mk();
        let authority = t.fresh_address();
        let (pda, bump) = t.derive_pda_with_bump(DataPdaDyn::seeds(&authority));
        seed(&mut t, pda, &client::Data { byte: 0, bump });
        let out = t.send(client::VerifyPdaDynamicInstruction { authority });
        note(&mut rows, "verify_pda_dynamic", &out);
    }

    // ---------------- 22..=27 dynamic ----------------
    {
        let mut t = mk();
        let account = t.fresh_address();
        seed(
            &mut t,
            account,
            &client::DynamicAccount {
                name: DynString::new("hi"),
                tags: DynVec::new(vec![]),
            },
        );
        let out = t.send(client::DynamicReadbackInstruction {
            account,
            expected_name_len: 2,
            expected_tags_count: 0,
        });
        note(&mut rows, "dynamic_readback", &out);
    }
    {
        let mut t = mk();
        let account = t.fresh_address();
        seed(
            &mut t,
            account,
            &client::DynamicAccount {
                name: DynString::new("hi"),
                tags: DynVec::new(vec![]),
            },
        );
        let payer = t.add(Wallet::new());
        let out = t.send(client::DynamicMutateInstruction {
            account,
            payer,
            new_name: DynString::new("world"),
        });
        note(&mut rows, "dynamic_mutate", &out);
    }
    {
        let mut t = mk();
        let account = t.fresh_address();
        seed(
            &mut t,
            account,
            &client::DynamicAccount {
                name: DynString::new("hi"),
                tags: DynVec::new(vec![]),
            },
        );
        let payer = t.add(Wallet::new());
        let tag = t.fresh_address();
        let out = t.send(client::DynamicViewMutInstruction {
            account,
            payer,
            new_name: DynString::new("xy"),
            new_tags: DynVec::new(vec![tag]),
        });
        note(&mut rows, "dynamic_view_mut", &out);
    }
    {
        let mut t = mk();
        let account = t.fresh_address();
        t.write(
            account,
            TwoDynArgsAccountData {
                tag: 0u64.into(),
                a_len: 0,
                a: 0u64.into(),
                b_len: 0,
                b: 0u64.into(),
            },
        );
        let out = t.send(client::TwoDynInstruction {
            account,
            tag: 7,
            a: DynString::new("aa"),
            b: DynString::new("zz"),
        });
        note(&mut rows, "two_dyn", &out);
    }
    {
        let mut t = mk();
        let account = t.fresh_address();
        let authority = t.fresh_address();
        seed(
            &mut t,
            account,
            &client::DynBytesAccount {
                authority,
                data: DynVec::new(vec![1, 2, 3, 4, 5]),
            },
        );
        let out = t.send(client::DynBytesCheckInstruction {
            account,
            expected_len: 5,
        });
        note(&mut rows, "dyn_bytes_check", &out);
    }
    {
        let mut t = mk();
        let account = t.fresh_address();
        let authority = t.fresh_address();
        seed(
            &mut t,
            account,
            &client::DynStrAccount {
                authority,
                label: DynString::new("hello"),
            },
        );
        let out = t.send(client::DynStrCheckInstruction {
            account,
            expected_len: 5,
        });
        note(&mut rows, "dyn_str_check", &out);
    }

    // ---------------- 28..=33 lifecycle ----------------
    {
        let mut t = mk();
        let payer = t.add(Wallet::new());
        let new_account = t.fresh_address();
        let owner = t.fresh_address();
        let out = t.send(client::CreateAccountCpiInstruction {
            payer,
            new_account,
            lamports: 1_000_000,
            space: 100,
            owner,
        });
        note(&mut rows, "create_account_cpi", &out);
    }
    {
        let mut t = mk();
        let authority = t.add(Wallet::new());
        seed_simple_account(&mut t, authority, 42);
        let out = t.send(client::CloseAccountInstruction { authority });
        note(&mut rows, "close_account", &out);
    }
    {
        let mut t = mk();
        let authority = t.add(Wallet::new());
        let account = seed_simple_account(&mut t, authority, 42);
        let payer = t.add(Wallet::new());
        let out = t.send(client::ReallocCheckInstruction {
            account,
            payer,
            new_space: 100,
        });
        note(&mut rows, "realloc_check", &out);
    }
    {
        let mut t = mk();
        let account = t.add(Wallet::new());
        let owner = t.fresh_address();
        let out = t.send(client::AssignTestInstruction { account, owner });
        note(&mut rows, "assign_test", &out);
    }
    {
        let mut t = mk();
        let payer = t.add(Wallet::new());
        let out = t.send(client::SpaceOverrideInstruction { payer, value: 42 });
        note(&mut rows, "space_override", &out);
    }

    // ---------------- 40..=51 token ----------------
    {
        let mut t = mk();
        let payer = t.add(Wallet::new());
        let mint_authority = t.add(Wallet::new());
        let mint = t.fresh_address();
        let out = t.send(client::InitMintInstruction {
            payer,
            mint,
            mint_authority,
        });
        note(&mut rows, "init_mint", &out);
    }
    {
        let mut t = mk();
        let payer = t.add(Wallet::new());
        let mint = t.add(Mint::new(payer).decimals(6));
        let token_account = t.fresh_address();
        let out = t.send(client::InitTokenAccountInstruction {
            payer,
            token_account,
            mint,
        });
        note(&mut rows, "init_token_account", &out);
    }
    {
        let mut t = mk();
        let payer = t.add(Wallet::new());
        let wallet = t.add(Wallet::new());
        let mint = t.add(Mint::new(payer).decimals(6));
        let out = t.send(client::InitAtaInstruction {
            payer,
            wallet,
            mint,
        });
        note(&mut rows, "init_ata", &out);
    }
    {
        let mut t = mk();
        let authority = t.add(Wallet::new());
        let mint = t.add(Mint::new(authority).decimals(6));
        let from = t.add(TokenAccount::new(mint, authority).amount(500));
        let to = t.add(TokenAccount::new(mint, authority).amount(0));
        let out = t.send(client::TransferCheckedInstruction {
            authority,
            from,
            mint,
            to,
            amount: 200,
            decimals: 6,
        });
        note(&mut rows, "transfer_checked", &out);
    }
    {
        let mut t = mk();
        let authority = t.add(Wallet::new());
        let mint = t.add(Mint::new(authority).decimals(6));
        let to = t.add(TokenAccount::new(mint, authority).amount(0));
        let out = t.send(client::MintToInstruction {
            authority,
            mint,
            to,
            amount: 5000,
        });
        note(&mut rows, "mint_to", &out);
    }
    {
        let mut t = mk();
        let authority = t.add(Wallet::new());
        let mint = t.add(Mint::new(authority).decimals(6).supply(1000));
        let from = t.add(TokenAccount::new(mint, authority).amount(1000));
        let out = t.send(client::BurnInstruction {
            authority,
            from,
            mint,
            amount: 500,
        });
        note(&mut rows, "burn", &out);
    }
    {
        let mut t = mk();
        let authority = t.add(Wallet::new());
        let mint = t.add(Mint::new(authority).decimals(6));
        let source = t.add(TokenAccount::new(mint, authority).amount(1000));
        let delegate = t.fresh_address();
        let out = t.send(client::ApproveInstruction {
            authority,
            source,
            delegate,
            amount: 500,
        });
        note(&mut rows, "approve", &out);
    }
    {
        let mut t = mk();
        let authority = t.add(Wallet::new());
        let mint = t.add(Mint::new(authority).decimals(6));
        let source = t.add(TokenAccount::new(mint, authority).amount(1000));
        let out = t.send(client::RevokeInstruction { authority, source });
        note(&mut rows, "revoke", &out);
    }
    {
        let mut t = mk();
        let authority = t.add(Wallet::new());
        let mint = t.add(Mint::new(authority).decimals(6));
        let account = t.add(TokenAccount::new(mint, authority).amount(0));
        let out = t.send(client::CloseTokenAccountInstruction {
            account,
            destination: authority,
            authority,
        });
        note(&mut rows, "close_token_account", &out);
    }
    {
        let mut t = mk();
        let authority = t.add(Wallet::new());
        let mint = t.add(Mint::new(authority).decimals(6));
        let source = t.add(TokenAccount::new(mint, authority).amount(1000));
        let receiver = t.add(TokenAccount::new(mint, authority).amount(0));
        let destination = t.fresh_address();
        let out = t.send(client::SweepAndCloseInstruction {
            authority,
            source,
            receiver,
            mint,
            destination,
        });
        note(&mut rows, "sweep_and_close", &out);
    }
    {
        let mut t = mk();
        let payer = t.add(Wallet::new());
        let mint_authority = t.add(Wallet::new());
        let mint = t.fresh_address();
        let out = t.send(client::InitMintT22Instruction {
            payer,
            mint,
            mint_authority,
        });
        note(&mut rows, "init_mint_t22", &out);
    }
    {
        let mut t = mk();
        let authority = t.add(Wallet::new());
        let mint = t.add(Mint::new(authority).decimals(6));
        let from = t.add(TokenAccount::new(mint, authority).amount(500));
        let to = t.add(TokenAccount::new(mint, authority).amount(0));
        let out = t.send(client::TransferCheckedInterfaceInstruction {
            authority,
            from,
            mint,
            to,
            token_program: spl,
            amount: 200,
            decimals: 6,
        });
        note(&mut rows, "transfer_checked_interface", &out);
    }

    // ---------------- 80..=92 validation ----------------
    {
        let mut t = mk();
        let account = t.add(Wallet::new());
        let out = t.send(client::HeaderNodupSignerInstruction { account });
        note(&mut rows, "header_nodup_signer", &out);
    }
    {
        let mut t = mk();
        let a = t.add(Wallet::new());
        let out = t.send(client::HeaderDupSignerInstruction {
            payer: a,
            authority: a,
        });
        note(&mut rows, "header_dup_signer", &out);
    }
    {
        let mut t = mk();
        let a = t.fresh_address();
        let b = t.fresh_address();
        seed(&mut t, a, &client::ErrorTestAccount { authority: a, value: 1 });
        seed(&mut t, b, &client::ErrorTestAccount { authority: b, value: 2 });
        let out = t.send(client::TwoAccountsCheckInstruction {
            first: a,
            second: b,
        });
        note(&mut rows, "two_accounts_check", &out);
    }
    {
        let mut t = mk();
        let authority = t.add(Wallet::new());
        let account = t.fresh_address();
        seed(
            &mut t,
            account,
            &client::ErrorTestAccount {
                authority,
                value: 7,
            },
        );
        let out = t.send(client::HasOneDefaultInstruction { authority, account });
        note(&mut rows, "has_one_default", &out);
    }
    {
        let mut t = mk();
        let authority = t.add(Wallet::new());
        let account = seed_simple_account(&mut t, authority, 1);
        let out = t.send(client::OwnerCheckInstruction { account });
        note(&mut rows, "owner_check", &out);
    }
    {
        let mut t = mk();
        let out = t.send(client::ProgramCheckInstruction {});
        note(&mut rows, "program_check", &out);
    }
    {
        let mut t = mk();
        let signer = t.add(Wallet::new());
        let out = t.send(client::RequireEqCheckInstruction { signer, a: 1, b: 1 });
        note(&mut rows, "require_eq_check", &out);
    }
    {
        let mut t = mk();
        let authority = t.add(Wallet::new());
        let out = t.send(client::RemainingTypedCheckInstruction {
            authority,
            remaining_accounts: vec![],
        });
        note(&mut rows, "remaining_typed_check", &out);
    }

    // ---------------- 93..=98 events ----------------
    {
        let mut t = mk();
        let signer = t.add(Wallet::new());
        let out = t.send(client::EmitEmptyEventInstruction { signer });
        note(&mut rows, "emit_empty_event", &out);
    }
    {
        let mut t = mk();
        let signer = t.add(Wallet::new());
        let out = t.send(client::EmitU64EventInstruction { signer, value: 42 });
        note(&mut rows, "emit_u64_event", &out);
    }
    {
        let mut t = mk();
        let signer = t.add(Wallet::new());
        let c = t.fresh_address();
        let out = t.send(client::EmitMultiFieldInstruction {
            signer,
            a: 1,
            b: 2,
            c,
        });
        note(&mut rows, "emit_multi_field", &out);
    }
    {
        let mut t = mk();
        let signer = t.add(Wallet::new());
        let e = t.fresh_address();
        let f = t.fresh_address();
        let out = t.send(client::EmitLargeEventInstruction {
            signer,
            a: 1,
            b: 2,
            c: 3,
            d: 4,
            e,
            f,
            g: 7,
            h: 8,
        });
        note(&mut rows, "emit_large_event", &out);
    }
    {
        let mut t = mk();
        let signer = t.add(Wallet::new());
        let out = t.send(client::EmitTwoEventsInstruction {
            signer,
            first: 1,
            second: 2,
        });
        note(&mut rows, "emit_two_events", &out);
    }
    {
        let mut t = mk();
        let signer = t.add(Wallet::new());
        let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &ID);
        let out = t.send(client::EmitViaCpiInstruction {
            signer,
            event_authority,
            value: 42,
        });
        note(&mut rows, "emit_via_cpi", &out);
    }

    // ---------------- 99..=100 heap ----------------
    {
        let mut t = mk();
        let signer = t.add(Wallet::new());
        let out = t.send(client::HeapVecOkInstruction { signer });
        note(&mut rows, "heap_vec_ok", &out);
    }

    // ---------------- 101..=104 sysvar ----------------
    {
        let mut t = mk();
        let payer = t.add(Wallet::new());
        let out = t.send(client::ReadClockInstruction { payer });
        note(&mut rows, "read_clock", &out);
    }
    {
        let mut t = mk();
        let payer = t.add(Wallet::new());
        let (snapshot, _) = t.derive_pda_with_bump(ClockSnapshot::seeds());
        t.write(
            snapshot,
            ClockSnapshotData {
                slot: 0u64.into(),
                unix_timestamp: 0i64.into(),
            },
        );
        let out = t.send(client::ReadClockFromAccountInstruction { payer, snapshot });
        note(&mut rows, "read_clock_from_account", &out);
    }
    {
        let mut t = mk();
        let payer = t.add(Wallet::new());
        let out = t.send(client::ReadRentInstruction { payer });
        note(&mut rows, "read_rent", &out);
    }
    {
        let mut t = mk();
        let payer = t.add(Wallet::new());
        let out = t.send(client::ReadRentCalcInstruction { payer, data_len: 1 });
        note(&mut rows, "read_rent_calc", &out);
    }

    // ---------------- 105..=108 CPI ----------------
    {
        let mut t = mk();
        let out = t.send(client::ReturnU64Instruction {});
        note(&mut rows, "return_u64", &out);
    }
    {
        let mut t = mk();
        let out = t.send(client::CpiInvokeWithReturnInstruction {});
        note(&mut rows, "cpi_invoke_with_return", &out);
    }
    {
        let mut t = mk();
        let out = t.send(client::CpiInvokeIgnoreReturnInstruction {});
        note(&mut rows, "cpi_invoke_ignore_return", &out);
    }
    {
        let mut t = mk();
        let authority = t.add(Wallet::new());
        let account = seed_simple_account(&mut t, authority, 1);
        let payer = t.add(Wallet::new());
        let out = t.send(client::CpiMutReadbackInstruction {
            account,
            payer,
            new_value: 99,
        });
        note(&mut rows, "cpi_mut_readback", &out);
    }

    write_markdown(&rows);
}

/// The previous run's numbers, parsed back out of `BENCHMARK.md`.
///
/// The committed table *is* the baseline — no second state file to keep in sync
/// or forget to commit. Rows that failed last run parse as absent rather than
/// as a number, so they read as "new" instead of producing a bogus delta.
#[derive(Default)]
struct Baseline {
    cu: HashMap<String, u64>,
    binary_size: Option<u64>,
}

fn read_baseline(path: &Path) -> Baseline {
    let Ok(text) = fs::read_to_string(path) else {
        return Baseline::default();
    };
    let mut cu = HashMap::new();
    for line in text.lines() {
        // `| name | 123 |` splits to ["", "name", "123", ""]; the header and
        // the `| --- |` separator fail the parse and drop out.
        let mut cells = line.split('|').map(str::trim);
        if cells.next() != Some("") {
            continue;
        }
        if let (Some(name), Some(Ok(v))) = (cells.next(), cells.next().map(str::parse)) {
            cu.insert(name.to_string(), v);
        }
    }
    let binary_size = text
        .rsplit_once("*Binary size*: ")
        .and_then(|(_, tail)| tail.split_whitespace().next())
        .and_then(|n| n.parse().ok());
    Baseline { cu, binary_size }
}

fn write_markdown(rows: &[Row]) {
    let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/BENCHMARK.md"));
    // Must happen before the write below clobbers it.
    let baseline = read_baseline(&path);

    let mut ok = 0usize;
    let mut fail = 0usize;
    let mut out = String::from(
        "## Quasar CU Benchmark\n\nCompute units per instruction. Δ is the change against the previous run \
         of this table.\n\n| Instruction | CU | Δ |\n| --- | --- | --- |\n",
    );
    for (name, cu) in rows {
        match cu {
            Ok(v) => {
                ok += 1;
                let d = delta_md(name, *v, &baseline);
                out.push_str(&format!("| {name} | {v} | {d} |\n"));
            }
            Err(_) => {
                fail += 1;
                out.push_str(&format!("| {name} | FAIL | |\n"));
            }
        }
    }
    let binary_size = fs::metadata(PROGRAM_SO)
        .expect("stat program .so")
        .len();
    // Every instruction in the program is measured above, so this covers
    // exactly the rows in the table.
    out.push_str(&format!("\n*Binary size*: {binary_size} bytes."));
    if let Some(rev) = git_revision() {
        out.push_str(&format!(" *Commit*: `{rev}`."));
    }
    out.push('\n');

    fs::write(&path, out).expect("write BENCHMARK.md");

    print_report(rows, ok, fail, binary_size, &path, &baseline);
}

// ---------------------------------------------------------------------------
// Terminal report
// ---------------------------------------------------------------------------

/// ANSI SGR codes, or empty strings when styling is off.
struct Palette {
    reset: &'static str,
    dim: &'static str,
    bold: &'static str,
    cheap: &'static str,
    mid: &'static str,
    pricey: &'static str,
    heavy: &'static str,
    fail: &'static str,
}

impl Palette {
    /// Off when stderr is not a terminal (piped output, captured test runs) or
    /// `NO_COLOR` is set — see https://no-color.org.
    fn detect() -> Palette {
        let plain = Palette {
            reset: "",
            dim: "",
            bold: "",
            cheap: "",
            mid: "",
            pricey: "",
            heavy: "",
            fail: "",
        };
        if std::env::var_os("NO_COLOR").is_some() || !std::io::stderr().is_terminal() {
            return plain;
        }
        Palette {
            reset: "\x1b[0m",
            dim: "\x1b[2m",
            bold: "\x1b[1m",
            cheap: "\x1b[32m",  // green
            mid: "\x1b[36m",    // cyan
            pricey: "\x1b[33m", // yellow
            heavy: "\x1b[31m",  // red
            fail: "\x1b[1;31m",
        }
    }

    /// Cost bucket. The thresholds are order-of-magnitude landmarks, not
    /// budgets: <100 is framework overhead, >=5k means a CPI dominates.
    fn for_cu(&self, cu: u64) -> &'static str {
        match cu {
            ..100 => self.cheap,
            100..1_000 => self.mid,
            1_000..5_000 => self.pricey,
            _ => self.heavy,
        }
    }
}

/// Log-scaled bar, normalized across the observed `min..=max` range.
///
/// CU spans three orders of magnitude (22 for `ping`, 23153 for `init_ata`), so
/// a linear bar collapses everything below the token instructions into one
/// cell. Anchoring the low end at `min` rather than 1 spends the full width on
/// the range that actually occurs.
fn bar(cu: u64, min: u64, max: u64, width: usize) -> String {
    // A degenerate span (every row tied, or one row) has no gradient to show.
    // Fall back to the minimum bar rather than emitting nothing. Guarding the
    // span rather than clamping the denominator keeps a legitimately narrow
    // range — where `ln(max) - ln(min)` is under 1 — scaled honestly.
    let span = (max as f64).ln() - (min as f64).ln();
    let scale = if span > 0.0 {
        ((cu as f64).ln() - (min as f64).ln()) / span
    } else {
        0.0
    };
    let cells = ((scale * width as f64).round() as usize).clamp(1, width);
    "\u{2588}".repeat(cells)
}

/// How one row moved against the baseline. Computed once so the terminal and
/// the markdown cannot disagree about what counts as a regression.
enum Change {
    /// Nothing to compare against — first run, or the table was deleted.
    NoBaseline,
    /// Measured before, and unchanged.
    Same,
    /// Absent from the baseline — a new instruction, or one that failed last run.
    New,
    Moved { diff: i64, pct: f64, warn: bool },
}

fn change(name: &str, cu: u64, baseline: &Baseline) -> Change {
    if baseline.cu.is_empty() {
        return Change::NoBaseline;
    }
    let Some(&old) = baseline.cu.get(name) else {
        return Change::New;
    };
    let diff = cu as i64 - old as i64;
    if diff == 0 {
        return Change::Same;
    }
    let pct = diff as f64 / old as f64 * 100.0;
    Change::Moved {
        diff,
        pct,
        warn: diff >= REGRESSION_CU && pct >= REGRESSION_PCT,
    }
}

/// Trailing `(+120 / +1.6% ⚠)` cell for the terminal report.
fn delta_cell(p: &Palette, name: &str, cu: u64, baseline: &Baseline) -> String {
    match change(name, cu, baseline) {
        // Unchanged rows stay blank here: the terminal view is for spotting
        // what moved, and 56 rows of `(0)` would bury it.
        Change::NoBaseline | Change::Same => String::new(),
        Change::New => format!("  {d}(new){r}", d = p.dim, r = p.reset),
        Change::Moved { diff, pct, warn } => format!(
            "  {c}({diff:+} / {pct:+.1}%{w}){r}",
            c = if diff > 0 { p.heavy } else { p.cheap },
            w = if warn { " ⚠" } else { "" },
            r = p.reset,
        ),
    }
}

/// Same figure for the markdown Δ column, without the ANSI styling.
fn delta_md(name: &str, cu: u64, baseline: &Baseline) -> String {
    match change(name, cu, baseline) {
        // An explicit `0` reads as "measured, held steady". Blank is reserved
        // for the case where there was no previous number at all.
        Change::NoBaseline => String::new(),
        Change::Same => "0".into(),
        Change::New => "new".into(),
        Change::Moved { diff, pct, warn } => {
            format!("{diff:+} ({pct:+.1}%){}", if warn { " ⚠" } else { "" })
        }
    }
}

/// A ⚠ needs both thresholds. The percentage alone misfires on the cheap rows —
/// `ping` sits at ~21 CU, where the ±1–2 dispatch jitter is already 5–10% — and
/// the absolute floor alone would flag every token instruction's normal drift.
const REGRESSION_PCT: f64 = 1.0;
const REGRESSION_CU: i64 = 3;

fn print_report(
    rows: &[Row],
    ok: usize,
    fail: usize,
    binary_size: u64,
    path: &Path,
    baseline: &Baseline,
) {
    let p = Palette::detect();
    let p_ref = &p;
    let measured = || rows.iter().filter_map(|(_, cu)| cu.as_ref().ok().copied());
    let max_cu = measured().max().unwrap_or(1);
    let min_cu = measured().min().unwrap_or(1);
    let w = rows.iter().map(|(name, _)| name.len()).max().unwrap_or(0);

    let size_delta = match baseline.binary_size {
        Some(old) if old != binary_size => {
            let diff = binary_size as i64 - old as i64;
            let c = if diff > 0 { p.heavy } else { p.cheap };
            format!(" {c}({diff:+} B){r}", c = c, r = p.reset)
        }
        _ => String::new(),
    };
    eprintln!(
        "\n  {b}quasar CU benchmark{r}  {d}{} instructions · {:.1} KiB{r}{size_delta}\n",
        rows.len(),
        binary_size as f64 / 1024.0,
        b = p.bold,
        d = p.dim,
        r = p.reset,
    );

    for (name, cu) in rows {
        match cu {
            Ok(v) => eprintln!(
                "  {name:<w$}  {c}{v:>6}{r}  {c}{bar}{r}{delta}",
                c = p.for_cu(*v),
                r = p.reset,
                bar = bar(*v, min_cu, max_cu, 28),
                delta = delta_cell(p_ref, name, *v, baseline),
            ),
            Err(why) => eprintln!(
                "  {name:<w$}  {c}{:>6}{r}  {c}{why}{r}",
                "FAIL",
                c = p.fail,
                r = p.reset,
            ),
        }
    }

    let fail_cell = if fail == 0 {
        format!("{}0 failed{}", p.dim, p.reset)
    } else {
        format!("{}{fail} failed{}", p.fail, p.reset)
    };
    eprintln!(
        "\n  {g}✔ {ok} measured{r}  ·  {fail_cell}  ·  {d}wrote {}{r}\n",
        path.display(),
        g = p.cheap,
        d = p.dim,
        r = p.reset,
    );

    if fail > 0 {
        panic!("{fail} instruction(s) failed — see the table above");
    }
}
