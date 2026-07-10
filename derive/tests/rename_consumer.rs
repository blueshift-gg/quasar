//! Crate-identity smoke test (Workstream H1).
//!
//! `tests/rename-consumer/` is a standalone crate that depends on `quasar-lang`
//! under the RENAMED package `ql` and does not `use ql::prelude::*`. If any
//! macro emits a hard-coded `quasar_lang::` path (broken by the rename) or a
//! bare prelude *name* (broken without the glob), `cargo check` there fails.
//!
//! Before H1's fully-qualified emission this crate did NOT compile; keeping the
//! check green guards against regressions in generated-code hygiene.

use std::process::Command;

#[test]
fn rename_consumer_compiles() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rename-consumer");
    let status = Command::new(env!("CARGO"))
        .args(["check", "--quiet"])
        .current_dir(dir)
        .status()
        .expect("failed to spawn `cargo check` for the rename-consumer fixture");
    assert!(
        status.success(),
        "rename-consumer failed to compile: generated code is not rename-safe (a `quasar_lang::` \
         path or a bare prelude name leaked into an emitter). Run `cargo check` in {dir} to see \
         the error."
    );
}
