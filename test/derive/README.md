# quasar-test-derive

Implementation crate for `quasar-test`'s attribute macro. Applications should
depend on `quasar-test`, which re-exports `#[quasar_test]` and resolves renamed
Cargo dependencies correctly.

```rust,ignore
use quasar_test::prelude::*;

#[quasar_test]
fn initialize(test: &mut Test) -> Result<(), Box<dyn std::error::Error>> {
    let payer = test.add(Wallet::new());
    test.send(InitializeInstruction { payer }).succeeds();
    Ok(())
}
```

The macro preserves normal Rust test attributes and return types. It loads the
current program artifact and supplies one isolated `Test` world.
