use {
    super::types::GitSetup,
    crate::error::CliError,
    std::{path::Path, process::Command},
};

pub(super) fn maybe_initialize_git_repo(name: &str, git_setup: GitSetup) -> Result<(), CliError> {
    let root = Path::new(name);
    let already_git = if name == "." {
        Path::new(".git").exists()
    } else {
        root.join(".git").exists()
    };

    if !already_git {
        initialize_git_repo(root, git_setup)?;
    }
    Ok(())
}

fn initialize_git_repo(root: &Path, git_setup: GitSetup) -> Result<(), CliError> {
    initialize_git_repo_with(git_setup, |args| run_git(Path::new("git"), root, args))
}

fn initialize_git_repo_with(
    git_setup: GitSetup,
    mut run: impl FnMut(&[&str]) -> Result<(), CliError>,
) -> Result<(), CliError> {
    run(&["init", "--quiet"])?;
    match git_setup {
        GitSetup::InitializeAndCommit => {
            run(&["add", "."])?;
            run(&["commit", "-am", "chore: initial commit", "--quiet"])
        }
        #[cfg(test)]
        GitSetup::Initialize => Ok(()),
    }
}

fn run_git(git: &Path, root: &Path, args: &[&str]) -> Result<(), CliError> {
    let output = Command::new(git)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| {
            CliError::message(format!("failed to run `git {}`: {error}", args.join(" ")))
        })?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    Err(CliError::message(format!(
        "`git {}` failed{}",
        args.join(" "),
        if detail.is_empty() {
            String::new()
        } else {
            format!(": {detail}")
        }
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_git_repo_runs_init_add_and_commit() {
        let mut calls = Vec::new();

        initialize_git_repo_with(GitSetup::InitializeAndCommit, |args| {
            calls.push(args.join(" "));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            calls,
            vec![
                "init --quiet",
                "add .",
                "commit -am chore: initial commit --quiet",
            ]
        );
    }

    #[test]
    fn initialize_git_repo_can_skip_initial_commit() {
        let mut calls = Vec::new();

        initialize_git_repo_with(GitSetup::Initialize, |args| {
            calls.push(args.join(" "));
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, vec!["init --quiet"]);
    }

    #[test]
    fn initialize_git_repo_stops_when_git_init_fails() {
        let mut calls = Vec::new();

        let error = initialize_git_repo_with(GitSetup::InitializeAndCommit, |args| {
            calls.push(args.join(" "));
            Err(CliError::message("`git init --quiet` failed"))
        })
        .unwrap_err();

        assert!(error.to_string().contains("`git init --quiet` failed"));
        assert_eq!(calls, vec!["init --quiet"]);
    }
}
