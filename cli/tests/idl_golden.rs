//! Golden + determinism guard for IDL generation.
//!
//! Builds the `examples/multisig` IDL and asserts (1) two independent builds
//! are byte-identical (the deterministic-assembly sort in `build_idl` must
//! hold) and (2) the output matches a committed golden. Regenerate the golden
//! after an intentional IDL-affecting change with `UPDATE_GOLDEN=1` and review
//! the diff.

use {
    quasar_idl::types::{canonical_json_pretty, Idl},
    std::{fs, path::PathBuf},
};

const EXPECTED_ABI_HASH: &str = "b9a73ab5b29c6bd7d42fdeb8c67f353dac97d4fe0d0c8a530ed6cd38413506c0";

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/multisig.idl.json")
}

#[test]
fn multisig_idl_builds_deterministically_and_matches_golden() {
    let fixture = workspace_root().join("examples/multisig");

    let first = quasar_cli::idl::build(&fixture).expect("build multisig IDL");
    let second = quasar_cli::idl::build(&fixture).expect("re-build multisig IDL");

    let first_bytes = canonical_json_pretty(&first).expect("serialize IDL");
    let second_bytes = canonical_json_pretty(&second).expect("serialize IDL again");
    assert_eq!(
        first_bytes, second_bytes,
        "IDL build must be byte-identical across builds (deterministic assembly sort)"
    );

    let path = golden_path();
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        fs::create_dir_all(path.parent().expect("golden dir")).expect("create golden dir");
        fs::write(&path, &first_bytes).expect("write golden");
    }

    let golden = fs::read(&path).unwrap_or_else(|_| {
        panic!(
            "committed golden IDL missing at {}; regenerate with UPDATE_GOLDEN=1",
            path.display()
        )
    });
    let golden_idl: Idl = serde_json::from_slice(&golden).expect("golden must be valid IDL JSON");
    let expected_generator_version = env!("CARGO_PKG_VERSION");
    assert_eq!(
        first.metadata.generator_version.as_deref(),
        Some(expected_generator_version),
        "generated metadata.generatorVersion must match the CLI package version"
    );
    assert_eq!(
        golden_idl.metadata.generator_version.as_deref(),
        Some(expected_generator_version),
        "committed golden metadata.generatorVersion is stale; regenerate with UPDATE_GOLDEN=1 and \
         review the full-IDL hash change"
    );
    assert_eq!(
        first.hashes.as_ref().map(|hashes| hashes.abi.as_str()),
        Some(EXPECTED_ABI_HASH),
        "the multisig ABI changed; review it separately from generator metadata"
    );
    assert_eq!(
        golden_idl.hashes.as_ref().map(|hashes| hashes.abi.as_str()),
        Some(EXPECTED_ABI_HASH),
        "the committed golden ABI hash drifted unexpectedly"
    );
    assert_eq!(
        String::from_utf8_lossy(&first_bytes),
        String::from_utf8_lossy(&golden),
        "multisig IDL drifted from the committed golden; if intentional, regenerate with \
         UPDATE_GOLDEN=1 and review the diff"
    );
}
