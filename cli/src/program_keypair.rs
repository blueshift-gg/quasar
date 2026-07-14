use {
    crate::error::CliError,
    ed25519_dalek::SigningKey,
    std::{
        fs::{self, File},
        io::{ErrorKind, Write},
        path::Path,
    },
    tempfile::{Builder, NamedTempFile},
};

const KEYPAIR_LEN: usize = 64;
const SECRET_KEY_LEN: usize = 32;

/// A validated Solana keypair encoding: 32 secret bytes followed by the
/// corresponding 32-byte public key.
pub(crate) struct ProgramKeypair {
    bytes: [u8; KEYPAIR_LEN],
}

impl ProgramKeypair {
    pub(crate) fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut rand::thread_rng());
        let mut bytes = [0; KEYPAIR_LEN];
        bytes[..SECRET_KEY_LEN].copy_from_slice(signing_key.as_bytes());
        bytes[SECRET_KEY_LEN..].copy_from_slice(signing_key.verifying_key().as_bytes());
        Self { bytes }
    }

    pub(crate) fn read(path: &Path) -> Result<Self, CliError> {
        ensure_regular_keypair(path, "read")?;
        let json = fs::read(path).map_err(|error| CliError::io_path("read", path, error))?;
        let bytes: Vec<u8> = serde_json::from_slice(&json).map_err(|error| {
            CliError::json_parse(format!("keypair file {}", path.display()), error)
        })?;
        Self::from_bytes(path, bytes)
    }

    fn from_bytes(path: &Path, bytes: Vec<u8>) -> Result<Self, CliError> {
        let bytes: [u8; KEYPAIR_LEN] = bytes.try_into().map_err(|bytes: Vec<u8>| {
            CliError::message(format!(
                "invalid keypair file {}: expected {KEYPAIR_LEN} bytes, found {}",
                path.display(),
                bytes.len()
            ))
        })?;

        let secret: [u8; SECRET_KEY_LEN] = bytes[..SECRET_KEY_LEN]
            .try_into()
            .expect("fixed-size keypair prefix");
        let expected_public = SigningKey::from_bytes(&secret).verifying_key();
        if bytes[SECRET_KEY_LEN..] != expected_public.as_bytes()[..] {
            return Err(CliError::message(format!(
                "invalid keypair file {}: public key does not match secret key",
                path.display()
            )));
        }

        Ok(Self { bytes })
    }

    pub(crate) fn program_id(&self) -> String {
        bs58::encode(&self.bytes[SECRET_KEY_LEN..]).into_string()
    }

    fn json(&self) -> Result<Vec<u8>, CliError> {
        serde_json::to_vec(&self.bytes[..])
            .map_err(|error| CliError::json_serialize("program keypair JSON", error))
    }
}

/// A regular file update committed with a keypair replacement. The expected
/// contents prevent a concurrent edit from being silently overwritten.
pub(crate) struct CompanionUpdate<'a> {
    pub(crate) path: &'a Path,
    pub(crate) expected: &'a [u8],
    pub(crate) replacement: &'a [u8],
}

/// Write a keypair through an owner-only temporary file in the destination
/// directory. When a companion update is supplied, it is staged first and
/// rolled back if the final keypair publication fails.
pub(crate) fn write(
    path: &Path,
    keypair: &ProgramKeypair,
    overwrite: bool,
    companion: Option<CompanionUpdate<'_>>,
) -> Result<(), CliError> {
    write_with_hook(path, keypair, overwrite, companion, || Ok(()))
}

/// Atomically replace a regular non-secret file after verifying that it still
/// contains the bytes the caller inspected.
pub(crate) fn replace_regular_file(
    path: &Path,
    expected: &[u8],
    replacement: &[u8],
) -> Result<(), CliError> {
    ensure_expected_regular_file(path, expected)?;
    let mut staged = StagedFile::create(path, replacement, FileMode::Preserve(path))?;
    ensure_expected_regular_file(path, expected)?;
    staged.publish_to(path, true)
}

fn write_with_hook<F>(
    path: &Path,
    keypair: &ProgramKeypair,
    overwrite: bool,
    companion: Option<CompanionUpdate<'_>>,
    before_keypair_commit: F,
) -> Result<(), CliError>
where
    F: FnOnce() -> Result<(), CliError>,
{
    validate_write_target(path, overwrite)?;
    let json = keypair.json()?;
    let mut staged_keypair = StagedFile::create(path, &json, FileMode::OwnerOnly)?;

    let mut staged_companion = match companion.as_ref() {
        Some(update) => {
            ensure_expected_regular_file(update.path, update.expected)?;
            Some(StagedFile::create(
                update.path,
                update.replacement,
                FileMode::Preserve(update.path),
            )?)
        }
        None => None,
    };

    let mut companion_backup = match companion.as_ref() {
        Some(update) => {
            ensure_expected_regular_file(update.path, update.expected)?;
            Some(StagedFile::create(
                update.path,
                update.expected,
                FileMode::Preserve(update.path),
            )?)
        }
        None => None,
    };

    let mut companion_committed = false;

    let commit_result = (|| {
        if let (Some(update), Some(staged)) = (companion.as_ref(), staged_companion.as_mut()) {
            staged.publish_to(update.path, true)?;
            companion_committed = true;
        }

        before_keypair_commit()?;
        validate_write_target(path, overwrite)?;
        staged_keypair.publish_to(path, overwrite)
    })();

    if let Err(error) = commit_result {
        if companion_committed {
            let update = companion
                .as_ref()
                .expect("committed companion has an update");
            let backup = companion_backup
                .as_mut()
                .expect("committed companion has a backup");
            backup.restore_to(update.path)?;
        }
        return Err(error);
    }

    Ok(())
}

fn ensure_regular_keypair(path: &Path, action: &str) -> Result<fs::Metadata, CliError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| CliError::io_path("inspect", path, error))?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Err(CliError::message(format!(
            "refusing to {action} keypair through symlink: {}",
            path.display()
        )));
    }
    if !file_type.is_file() {
        return Err(CliError::message(format!(
            "refusing to {action} non-regular keypair file: {}",
            path.display()
        )));
    }
    Ok(metadata)
}

pub(crate) fn validate_write_target(path: &Path, overwrite: bool) -> Result<(), CliError> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            ensure_regular_keypair(path, "overwrite")?;
            if !overwrite {
                return Err(CliError::message(format!(
                    "keypair already exists: {}\n\n  Use quasar keys new --force to overwrite \
                     it.\n  Warning: this will change your program address.",
                    path.display()
                )));
            }
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CliError::io_path("inspect", path, error)),
    }
}

fn ensure_expected_regular_file(path: &Path, expected: &[u8]) -> Result<(), CliError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| CliError::io_path("inspect", path, error))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(CliError::message(format!(
            "refusing to replace non-regular file: {}",
            path.display()
        )));
    }
    let current = fs::read(path).map_err(|error| CliError::io_path("read", path, error))?;
    if current != expected {
        return Err(CliError::message(format!(
            "refusing to replace concurrently modified file: {}",
            path.display()
        )));
    }
    Ok(())
}

enum FileMode<'a> {
    OwnerOnly,
    Preserve(&'a Path),
}

struct StagedFile {
    file: Option<NamedTempFile>,
}

impl StagedFile {
    fn create(destination: &Path, contents: &[u8], mode: FileMode<'_>) -> Result<Self, CliError> {
        let parent = destination.parent().ok_or_else(|| {
            CliError::message(format!(
                "keypair destination has no parent: {}",
                destination.display()
            ))
        })?;
        fs::create_dir_all(parent)
            .map_err(|error| CliError::io_path("create directory", parent, error))?;

        let name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("keypair");
        let prefix = format!(".{name}.quasar-tmp-");
        let mut builder = Builder::new();
        builder.prefix(&prefix);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            builder.permissions(fs::Permissions::from_mode(0o600));
        }

        let mut file = builder
            .tempfile_in(parent)
            .map_err(|error| CliError::io_path("create temporary file", parent, error))?;
        set_staged_permissions(file.as_file(), file.path(), &mode)?;
        file.write_all(contents)
            .and_then(|()| file.flush())
            .and_then(|()| file.as_file().sync_all())
            .map_err(|error| CliError::io_path("write temporary file", file.path(), error))?;
        Ok(Self { file: Some(file) })
    }

    fn publish_to(&mut self, destination: &Path, overwrite: bool) -> Result<(), CliError> {
        let file = self.file.take().expect("staged file is published once");
        let result = if overwrite {
            file.persist(destination)
        } else {
            file.persist_noclobber(destination)
        };
        result
            .map(|_| ())
            .map_err(|error| CliError::io_path("atomically publish", destination, error.error))
    }

    fn restore_to(&mut self, destination: &Path) -> Result<(), CliError> {
        let mut file = self.file.take().expect("staged backup is restored once");
        let retained_path = file.path().to_path_buf();
        file.disable_cleanup(true);
        match file.persist(destination) {
            Ok(_) => Ok(()),
            Err(error) => Err(CliError::message(format!(
                "failed to restore {}: {}; original retained at {}",
                destination.display(),
                error.error,
                retained_path.display()
            ))),
        }
    }
}

fn set_staged_permissions(file: &File, path: &Path, mode: &FileMode<'_>) -> Result<(), CliError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let unix_mode = match mode {
            FileMode::OwnerOnly => 0o600,
            FileMode::Preserve(source) => {
                fs::metadata(source)
                    .map_err(|error| CliError::io_path("inspect permissions", source, error))?
                    .permissions()
                    .mode()
                    & 0o777
            }
        };
        file.set_permissions(fs::Permissions::from_mode(unix_mode))
            .map_err(|error| CliError::io_path("secure temporary file", path, error))?;
    }

    #[cfg(not(unix))]
    let _ = (file, path, mode);

    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, tempfile::tempdir};

    fn keypair_bytes(path: &Path) -> Vec<u8> {
        fs::read(path).expect("read keypair")
    }

    #[test]
    fn generated_keypair_is_owner_only_and_round_trips() {
        let root = tempdir().unwrap();
        let path = root.path().join("program-keypair.json");
        let keypair = ProgramKeypair::generate();
        let expected_id = keypair.program_id();

        write(&path, &keypair, false, None).unwrap();

        assert_eq!(
            ProgramKeypair::read(&path).unwrap().program_id(),
            expected_id
        );
        let bytes: Vec<u8> = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(bytes.len(), KEYPAIR_LEN);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn rejects_secret_public_mismatch() {
        let root = tempdir().unwrap();
        let path = root.path().join("program-keypair.json");
        let mut bytes = ProgramKeypair::generate().bytes;
        bytes[KEYPAIR_LEN - 1] ^= 1;
        fs::write(&path, serde_json::to_vec(&bytes[..]).unwrap()).unwrap();

        let error = match ProgramKeypair::read(&path) {
            Ok(_) => panic!("mismatch must fail"),
            Err(error) => error,
        };
        assert!(error
            .to_string()
            .contains("public key does not match secret key"));
    }

    #[test]
    fn rejects_wrong_keypair_length() {
        let root = tempdir().unwrap();
        let path = root.path().join("program-keypair.json");
        fs::write(&path, serde_json::to_vec(&vec![1u8; 65]).unwrap()).unwrap();

        let error = match ProgramKeypair::read(&path) {
            Ok(_) => panic!("extra byte must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("expected 64 bytes, found 65"));
    }

    #[test]
    fn force_replaces_a_regular_keypair_but_default_does_not() {
        let root = tempdir().unwrap();
        let path = root.path().join("program-keypair.json");
        let old = ProgramKeypair::generate();
        let new = ProgramKeypair::generate();
        write(&path, &old, false, None).unwrap();
        let old_contents = keypair_bytes(&path);

        let error = write(&path, &new, false, None).expect_err("overwrite requires force");
        assert!(error.to_string().contains("--force"));
        assert_eq!(keypair_bytes(&path), old_contents);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        }
        write(&path, &new, true, None).unwrap();
        assert_eq!(
            ProgramKeypair::read(&path).unwrap().program_id(),
            new.program_id()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_read_and_overwrite_targets() {
        use std::os::unix::fs::symlink;

        let root = tempdir().unwrap();
        let real = root.path().join("real.json");
        let link = root.path().join("link.json");
        let old = ProgramKeypair::generate();
        write(&real, &old, false, None).unwrap();
        let old_contents = keypair_bytes(&real);
        symlink(&real, &link).unwrap();

        let read_error = match ProgramKeypair::read(&link) {
            Ok(_) => panic!("symlink read must fail"),
            Err(error) => error,
        };
        assert!(read_error.to_string().contains("through symlink"));
        let write_error = write(&link, &ProgramKeypair::generate(), true, None)
            .expect_err("symlink overwrite must fail");
        assert!(write_error.to_string().contains("through symlink"));
        assert_eq!(keypair_bytes(&real), old_contents);
    }

    #[test]
    fn rejects_non_regular_overwrite_target() {
        let root = tempdir().unwrap();
        let path = root.path().join("program-keypair.json");
        fs::create_dir(&path).unwrap();

        let error = write(&path, &ProgramKeypair::generate(), true, None)
            .expect_err("directory overwrite must fail");
        assert!(error.to_string().contains("non-regular keypair"));
        assert!(path.is_dir());
    }

    #[test]
    fn dropping_staged_write_preserves_existing_keypair() {
        let root = tempdir().unwrap();
        let path = root.path().join("program-keypair.json");
        let old = ProgramKeypair::generate();
        write(&path, &old, false, None).unwrap();
        let old_contents = keypair_bytes(&path);
        let staged_json = ProgramKeypair::generate().json().unwrap();

        let staged = StagedFile::create(&path, &staged_json, FileMode::OwnerOnly).unwrap();
        drop(staged);

        assert_eq!(keypair_bytes(&path), old_contents);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn later_failure_restores_keypair_and_companion() {
        let root = tempdir().unwrap();
        let keypair_path = root.path().join("program-keypair.json");
        let source_path = root.path().join("lib.rs");
        let old = ProgramKeypair::generate();
        let new = ProgramKeypair::generate();
        write(&keypair_path, &old, false, None).unwrap();
        let old_keypair = keypair_bytes(&keypair_path);
        let old_source = format!("declare_id!(\"{}\");\n", old.program_id());
        let new_source = format!("declare_id!(\"{}\");\n", new.program_id());
        fs::write(&source_path, &old_source).unwrap();

        let error = write_with_hook(
            &keypair_path,
            &new,
            true,
            Some(CompanionUpdate {
                path: &source_path,
                expected: old_source.as_bytes(),
                replacement: new_source.as_bytes(),
            }),
            || Err(CliError::message("injected interruption")),
        )
        .expect_err("injected failure must fail");

        assert_eq!(error.to_string(), "injected interruption");
        assert_eq!(keypair_bytes(&keypair_path), old_keypair);
        assert_eq!(fs::read_to_string(&source_path).unwrap(), old_source);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
    }

    #[test]
    fn rollback_failure_retains_the_original_companion() {
        let root = tempdir().unwrap();
        let keypair_path = root.path().join("program-keypair.json");
        let source_path = root.path().join("lib.rs");
        let old = ProgramKeypair::generate();
        let new = ProgramKeypair::generate();
        write(&keypair_path, &old, false, None).unwrap();
        let old_keypair = keypair_bytes(&keypair_path);
        let old_source = format!("declare_id!(\"{}\");\n", old.program_id());
        let new_source = format!("declare_id!(\"{}\");\n", new.program_id());
        fs::write(&source_path, &old_source).unwrap();

        let error = write_with_hook(
            &keypair_path,
            &new,
            true,
            Some(CompanionUpdate {
                path: &source_path,
                expected: old_source.as_bytes(),
                replacement: new_source.as_bytes(),
            }),
            || {
                fs::remove_file(&source_path).unwrap();
                fs::create_dir(&source_path).unwrap();
                Err(CliError::message("injected interruption"))
            },
        )
        .expect_err("rollback obstruction must fail");

        let message = error.to_string();
        let retained = Path::new(
            message
                .split_once("original retained at ")
                .expect("error includes retained backup path")
                .1,
        );
        assert_eq!(keypair_bytes(&keypair_path), old_keypair);
        assert_eq!(fs::read_to_string(retained).unwrap(), old_source);
    }
}
