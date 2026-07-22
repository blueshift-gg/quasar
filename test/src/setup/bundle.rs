use {
    super::SetupError,
    crate::Pubkey,
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

pub(super) struct BundledProgram {
    pub(super) id: Pubkey,
    pub(super) path: PathBuf,
}

pub(super) fn discover_program_bundle(
    primary_program: &Path,
    primary_id: Pubkey,
) -> Result<Vec<BundledProgram>, SetupError> {
    let Some(deploy) = primary_program.parent() else {
        return Ok(Vec::new());
    };
    let entries = fs::read_dir(deploy).map_err(|source| SetupError::ReadDeployDirectory {
        path: deploy.to_path_buf(),
        source,
    })?;
    let mut programs = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|source| SetupError::ReadDeployDirectory {
                path: deploy.to_path_buf(),
                source,
            })?
            .path();
        if path != primary_program && path.extension().is_some_and(|extension| extension == "so") {
            programs.push(path);
        }
    }
    programs.sort();

    let mut bundle = Vec::new();
    for path in programs {
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let keypair = path.with_file_name(format!("{stem}-keypair.json"));
        if !keypair.is_file() {
            continue;
        }
        let id = read_program_id(&keypair)?;
        if id != primary_id {
            bundle.push(BundledProgram { id, path });
        }
    }
    Ok(bundle)
}

fn read_program_id(keypair: &Path) -> Result<Pubkey, SetupError> {
    let bytes = fs::read(keypair).map_err(|source| SetupError::ReadProgramKeypair {
        path: keypair.to_path_buf(),
        source,
    })?;
    let secret = serde_json::from_slice::<Vec<u8>>(&bytes).map_err(|source| {
        SetupError::InvalidProgramKeypair {
            path: keypair.to_path_buf(),
            reason: source.to_string(),
        }
    })?;
    let secret: [u8; 64] =
        secret
            .try_into()
            .map_err(|secret: Vec<u8>| SetupError::InvalidProgramKeypair {
                path: keypair.to_path_buf(),
                reason: format!("expected 64 bytes, found {}", secret.len()),
            })?;
    Ok(Pubkey::new_from_array(
        secret[32..].try_into().expect("32-byte public key"),
    ))
}

#[cfg(test)]
mod tests {
    use {super::*, tempfile::tempdir};

    fn write_program(deploy: &Path, name: &str, public_key: [u8; 32]) -> PathBuf {
        let program = deploy.join(format!("{name}.so"));
        fs::write(&program, b"elf").unwrap();
        let mut keypair = vec![0_u8; 32];
        keypair.extend_from_slice(&public_key);
        fs::write(
            deploy.join(format!("{name}-keypair.json")),
            serde_json::to_vec(&keypair).unwrap(),
        )
        .unwrap();
        program
    }

    #[test]
    fn discovers_keyed_siblings_in_stable_order() {
        let root = tempdir().unwrap();
        let deploy = root.path().join("target/deploy");
        fs::create_dir_all(&deploy).unwrap();
        let primary = write_program(&deploy, "primary", [1; 32]);
        write_program(&deploy, "z_program", [3; 32]);
        write_program(&deploy, "a_program", [2; 32]);
        fs::write(deploy.join("not-a-program.so"), b"ignored without keypair").unwrap();

        let programs = discover_program_bundle(&primary, Pubkey::new_from_array([1; 32])).unwrap();
        assert_eq!(
            programs
                .iter()
                .map(|program| program.id)
                .collect::<Vec<_>>(),
            [
                Pubkey::new_from_array([2; 32]),
                Pubkey::new_from_array([3; 32])
            ]
        );
    }

    #[test]
    fn rejects_nonstandard_keypair_lengths() {
        let root = tempdir().unwrap();
        let deploy = root.path().join("target/deploy");
        fs::create_dir_all(&deploy).unwrap();
        let primary = write_program(&deploy, "primary", [1; 32]);
        fs::write(deploy.join("broken.so"), b"elf").unwrap();
        fs::write(deploy.join("broken-keypair.json"), b"[1, 2]").unwrap();

        assert!(matches!(
            discover_program_bundle(&primary, Pubkey::new_from_array([1; 32])),
            Err(SetupError::InvalidProgramKeypair { .. })
        ));

        let overlong = vec![0; 65];
        fs::write(
            deploy.join("broken-keypair.json"),
            serde_json::to_vec(&overlong).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            discover_program_bundle(&primary, Pubkey::new_from_array([1; 32])),
            Err(SetupError::InvalidProgramKeypair { .. })
        ));
    }
}
