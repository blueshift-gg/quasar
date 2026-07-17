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
use quasar_test::prelude::*;

quasar_test! {
    fn deposits_into_the_vault(q) {
        let authority = q.actor();
        let vault = find_vault_address(&authority, &crate::ID).0;
        q.empty(vault);

        q.send(InitializeInstructionInput { authority }).succeeds();
        let deposited = q.send(DepositInstructionInput {
            authority,
            amount: 1_000_000_000,
        });
        deposited.succeeds().cu_below(10_000);

        let account = deposited.account(&vault).expect("vault exists");
        let ProgramAccount::Vault(state) = decode_account(&account.data).unwrap();
        assert_eq!(state.balance, 1_000_000_000);
    }
}
```

`actor`, `actors`, `actor_at`, `mint`, `ata`, and `empty` put common fixtures
directly into the test world. `send` executes generated client inputs without
an intermediate `Instruction` or account slice. The returned result supports
fluent success, typed error, compute-unit, balance, supply, and account-closure
checks. Raw fixtures and the full `QuasarSvm` API remain available for unusual
cases.

`quasar test` passes the exact program artifact through
`QUASAR_PROGRAM_PATH`. Direct `cargo test` runs discover one `.so` in the
nearest ancestor `target/deploy` directory. Discovery fails when the directory
contains multiple programs, so a test cannot silently execute the wrong
binary. Use `QuasarTest::from_program_path` when you need an explicit path.

`QuasarTest` dereferences to `QuasarSvm`, so the complete VM API remains
available. Use `quasar-svm` directly only when you are testing the VM itself or
building a non-Quasar integration.
