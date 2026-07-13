//! Versioned generated-client output baselines.

use {
    quasar_cli::idl,
    quasar_idl::types::canonical_json,
    std::{
        collections::{BTreeMap, BTreeSet},
        error::Error,
        fs,
        path::{Path, PathBuf},
    },
    tempfile::tempdir,
};

const CLIENT_LANGUAGES: &[&str] = &["typescript", "python", "golang", "c"];
const OUTPUT_ROOTS: &[&str] = &["c", "golang", "python", "rust", "typescript"];

struct Profile {
    fixture: String,
    crate_path: String,
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn baseline_root() -> PathBuf {
    std::env::var_os("QUASAR_GENERATED_CLIENT_BASELINE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            workspace_root().join("compatibility-baselines/v0.1.0/generated-clients")
        })
}

fn read_profiles(root: &Path) -> Vec<Profile> {
    let path = root.join("profiles.tsv");
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    contents
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 2, "invalid profile row: {line}");
            Profile {
                fixture: fields[0].to_owned(),
                crate_path: fields[1].to_owned(),
            }
        })
        .collect()
}

fn snapshot_tree(root: &Path) -> Result<BTreeMap<String, String>, Box<dyn Error>> {
    fn walk(
        root: &Path,
        path: &Path,
        files: &mut BTreeMap<String, String>,
    ) -> Result<(), Box<dyn Error>> {
        let mut entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                walk(root, &path, files)?;
                continue;
            }
            if !file_type.is_file() {
                return Err(
                    format!("generated output is not a regular file: {}", path.display()).into(),
                );
            }

            let relative = path
                .strip_prefix(root)?
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let contents = String::from_utf8(fs::read(&path)?)
                .map_err(|error| format!("generated output is not UTF-8 ({relative}): {error}"))?;
            if contents.contains('\r') {
                return Err(
                    format!("generated output contains a carriage return: {relative}").into(),
                );
            }
            files.insert(relative, contents);
        }
        Ok(())
    }

    let mut files = BTreeMap::new();
    if root.is_dir() {
        walk(root, root, &mut files)?;
    }
    Ok(files)
}

fn normalize_generated_text(contents: &str) -> String {
    let mut lines = contents
        .split('\n')
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect::<Vec<_>>();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.join("\n") + "\n"
}

fn normalize_tree(files: BTreeMap<String, String>) -> BTreeMap<String, String> {
    files
        .into_iter()
        .map(|(path, contents)| (path, normalize_generated_text(&contents)))
        .collect()
}

fn assert_complete_language_set(fixture: &str, files: &BTreeMap<String, String>) {
    let roots = files
        .keys()
        .map(|path| path.split('/').next().expect("non-empty output path"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        roots,
        OUTPUT_ROOTS.iter().copied().collect(),
        "{fixture} must emit every supported generated-client language"
    );
}

fn assert_same_tree(
    fixture: &str,
    first: &BTreeMap<String, String>,
    second: &BTreeMap<String, String>,
) {
    assert_eq!(
        first.keys().collect::<Vec<_>>(),
        second.keys().collect::<Vec<_>>(),
        "{fixture} generated-client file inventory is not deterministic"
    );
    for (path, contents) in first {
        assert_eq!(
            Some(contents),
            second.get(path),
            "{fixture} generated output is not deterministic: {path}"
        );
    }
}

#[test]
fn generated_clients_match_v0_1_0() -> Result<(), Box<dyn Error>> {
    let root = baseline_root();
    let profiles = read_profiles(&root);
    assert!(
        !profiles.is_empty(),
        "generated-client profile inventory is empty"
    );
    let blessing = std::env::var_os("UPDATE_EXPECT").is_some();

    for profile in profiles {
        let crate_path = workspace_root().join(&profile.crate_path);
        let first_temp = tempdir()?;
        let second_temp = tempdir()?;
        let first_clients = first_temp.path().join("clients");
        let second_clients = second_temp.path().join("clients");

        let first_idl = idl::generate(&crate_path, CLIENT_LANGUAGES, &first_clients)
            .map_err(|error| format!("generate {} clients: {error}", profile.fixture))?;
        let second_idl = idl::generate(&crate_path, CLIENT_LANGUAGES, &second_clients)
            .map_err(|error| format!("regenerate {} clients: {error}", profile.fixture))?;
        assert_eq!(
            canonical_json(&first_idl)?,
            canonical_json(&second_idl)?,
            "{} IDL generation is not deterministic",
            profile.fixture
        );

        let first_raw = snapshot_tree(&first_clients)?;
        let second_raw = snapshot_tree(&second_clients)?;
        assert_same_tree(&profile.fixture, &first_raw, &second_raw);
        let first = normalize_tree(first_raw);
        assert_complete_language_set(&profile.fixture, &first);

        let fixture_root = root.join("outputs").join(&profile.fixture);
        let recorded = snapshot_tree(&fixture_root)?;
        if !blessing {
            assert_eq!(
                recorded.keys().collect::<Vec<_>>(),
                first.keys().collect::<Vec<_>>(),
                "{} generated-client baseline file inventory changed",
                profile.fixture
            );
        }

        for (relative, contents) in &first {
            let baseline = fixture_root.join(relative);
            if blessing {
                fs::create_dir_all(baseline.parent().expect("baseline parent"))?;
            }
            expect_test::expect_file![baseline].assert_eq(contents);
        }

        let recorded = snapshot_tree(&fixture_root)?;
        assert_eq!(
            recorded.keys().collect::<Vec<_>>(),
            first.keys().collect::<Vec<_>>(),
            "{} generated-client baseline contains stale files",
            profile.fixture
        );
    }

    Ok(())
}

#[test]
fn generated_source_normalization_is_presentation_only() {
    assert_eq!(
        normalize_generated_text("first  \n\nsecond\t\n\n"),
        "first\n\nsecond\n"
    );
}
