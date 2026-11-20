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
        let _ = initialize_git_repo(git_setup, |args| run_git(root, args));
    }
}

/// Runs each step through `git`, stopping at the first that fails.
fn initialize_git_repo(git_setup: GitSetup, mut git: impl FnMut(&[&str]) -> bool) -> bool {
    git(&["init", "--quiet"])
        && match git_setup {
            GitSetup::InitializeAndCommit => {
                git(&["add", "."]) && git(&["commit", "-am", "chore: initial commit", "--quiet"])
            }
            GitSetup::Initialize | GitSetup::Skip => true,
        }
}

fn run_git(root: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .ok()
        .is_some_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

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
            // The steps are recorded, not run: a script written here and run
            // at once fails with "text file busy" whenever another test's
            // child process still holds the script open.
            let mut ran = Vec::new();
            let succeeded = initialize_git_repo(setup, |args| {
                ran.push(args.join(" "));
                args[0] != fail
            });
            assert_eq!(succeeded, succeeds);
            assert_eq!(ran, expected);
        }
    }
}
