#[test]
fn compile_fail_tests() {
    // Set check-cfg for solana target_os so trybuild doesn't warn about it
    std::env::set_var(
        "RUSTFLAGS",
        "--check-cfg=cfg(target_os,values(\"solana\"))",
    );

    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
