//! Versioned, documentation-free IDL wire-contract baselines.

use {
    quasar_idl::types::{canonical_abi_json_pretty, compute_abi_hash},
    std::path::{Path, PathBuf},
};

struct Profile {
    fixture: String,
    crate_path: String,
    baseline_file: String,
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn baseline_root() -> PathBuf {
    std::env::var_os("QUASAR_IDL_WIRE_BASELINE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("compatibility-baselines/v0.1.0/idl-wire"))
}

fn read_profiles(root: &Path) -> Vec<Profile> {
    let path = root.join("profiles.tsv");
    let contents = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    contents
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 3, "invalid profile row: {line}");
            Profile {
                fixture: fields[0].to_owned(),
                crate_path: fields[1].to_owned(),
                baseline_file: fields[2].to_owned(),
            }
        })
        .collect()
}

#[test]
fn published_wire_contracts_match_v0_1_0() {
    let root = baseline_root();
    let profiles = read_profiles(&root);
    assert!(!profiles.is_empty(), "IDL wire profile inventory is empty");

    for profile in profiles {
        let crate_path = workspace_root().join(&profile.crate_path);
        let first = quasar_cli::idl::build(&crate_path)
            .unwrap_or_else(|error| panic!("build {} IDL: {error}", profile.fixture));
        let second = quasar_cli::idl::build(&crate_path)
            .unwrap_or_else(|error| panic!("rebuild {} IDL: {error}", profile.fixture));

        let first_projection = canonical_abi_json_pretty(&first)
            .unwrap_or_else(|error| panic!("project {} ABI: {error}", profile.fixture));
        let second_projection = canonical_abi_json_pretty(&second)
            .unwrap_or_else(|error| panic!("reproject {} ABI: {error}", profile.fixture));
        assert_eq!(
            first_projection, second_projection,
            "{} ABI projection is not deterministic",
            profile.fixture
        );

        let stored_hash = first
            .hashes
            .as_ref()
            .unwrap_or_else(|| panic!("{} IDL has no hashes", profile.fixture));
        assert_eq!(
            stored_hash.abi,
            compute_abi_hash(&first),
            "{} stored ABI hash does not match its projection",
            profile.fixture
        );

        let actual = String::from_utf8(first_projection)
            .unwrap_or_else(|error| panic!("{} ABI JSON is not UTF-8: {error}", profile.fixture));
        expect_test::expect_file![root.join(&profile.baseline_file)].assert_eq(&actual);
    }
}
