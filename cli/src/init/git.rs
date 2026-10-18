use {
    super::types::GitSetup,
    std::{path::Path, process::Command},
};

pub(super) fn maybe_initialize_git_repo(name: &str, git_setup: GitSetup) {
    if matches!(git_setup, GitSetup::Skip) {
        return;
    }

    let root = Path::new(name);
    let already_git =
        if name == "." { Path::new(".git").exists() } else { root.join(".git").exists() };

    if !already_git {
        let _ = initialize_git_repo(root, git_setup, Path::new("git"));
    }
}

fn initialize_git_repo(root: &Path, git_setup: GitSetup, git: &Path) -> bool {
    run_git(git, root, &["init", "--quiet"])
        && match git_setup {
            GitSetup::InitializeAndCommit => {
                run_git(git, root, &["add", "."])
                    && run_git(git, root, &["commit", "-am", "chore: initial commit", "--quiet"])
            }
            GitSetup::Initialize | GitSetup::Skip => true,
        }
}

fn run_git(git: &Path, root: &Path, args: &[&str]) -> bool {
    Command::new(git)
        .args(args)
        .current_dir(root)
        .status()
        .ok()
        .is_some_and(|status| status.success())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};

    #[test]
    fn git_setup_preserves_order_and_stops_on_failure() {
        for (setup, fail, expected, succeeds) in [
            (
                GitSetup::InitializeAndCommit,
                "",
                vec!["init --quiet", "add .", "commit -am chore: initial commit --quiet"],
                true,
            ),
            (GitSetup::Initialize, "", vec!["init --quiet"], true),
            (GitSetup::InitializeAndCommit, "init", vec!["init --quiet"], false),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let git = temp.path().join("git");
            fs::write(
                &git,
                format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> git.log\n[ \"$1\" != '{fail}' ]\n"),
            )
            .unwrap();
            fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
            assert_eq!(initialize_git_repo(temp.path(), setup, &git), succeeds);
            assert_eq!(
                fs::read_to_string(temp.path().join("git.log"))
                    .unwrap()
                    .lines()
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }
}
