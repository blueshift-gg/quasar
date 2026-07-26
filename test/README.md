# quasar-test

`quasar-test` makes an on-chain program test read like a normal Rust test:

```toml
[dev-dependencies]
quasar-test = "=0.1.0"
```

```rust,ignore
use {crate::cpi::DepositInstruction, quasar_test::prelude::*};

#[quasar_test]
fn deposits_into_the_vault(test: &mut Test) {
    let authority = test.add(Wallet::account());

    test.execute(DepositInstruction {
        authority,
        amount: 1_000_000_000,
    })
    .succeeds()
    .checks([
        Cu::spent(|cu| cu <= 10_000),
        Account::lamports(authority, |x| x < DEFAULT_WALLET_LAMPORTS),
    ]);
}
```

`Wallet::account()` funds an actor with the default balance and returns its
address; `.fund(n)` sets an exact one. A signer a transaction names but never
installs is auto-funded on execute, so co-signers cost nothing extra.

`#[quasar_test]` is an ordinary `#[test]`: filters, `#[ignore]`,
`#[should_panic]`, and `Result<(), E>` work normally. `quasar test` builds the
program first and then runs the complete Cargo test graph. Direct `cargo test`
is also supported when a compiled artifact already exists in `target/deploy`.

Fixtures are values, and they compose as one algebra: tuples install a
heterogeneous world in one `add`, arrays repeat one fixture, and closures
receiving `&mut Test` are fixtures whose return value is the output — so a
protocol world is a plain function, and because it can register invariants
while it builds, the world polices itself:

```rust,ignore
let (maker, taker, [mint_a, mint_b]) = test.add((
    Wallet::account(),
    Wallet::account(),
    Mint::accounts().with_supply(1_000_000),
));

struct Pool {
    authority: Pubkey,
    mint: Pubkey,
    authority_tokens: Pubkey,
}

fn funded_pool() -> impl Fixture<Output = Pool> {
    |test: &mut Test| {
        let authority = test.add(Wallet::account());
        let mint = test.add(Mint::account().with_authority(authority).with_supply(1_000_000));
        let authority_tokens =
            test.add(AssociatedTokenAccount::account(mint, authority).with_amount(10_000));
        test.invariant(Mint::supply(mint, 1_000_000));
        Pool { authority, mint, authority_tokens }
    }
}

let pool = test.add(funded_pool());
```

`Dump::accounts([..])` and `Load::accounts(path)` copy real cluster state into
the world through the committed `.parallax/` store; the RPC endpoint comes from
`Test::builder(id).rpc(url)`.

Generated instructions derive canonical PDA, ATA, program, and sysvar accounts.
When an instruction has a derived account, the normal `{Name}Instruction` asks
only for caller-controlled values and `{Name}InstructionRaw` makes PDAs and
ATAs explicit for adversarial tests. Program and sysvar addresses remain fixed.

```rust,ignore
test.execute(WithdrawInstructionRaw {
    authority: attacker,
    vault: victim_vault,
    amount: 1,
})
.fails_with(VaultError::Unauthorized);
```

`execute` commits and `simulate` never does; both take a single instruction or
a chain as a tuple, array, or `Vec`. `succeeds()` yields the transaction
witness that `check`/`checks` run against — every fact takes its subject and
one expectation, a plain value meaning equality or a closure predicate — while
`fails`/`fails_with` yield a read-only failed witness. Accounts a transaction
names but the world has not installed are backfilled by role: a read-only
signer (a payer or co-signer) enters as a funded system account; a writable
non-signer enters as an empty init target, committed on success and leaving no
placeholder after failure.

Every sibling program in `target/deploy` with a matching `-keypair.json` is
preloaded for CPI. `test.add(Program::new(id, elf))` is the explicit option for
an artifact outside that bundle.

The SVM remains an implementation detail. Tests depend on Solana's public
semantics — fees are zero, rent and ownership are real — and the strict typed
state path (`read`, `write`, `State::of`) validates ownership, discriminator,
length, and zero-copy layout on every access.
