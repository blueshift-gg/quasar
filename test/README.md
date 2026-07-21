# quasar-test

`quasar-test` is the Rust test SDK for Quasar programs. It loads the program
built by `quasar test`, starts QuasarSVM, and provides the fixtures and
assertions used in program tests.

Your test crate needs one direct dependency:

```toml
[dev-dependencies]
quasar-test = "=0.1.0"
```

```rust,ignore
use {
    crate::{cpi::*, state::Vault},
    quasar_test::prelude::*,
};

#[quasar_test]
fn deposits_into_the_vault(q: &mut QuasarTest) {
    let authority = q.add_wallet();

    q.send(InitializeInstruction { authority }).succeeds();
    q.send(DepositInstruction {
        authority,
        amount: 1_000_000_000,
    })
    .succeeds()
    .cu_below(10_000);

    let state = q.read::<Vault>(q.derive_pda(Vault::seeds(&authority)));
    assert_eq!(state.balance, 1_000_000_000);
}
```

A `#[quasar_test]` function is a plain `#[test]` whose world is loaded from
the crate's compiled program. Everything typed comes from the program itself:
instructions from the generated client (which fills in `Program<T>`/
`Sysvar<T>` addresses and derives `#[seeds]` PDA accounts, so neither appears
in the struct), addresses via `derive_pda` from `#[seeds]`, state via `read`/`write`
from `#[account]`. `add_wallet`, `add_mint`, `add_token_account`, `add_ata`,
and `fund` put common fixtures directly into the test world, and `send` backs
missing writable accounts with empty system accounts so init targets need no
setup.
The returned result supports fluent success, typed error, compute-unit,
balance, supply, and account-closure checks. For a deliberate deviation from
the canonical call — a spoofed PDA, a missing signature — adjust the built
instruction with `swap_account`/`signed_by` so the deviation is visible where
the test constructs it. Raw fixtures and the full `QuasarSvm` API remain
available for unusual cases.

`quasar test` passes the exact program artifact through
`QUASAR_PROGRAM_PATH`. Direct `cargo test` runs prefer
`target/deploy/{crate_name}.so` in the nearest ancestor target directory and
otherwise require a single unambiguous `.so`, so a test cannot silently
execute the wrong binary. Use `#[quasar_test(program_id = EXPR)]` for an
external program and `QuasarTest::builder(id)` (config, explicit artifact
path, crate name) when setup needs control.

The API keeps one shape per concept: builders own setup options, `add_*`
methods mutate the world, `derive_*` methods only calculate addresses, and
`send`/`send_with`/`simulate` own execution. New helpers should add test
vocabulary rather than mirror methods already available on `QuasarSvm`.

`QuasarTest` dereferences to `QuasarSvm`, so the complete VM API remains
available. Use `quasar-svm` directly only when you are testing the VM itself or
building a non-Quasar integration.
