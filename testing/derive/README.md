# quasar-test-derive

Attribute macros for [`quasar-test`](https://crates.io/crates/quasar-test).
Depend on `quasar-test` instead of this crate; it re-exports `#[quasar_test]`.

```rust,ignore
use quasar_test::prelude::*;

#[quasar_test]
fn initialize(q: &mut QuasarTest) {
    let payer = q.actor();
    q.send(InitializeInstruction { payer }).succeeds();
}
```

A `#[quasar_test]` function is a plain `#[test]` whose world is loaded from
the crate's compiled program (`target/deploy/{crate_name}.so`, or the
artifact `quasar test` supplies). Use
`#[quasar_test(program_id = EXPR)]` for an external program.
