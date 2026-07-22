use {
    crate::{backend::Backend, Pubkey, Test},
    std::{
        env,
        error::Error,
        fmt, fs,
        path::{Path, PathBuf},
    },
};

mod bundle;

use bundle::discover_program_bundle;

/// Environment variable set by `quasar test` to the freshly built program.
pub const PROGRAM_PATH_ENV: &str = "QUASAR_PROGRAM_PATH";

/// World setup: which program artifact to load and its runtime limits.
///
/// Created by [`Test::builder`].
pub struct TestBuilder {
    pub(super) program_id: Pubkey,
    pub(super) compute_unit_limit: Option<u64>,
    pub(super) program_path: Option<PathBuf>,
    pub(super) crate_name: Option<String>,
}

impl TestBuilder {
    pub(crate) fn new(program_id: Pubkey) -> Self {
        Self {
            program_id,
            compute_unit_limit: None,
            program_path: None,
            crate_name: None,
        }
    }

    /// Set the transaction compute-unit limit for this world.
    pub fn compute_unit_limit(mut self, limit: u64) -> Self {
        self.compute_unit_limit = Some(limit);
        self
    }

    /// Load an explicit program artifact instead of discovering one.
    pub fn program_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.program_path = Some(path.into());
        self
    }

    /// Prefer `target/deploy/{crate_name}.so` (with `-` mapped to `_`) during
    /// discovery, so tests resolve their own program in a workspace that
    /// builds several. `#[quasar_test]` passes `env!("CARGO_PKG_NAME")`.
    pub fn crate_name(mut self, name: impl Into<String>) -> Self {
        self.crate_name = Some(name.into());
        self
    }

    /// Load the program and start the world.
    pub fn build(self) -> Result<Test, SetupError> {
        let path = match self.program_path {
            Some(path) => path,
            None => resolve_program_path(self.crate_name.as_deref())?,
        };
        let elf = fs::read(&path).map_err(|source| SetupError::ReadProgram {
            path: path.clone(),
            source,
        })?;
        let mut backend = Backend::new();
        if let Some(limit) = self.compute_unit_limit {
            backend.set_compute_unit_limit(limit);
        }
        backend.load_program(&self.program_id, &elf);
        for program in discover_program_bundle(&path, self.program_id)? {
            let elf = fs::read(&program.path).map_err(|source| SetupError::ReadBundledProgram {
                path: program.path.clone(),
                source,
            })?;
            backend.load_program(&program.id, &elf);
        }
        Ok(Test {
            backend,
            program_id: self.program_id,
            program_path: path,
            fresh_addresses: 0,
        })
    }
}

/// Resolve the compiled program path: the `quasar test` override, then
/// discovery from the current directory.
fn resolve_program_path(crate_name: Option<&str>) -> Result<PathBuf, SetupError> {
    if let Some(path) = configured_program_path()? {
        return Ok(path);
    }
    let current_dir = env::current_dir().map_err(SetupError::CurrentDirectory)?;
    resolve_program_path_from_named(&current_dir, crate_name)
}

fn configured_program_path() -> Result<Option<PathBuf>, SetupError> {
    let Some(path) = env::var_os(PROGRAM_PATH_ENV) else {
        return Ok(None);
    };
    let path = PathBuf::from(path);
    if path.is_file() {
        return Ok(Some(path));
    }
    Err(SetupError::ConfiguredProgramMissing { path })
}

pub(super) fn resolve_program_path_from_named(
    start: &Path,
    crate_name: Option<&str>,
) -> Result<PathBuf, SetupError> {
    let artifact = crate_name.map(|name| format!("{}.so", name.replace('-', "_")));
    let mut checked = Vec::new();
    for ancestor in start.ancestors() {
        let deploy = ancestor.join("target/deploy");
        checked.push(deploy.clone());
        if let Some(ref artifact) = artifact {
            let path = deploy.join(artifact);
            if path.is_file() {
                return Ok(path);
            }
        }
        let mut programs = match fs::read_dir(&deploy) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "so"))
                .collect::<Vec<_>>(),
            Err(_) => continue,
        };
        programs.sort();
        if programs.len() == 1 {
            return Ok(programs.remove(0));
        }
        if programs.len() > 1 {
            return Err(SetupError::AmbiguousPrograms { deploy, programs });
        }
    }

    Err(SetupError::ProgramNotFound {
        start: start.to_path_buf(),
        checked,
    })
}

/// Failure to locate or load the current project's compiled program.
#[derive(Debug)]
#[non_exhaustive]
pub enum SetupError {
    /// The path supplied by `quasar test` no longer exists.
    ConfiguredProgramMissing {
        /// Missing path supplied through [`PROGRAM_PATH_ENV`].
        path: PathBuf,
    },
    /// The current working directory could not be read.
    CurrentDirectory(std::io::Error),
    /// No unambiguous program was found under an ancestor `target/deploy`.
    ProgramNotFound {
        /// Directory from which ancestor discovery began.
        start: PathBuf,
        /// Candidate deploy directories that were inspected.
        checked: Vec<PathBuf>,
    },
    /// More than one program artifact exists in the closest deploy directory.
    AmbiguousPrograms {
        /// Deploy directory containing multiple candidates.
        deploy: PathBuf,
        /// Candidate program artifacts in that directory.
        programs: Vec<PathBuf>,
    },
    /// The selected program artifact could not be read.
    ReadProgram {
        /// Program artifact that could not be read.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// The directory containing the primary program could not be inspected.
    ReadDeployDirectory {
        /// Deploy directory that could not be inspected.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// An automatically discovered CPI program could not be read.
    ReadBundledProgram {
        /// Bundled program artifact that could not be read.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// A deployed program's keypair file could not be read.
    ReadProgramKeypair {
        /// Program keypair that could not be read.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// A deployed program's keypair did not contain a valid 64-byte keypair.
    InvalidProgramKeypair {
        /// Invalid program keypair file.
        path: PathBuf,
        /// Why the keypair could not identify a program.
        reason: String,
    },
}

impl fmt::Display for SetupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfiguredProgramMissing { path } => write!(
                formatter,
                "{PROGRAM_PATH_ENV} points to missing program artifact {}; run `quasar test` \
                 without `--no-build`",
                path.display()
            ),
            Self::CurrentDirectory(source) => {
                write!(
                    formatter,
                    "could not resolve the current project directory: {source}"
                )
            }
            Self::ProgramNotFound { start, checked } => {
                write!(
                    formatter,
                    "could not find one compiled Quasar program from {}; run `quasar test` or set \
                     {PROGRAM_PATH_ENV}",
                    start.display()
                )?;
                if !checked.is_empty() {
                    write!(formatter, " (checked")?;
                    for path in checked {
                        write!(formatter, " {}", path.display())?;
                    }
                    write!(formatter, ")")?;
                }
                Ok(())
            }
            Self::AmbiguousPrograms { deploy, programs } => {
                write!(
                    formatter,
                    "found multiple program artifacts in {}; run `quasar test` or set \
                     {PROGRAM_PATH_ENV} to the intended artifact:",
                    deploy.display()
                )?;
                for path in programs {
                    write!(formatter, " {}", path.display())?;
                }
                Ok(())
            }
            Self::ReadProgram { path, source } => write!(
                formatter,
                "could not read program artifact {}: {source}",
                path.display()
            ),
            Self::ReadDeployDirectory { path, source } => write!(
                formatter,
                "could not inspect program bundle in {}: {source}",
                path.display()
            ),
            Self::ReadBundledProgram { path, source } => write!(
                formatter,
                "could not read bundled CPI program {}: {source}",
                path.display()
            ),
            Self::ReadProgramKeypair { path, source } => write!(
                formatter,
                "could not read program keypair {}: {source}",
                path.display()
            ),
            Self::InvalidProgramKeypair { path, reason } => write!(
                formatter,
                "invalid program keypair {}: {reason}",
                path.display()
            ),
        }
    }
}

impl Error for SetupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurrentDirectory(source)
            | Self::ReadProgram { source, .. }
            | Self::ReadDeployDirectory { source, .. }
            | Self::ReadBundledProgram { source, .. }
            | Self::ReadProgramKeypair { source, .. } => Some(source),
            Self::ConfiguredProgramMissing { .. }
            | Self::ProgramNotFound { .. }
            | Self::AmbiguousPrograms { .. }
            | Self::InvalidProgramKeypair { .. } => None,
        }
    }
}
