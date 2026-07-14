use {
    crate::error::CliError,
    rand::random,
    std::{
        collections::HashSet,
        fs,
        path::{Component, Path, PathBuf},
    },
};

pub(crate) enum PreparedContents {
    File(Vec<u8>),
    Directory(Vec<(PathBuf, Vec<u8>)>),
}

/// An output fully rendered in memory. `commit` stages every entry beside its
/// destination before it replaces any existing output.
pub(crate) struct PreparedOutput {
    destination: PathBuf,
    contents: PreparedContents,
}

impl PreparedOutput {
    pub(crate) fn file(path: impl Into<PathBuf>, contents: impl Into<Vec<u8>>) -> Self {
        Self {
            destination: path.into(),
            contents: PreparedContents::File(contents.into()),
        }
    }

    pub(crate) fn directory(path: impl Into<PathBuf>, files: Vec<(PathBuf, Vec<u8>)>) -> Self {
        Self {
            destination: path.into(),
            contents: PreparedContents::Directory(files),
        }
    }
}

struct StagedOutput {
    destination: PathBuf,
    staged: PathBuf,
    backup: Option<PathBuf>,
}

pub(crate) fn commit(outputs: Vec<PreparedOutput>) -> Result<(), CliError> {
    let mut destinations = HashSet::new();
    for output in &outputs {
        if !destinations.insert(output.destination.clone()) {
            return Err(CliError::message(format!(
                "duplicate generated output destination: {}",
                output.destination.display()
            )));
        }
    }

    let nonce = random::<u64>();
    let mut staged = Vec::with_capacity(outputs.len());
    for (index, output) in outputs.into_iter().enumerate() {
        match stage(output, nonce, index) {
            Ok(entry) => staged.push(entry),
            Err(error) => {
                for entry in &staged {
                    let _ = remove_path(&entry.staged);
                }
                return Err(error);
            }
        }
    }

    for index in 0..staged.len() {
        let destination = staged[index].destination.clone();
        if destination.exists() || fs::symlink_metadata(&destination).is_ok() {
            let backup = sibling_path(&destination, "backup", nonce, index)?;
            if let Err(error) = fs::rename(&destination, &backup) {
                rollback(&staged[..index]);
                for entry in &staged[index..] {
                    let _ = remove_path(&entry.staged);
                }
                return Err(CliError::io_path(
                    "back up generated output",
                    &destination,
                    error,
                ));
            }
            staged[index].backup = Some(backup);
        }

        if let Err(error) = fs::rename(&staged[index].staged, &destination) {
            if let Some(backup) = &staged[index].backup {
                let _ = fs::rename(backup, &destination);
            }
            let _ = remove_path(&staged[index].staged);
            rollback(&staged[..index]);
            for entry in &staged[index + 1..] {
                let _ = remove_path(&entry.staged);
            }
            return Err(CliError::io_path(
                "commit generated output",
                &destination,
                error,
            ));
        }
    }

    for entry in &staged {
        if let Some(backup) = &entry.backup {
            remove_path(backup)
                .map_err(|e| CliError::io_path("remove generated-output backup", backup, e))?;
        }
    }
    Ok(())
}

fn stage(output: PreparedOutput, nonce: u64, index: usize) -> Result<StagedOutput, CliError> {
    let parent = output.destination.parent().ok_or_else(|| {
        CliError::message(format!(
            "generated output has no parent: {}",
            output.destination.display()
        ))
    })?;
    fs::create_dir_all(parent)
        .map_err(|e| CliError::io_path("create generated-output parent", parent, e))?;
    let staged = sibling_path(&output.destination, "stage", nonce, index)?;

    let stage_result = (|| -> Result<(), CliError> {
        match output.contents {
            PreparedContents::File(contents) => fs::write(&staged, contents)
                .map_err(|e| CliError::io_path("stage generated file", &staged, e)),
            PreparedContents::Directory(files) => {
                fs::create_dir(&staged)
                    .map_err(|e| CliError::io_path("stage generated directory", &staged, e))?;
                for (relative, contents) in files {
                    validate_relative_path(&relative)?;
                    let path = staged.join(relative);
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent).map_err(|e| {
                            CliError::io_path("create staged generated directory", parent, e)
                        })?;
                    }
                    fs::write(&path, contents)
                        .map_err(|e| CliError::io_path("stage generated file", &path, e))?;
                }
                Ok(())
            }
        }
    })();
    if let Err(error) = stage_result {
        let _ = remove_path(&staged);
        return Err(error);
    }

    Ok(StagedOutput {
        destination: output.destination,
        staged,
        backup: None,
    })
}

fn sibling_path(
    destination: &Path,
    kind: &str,
    nonce: u64,
    index: usize,
) -> Result<PathBuf, CliError> {
    let parent = destination.parent().ok_or_else(|| {
        CliError::message(format!(
            "generated output has no parent: {}",
            destination.display()
        ))
    })?;
    Ok(parent.join(format!(
        ".quasar-{kind}-{}-{nonce:016x}-{index}",
        std::process::id()
    )))
}

fn validate_relative_path(path: &Path) -> Result<(), CliError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(CliError::message(format!(
            "generated file path must be relative and traversal-free: {}",
            path.display()
        )));
    }
    Ok(())
}

fn rollback(entries: &[StagedOutput]) {
    for entry in entries.iter().rev() {
        let _ = remove_path(&entry.destination);
        if let Some(backup) = &entry.backup {
            let _ = fs::rename(backup, &entry.destination);
        }
    }
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {
            fs::remove_dir_all(path)
        }
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use {super::*, tempfile::tempdir};

    #[test]
    fn commit_replaces_files_and_directories_after_staging() {
        let root = tempdir().unwrap();
        let file = root.path().join("client.json");
        let directory = root.path().join("src");
        fs::write(&file, "old").unwrap();
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("stale.rs"), "stale").unwrap();

        commit(vec![
            PreparedOutput::file(&file, b"new".to_vec()),
            PreparedOutput::directory(
                &directory,
                vec![(PathBuf::from("nested/lib.rs"), b"generated".to_vec())],
            ),
        ])
        .unwrap();

        assert_eq!(fs::read_to_string(file).unwrap(), "new");
        assert_eq!(
            fs::read_to_string(directory.join("nested/lib.rs")).unwrap(),
            "generated"
        );
        assert!(!directory.join("stale.rs").exists());
    }

    #[test]
    fn unsafe_relative_file_does_not_replace_existing_output() {
        let root = tempdir().unwrap();
        let directory = root.path().join("src");
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("lib.rs"), "old").unwrap();

        let error = commit(vec![PreparedOutput::directory(
            &directory,
            vec![(PathBuf::from("../escape.rs"), b"bad".to_vec())],
        )])
        .unwrap_err();

        assert!(error.to_string().contains("traversal-free"));
        assert_eq!(fs::read_to_string(directory.join("lib.rs")).unwrap(), "old");
        assert!(!root.path().join("escape.rs").exists());
    }
}
