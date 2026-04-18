#[test]
fn compile_pass_tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/compile_pass/*.rs");
}
