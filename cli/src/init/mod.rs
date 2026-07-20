mod git;
mod scaffold;
mod templates;
mod types;

use {
    crate::{
        error::{CliError, CliResult},
        style,
    },
    git::maybe_initialize_git_repo,
    types::GitSetup,
};

pub fn run(cmd: crate::InitCommand) -> CliResult {
    let name = cmd.name.trim().to_string();
    if name.is_empty() {
        return Err(CliError::message("project name cannot be empty"));
    }
    let crate_name = crate_name_for_target(&name)?;
    scaffold::validate_target_dir(&name)?;

    let mut progress = style::Progress::new(cmd.verbose);
    progress.step(format!("Scaffolding {crate_name}..."));
    scaffold::scaffold(&name, &crate_name)?;
    progress.done("Scaffold files written");

    if !cmd.no_git {
        progress.step("Configuring git...");
        maybe_initialize_git_repo(&name, GitSetup::InitializeAndCommit);
        progress.done("Git setup complete");
    }
    progress.clear();

    println!();
    println!(
        "  {}  Created {} {}",
        style::color(83, "\u{2714}"),
        style::bold(&crate_name),
        style::dim("project")
    );
    println!();
    println!("  {}", style::dim("Next steps:"));
    if name != "." {
        println!(
            "    {}  {}",
            style::color(45, "\u{276f}"),
            style::bold(&format!("cd {name}"))
        );
    }
    for command in ["quasar build", "quasar test"] {
        println!(
            "    {}  {}",
            style::color(45, "\u{276f}"),
            style::bold(command)
        );
    }
    println!();
    Ok(())
}

fn crate_name_for_target(target: &str) -> Result<String, CliError> {
    let crate_name = if target == "." {
        std::env::current_dir()
            .ok()
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "my-program".to_string())
    } else {
        target.to_string()
    };
    validate_project_name(&crate_name)?;
    Ok(crate_name)
}

fn validate_project_name(name: &str) -> CliResult {
    let module = name.replace('-', "_");
    let valid = !module.is_empty()
        && module
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase())
        && module.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
        && name.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_'
                || character == '-'
        });
    if valid {
        Ok(())
    } else {
        Err(CliError::message(format!(
            "invalid project name: \"{name}\"\n  use a valid Rust crate name, e.g. my-program or \
             my_program"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_like_project_name() {
        let error = validate_project_name("programs/vault").unwrap_err();
        assert!(error.to_string().contains("invalid project name"));
    }

    #[test]
    fn rejects_leading_digit() {
        let error = validate_project_name("1-vault").unwrap_err();
        assert!(error.to_string().contains("invalid project name"));
    }
}
