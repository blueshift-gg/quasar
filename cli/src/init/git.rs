use {
    super::types::GitSetup,
    std::{path::Path, process::Command},
};

pub(super) fn maybe_initialize_git_repo(name: &str, git_setup: GitSetup) {
    let root = Path::new(name);
    let already_git = if name == "." {
        Path::new(".git").exists()
    } else {
        root.join(".git").exists()
    };

    if !already_git {
        let _ = initialize_git_repo(root, git_setup);
    }
}

fn initialize_git_repo(root: &Path, git_setup: GitSetup) -> bool {
    initialize_git_repo_with(root, git_setup, Path::new("git"))
}

fn initialize_git_repo_with(root: &Path, git_setup: GitSetup, git: &Path) -> bool {
    run_git(git, root, &["init", "--quiet"])
        && match git_setup {
            GitSetup::InitializeAndCommit => {
                run_git(git, root, &["add", "."])
                    && run_git(
                        git,
                        root,
                        &["commit", "-am", "chore: initial commit", "--quiet"],
                    )
            }
            #[cfg(test)]
            GitSetup::Initialize => true,
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{
            fs,
            path::PathBuf,
            time::{SystemTime, UNIX_EPOCH},
        },
    };

    #[test]
    fn initialize_git_repo_runs_init_add_and_commit() {
        let sandbox = create_test_sandbox("success");
        let git = write_fake_git(&sandbox, None);
        let root = sandbox.join("repo");
        fs::create_dir_all(&root).unwrap();

        let ok = initialize_git_repo_with(&root, GitSetup::InitializeAndCommit, &git);

        assert!(ok);
        assert_eq!(
            read_git_log(&sandbox),
            vec![
                "init --quiet",
                "add .",
                "commit -am chore: initial commit --quiet",
            ]
        );
    }

    #[test]
    fn initialize_git_repo_can_skip_initial_commit() {
        let sandbox = create_test_sandbox("init-only");
        let git = write_fake_git(&sandbox, None);
        let root = sandbox.join("repo");
        fs::create_dir_all(&root).unwrap();

        let ok = initialize_git_repo_with(&root, GitSetup::Initialize, &git);

        assert!(ok);
        assert_eq!(read_git_log(&sandbox), vec!["init --quiet"]);
    }

    #[test]
    fn initialize_git_repo_stops_when_git_init_fails() {
        let sandbox = create_test_sandbox("fail-init");
        let git = write_fake_git(&sandbox, Some("init"));
        let root = sandbox.join("repo");
        fs::create_dir_all(&root).unwrap();

        let ok = initialize_git_repo_with(&root, GitSetup::InitializeAndCommit, &git);

        assert!(!ok);
        assert_eq!(read_git_log(&sandbox), vec!["init --quiet"]);
    }

    fn create_test_sandbox(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "quasar-init-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn read_git_log(sandbox: &Path) -> Vec<String> {
        fs::read_to_string(sandbox.join("git.log"))
            .unwrap_or_default()
            .lines()
            .map(|line| line.to_string())
            .collect()
    }

    fn write_fake_git(sandbox: &Path, fail_on: Option<&str>) -> PathBuf {
        let path = sandbox.join("git");
        let log = sandbox.join("git.log");
        let fail = fail_on.unwrap_or("");
        fs::write(
            &path,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nif [ \"$1\" = '{}' ]; then\n  exit \
                 1\nfi\nexit 0\n",
                log.display(),
                fail
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).unwrap();
        }
        path
    }
}
