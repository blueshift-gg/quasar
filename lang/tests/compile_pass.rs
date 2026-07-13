#[test]
fn compile_pass_tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/compile_pass/*.rs");
}

#[test]
fn readme_quick_start_matches_compile_fixture() {
    let readme = include_str!("../../README.md");
    let quick_start = readme
        .split_once("## Quick Start")
        .expect("README must contain a Quick Start section")
        .1;
    let snippet = quick_start
        .split_once("```rust\n")
        .expect("Quick Start must contain a Rust code block")
        .1
        .split_once("\n```")
        .expect("Quick Start Rust code block must be closed")
        .0
        .trim();

    let fixture = include_str!("compile_pass/readme_quick_start.rs");
    let fixture_snippet = fixture
        .split_once("// README_QUICK_START_BEGIN")
        .expect("compile fixture must contain the begin marker")
        .1
        .split_once("// README_QUICK_START_END")
        .expect("compile fixture must contain the end marker")
        .0
        .trim();

    assert_eq!(snippet, fixture_snippet);
}
