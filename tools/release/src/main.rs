use {
    clap::{Parser, Subcommand},
    serde::{Deserialize, Serialize},
    sha2::{Digest, Sha256},
    std::{
        collections::{BTreeMap, BTreeSet},
        env, fs,
        path::{Path, PathBuf},
        process::{Command, Output},
        thread,
        time::Duration,
    },
};

#[derive(Parser)]
#[command(about = "Cargo-metadata-derived Quasar release tooling")]
struct Cli {
    #[command(subcommand)]
    command: ReleaseCommand,
}

#[derive(Subcommand)]
enum ReleaseCommand {
    /// Print the complete publishable package graph.
    Graph {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Package every publishable workspace crate exactly once.
    Package {
        /// Directory that receives archives and manifest.json.
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify and unpack packaged crates for source-free rehearsal.
    Prepare {
        /// Directory containing archives and manifest.json.
        #[arg(long)]
        input: PathBuf,
        /// Empty directory that receives archives, sources, and Cargo patches.
        #[arg(long)]
        output: PathBuf,
    },
    /// Publish the packaged graph tier by tier.
    Publish {
        /// Lockstep workspace version and expected vVERSION tag.
        #[arg(long)]
        version: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
    workspace_root: PathBuf,
    target_directory: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    version: String,
    manifest_path: PathBuf,
    publish: Option<Vec<String>>,
    dependencies: Vec<CargoDependency>,
}

#[derive(Clone, Debug, Deserialize)]
struct CargoDependency {
    name: String,
    req: String,
    path: Option<PathBuf>,
    kind: Option<String>,
}

#[derive(Clone, Debug)]
struct Candidate {
    name: String,
    version: String,
    manifest_path: PathBuf,
    publishable: bool,
    dependencies: Vec<CandidateDependency>,
}

#[derive(Clone, Debug)]
struct CandidateDependency {
    name: String,
    requirement: String,
    is_dev: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ReleaseGraph {
    version: String,
    packages: Vec<ReleasePackage>,
    tiers: Vec<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ReleasePackage {
    name: String,
    version: String,
    manifest_path: PathBuf,
    dependencies: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PackageManifest {
    version: String,
    packages: Vec<PackagedCrate>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PackagedCrate {
    name: String,
    version: String,
    tier: usize,
    archive: String,
    sha256: String,
    dependencies: Vec<String>,
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("release error: {error}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let metadata = cargo_metadata()?;
    let candidates = candidates_from_metadata(&metadata)?;
    let graph = build_graph(&candidates)?;

    match cli.command {
        ReleaseCommand::Graph { json } => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&graph)
                        .map_err(|error| format!("serialize release graph: {error}"))?
                );
            } else {
                println!("Quasar {}", graph.version);
                for (index, tier) in graph.tiers.iter().enumerate() {
                    println!("tier {index}: {}", tier.join(", "));
                }
            }
            Ok(())
        }
        ReleaseCommand::Package { output } => package(&metadata, &graph, &output),
        ReleaseCommand::Prepare { input, output } => prepare_rehearsal(&graph, &input, &output),
        ReleaseCommand::Publish { version } => publish(&metadata, &graph, &version),
    }
}

fn cargo_metadata() -> Result<CargoMetadata, String> {
    let output = command_output(
        Command::new("cargo").args(["metadata", "--locked", "--format-version", "1", "--no-deps"]),
        "read Cargo metadata",
    )?;
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("parse Cargo metadata JSON: {error}"))
}

fn candidates_from_metadata(metadata: &CargoMetadata) -> Result<Vec<Candidate>, String> {
    let members = metadata
        .workspace_members
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let workspace_packages = metadata
        .packages
        .iter()
        .filter(|package| members.contains(package.id.as_str()))
        .collect::<Vec<_>>();
    let names = workspace_packages
        .iter()
        .map(|package| package.name.as_str())
        .collect::<BTreeSet<_>>();

    workspace_packages
        .into_iter()
        .map(|package| {
            let manifest_path = package
                .manifest_path
                .strip_prefix(&metadata.workspace_root)
                .map(Path::to_path_buf)
                .unwrap_or_else(|_| package.manifest_path.clone());
            let dependencies = package
                .dependencies
                .iter()
                .filter(|dependency| {
                    dependency.path.is_some() && names.contains(dependency.name.as_str())
                })
                .map(|dependency| CandidateDependency {
                    name: dependency.name.clone(),
                    requirement: dependency.req.clone(),
                    is_dev: dependency.kind.as_deref() == Some("dev"),
                })
                .collect();
            Ok(Candidate {
                name: package.name.clone(),
                version: package.version.clone(),
                manifest_path,
                publishable: package
                    .publish
                    .as_ref()
                    .is_none_or(|registries| !registries.is_empty()),
                dependencies,
            })
        })
        .collect()
}

fn build_graph(candidates: &[Candidate]) -> Result<ReleaseGraph, String> {
    let publishable = candidates
        .iter()
        .filter(|package| package.publishable)
        .map(|package| (package.name.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    if publishable.is_empty() {
        return Err("workspace has no publishable packages".to_owned());
    }

    let versions = publishable
        .values()
        .map(|package| package.version.as_str())
        .collect::<BTreeSet<_>>();
    if versions.len() != 1 {
        return Err(format!(
            "publishable packages do not use one lockstep version: {}",
            versions.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    let version = publishable
        .values()
        .next()
        .expect("non-empty publishable graph")
        .version
        .clone();

    let mut dependencies = BTreeMap::<String, BTreeSet<String>>::new();
    for package in publishable.values() {
        let mut internal = BTreeSet::new();
        for dependency in package
            .dependencies
            .iter()
            .filter(|dependency| !dependency.is_dev)
        {
            let Some(target) = candidates
                .iter()
                .find(|candidate| candidate.name == dependency.name)
            else {
                continue;
            };
            if !target.publishable {
                return Err(format!(
                    "publishable package `{}` depends on unpublished workspace package `{}`",
                    package.name, dependency.name
                ));
            }
            let expected = format!("={}", target.version);
            if dependency.requirement != expected {
                return Err(format!(
                    "internal dependency `{} -> {}` must use exact pin `{expected}`, found `{}`",
                    package.name, dependency.name, dependency.requirement
                ));
            }
            internal.insert(dependency.name.clone());
        }
        dependencies.insert(package.name.clone(), internal);
    }

    let mut remaining = dependencies.clone();
    let mut published = BTreeSet::new();
    let mut tiers = Vec::new();
    while !remaining.is_empty() {
        let tier = remaining
            .iter()
            .filter(|(_, required)| required.is_subset(&published))
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        if tier.is_empty() {
            let cycle = remaining.keys().cloned().collect::<Vec<_>>().join(", ");
            return Err(format!(
                "publishable package graph contains a dependency cycle involving: {cycle}"
            ));
        }
        for name in &tier {
            remaining.remove(name);
            published.insert(name.clone());
        }
        tiers.push(tier);
    }

    let packages = publishable
        .values()
        .map(|package| ReleasePackage {
            name: package.name.clone(),
            version: package.version.clone(),
            manifest_path: package.manifest_path.clone(),
            dependencies: dependencies
                .get(&package.name)
                .expect("dependency entry")
                .iter()
                .cloned()
                .collect(),
        })
        .collect();

    Ok(ReleaseGraph {
        version,
        packages,
        tiers,
    })
}

fn package(
    metadata: &CargoMetadata,
    graph: &ReleaseGraph,
    output_dir: &Path,
) -> Result<(), String> {
    ensure_clean_release_tree(&metadata.workspace_root)?;
    if output_dir.exists()
        && fs::read_dir(output_dir)
            .map_err(|error| format!("read {}: {error}", output_dir.display()))?
            .next()
            .is_some()
    {
        return Err(format!(
            "package output must be empty: {}",
            output_dir.display()
        ));
    }
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("create {}: {error}", output_dir.display()))?;

    let tier_lookup = graph
        .tiers
        .iter()
        .enumerate()
        .flat_map(|(tier, packages)| packages.iter().map(move |name| (name.as_str(), tier)))
        .collect::<BTreeMap<_, _>>();

    let cargo_package_dir = metadata.target_directory.join("package");
    let mut packaged = Vec::new();
    for package in &graph.packages {
        let archive_name = format!("{}-{}.crate", package.name, package.version);
        let cargo_archive = create_cargo_archive(metadata, graph, package)?;
        debug_assert_eq!(cargo_archive, cargo_package_dir.join(&archive_name));

        let output_archive = output_dir.join(&archive_name);
        fs::copy(&cargo_archive, &output_archive).map_err(|error| {
            format!(
                "copy {} to {}: {error}",
                cargo_archive.display(),
                output_archive.display()
            )
        })?;
        packaged.push(PackagedCrate {
            name: package.name.clone(),
            version: package.version.clone(),
            tier: *tier_lookup
                .get(package.name.as_str())
                .expect("package belongs to a tier"),
            archive: archive_name,
            sha256: sha256_file(&output_archive)?,
            dependencies: package.dependencies.clone(),
        });
    }
    packaged.sort_by(|left, right| left.name.cmp(&right.name));

    let manifest = PackageManifest {
        version: graph.version.clone(),
        packages: packaged,
    };
    let manifest_path = output_dir.join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest)
            .map_err(|error| format!("serialize package manifest: {error}"))?,
    )
    .map_err(|error| format!("write {}: {error}", manifest_path.display()))?;
    println!("{}", manifest_path.display());
    Ok(())
}

fn create_cargo_archive(
    metadata: &CargoMetadata,
    graph: &ReleaseGraph,
    package: &ReleasePackage,
) -> Result<PathBuf, String> {
    let archive_name = format!("{}-{}.crate", package.name, package.version);
    let cargo_archive = metadata.target_directory.join("package").join(archive_name);
    if cargo_archive.exists() {
        fs::remove_file(&cargo_archive)
            .map_err(|error| format!("remove stale {}: {error}", cargo_archive.display()))?;
    }

    let mut command = Command::new("cargo");
    command.current_dir(&metadata.workspace_root).args([
        "package",
        "--locked",
        "--no-verify",
        "-p",
        &package.name,
    ]);
    for patch in package_patch_args(metadata, graph, package)? {
        command.args(["--config", &patch]);
    }
    command_status(&mut command, &format!("package {}", package.name))?;
    if !cargo_archive.is_file() {
        return Err(format!(
            "cargo did not create expected archive {}",
            cargo_archive.display()
        ));
    }
    Ok(cargo_archive)
}

fn package_patch_args(
    metadata: &CargoMetadata,
    graph: &ReleaseGraph,
    package: &ReleasePackage,
) -> Result<Vec<String>, String> {
    let patch_names = dependency_closure(graph, &package.name);
    graph
        .packages
        .iter()
        .filter(|candidate| patch_names.contains(&candidate.name))
        .map(|candidate| {
            let parent = candidate.manifest_path.parent().ok_or_else(|| {
                format!(
                    "manifest has no parent: {}",
                    candidate.manifest_path.display()
                )
            })?;
            let package_dir = metadata.workspace_root.join(parent);
            Ok(format!(
                "patch.crates-io.{}.path={:?}",
                candidate.name,
                package_dir.to_string_lossy()
            ))
        })
        .collect()
}

fn dependency_closure(graph: &ReleaseGraph, root: &str) -> BTreeSet<String> {
    let packages = graph
        .packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let mut pending = packages
        .get(root)
        .map_or_else(Vec::new, |package| package.dependencies.clone());
    let mut closure = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if !closure.insert(name.clone()) {
            continue;
        }
        if let Some(package) = packages.get(name.as_str()) {
            pending.extend(package.dependencies.iter().cloned());
        }
    }
    closure
}

fn prepare_rehearsal(
    graph: &ReleaseGraph,
    input_dir: &Path,
    output_dir: &Path,
) -> Result<(), String> {
    if output_dir.exists() {
        return Err(format!(
            "rehearsal output already exists: {}",
            output_dir.display()
        ));
    }
    let manifest_path = input_dir.join("manifest.json");
    let manifest = read_package_manifest(&manifest_path)?;
    verify_package_manifest(graph, &manifest, input_dir)?;

    let archives_dir = output_dir.join("archives");
    let packages_dir = output_dir.join("packages");
    fs::create_dir_all(&archives_dir)
        .map_err(|error| format!("create {}: {error}", archives_dir.display()))?;
    fs::create_dir_all(&packages_dir)
        .map_err(|error| format!("create {}: {error}", packages_dir.display()))?;
    let output_dir = fs::canonicalize(output_dir)
        .map_err(|error| format!("canonicalize {}: {error}", output_dir.display()))?;
    let archives_dir = output_dir.join("archives");
    let packages_dir = output_dir.join("packages");

    let mut cargo_config = String::from("[patch.crates-io]\n");
    let mut inventory = String::from("package\tversion\tarchive\n");
    for package in &manifest.packages {
        let source_archive = input_dir.join(&package.archive);
        let output_archive = archives_dir.join(&package.archive);
        fs::copy(&source_archive, &output_archive).map_err(|error| {
            format!(
                "copy {} to {}: {error}",
                source_archive.display(),
                output_archive.display()
            )
        })?;
        command_status(
            Command::new("tar")
                .args(["-xzf"])
                .arg(&output_archive)
                .arg("-C")
                .arg(&packages_dir),
            &format!("unpack {}", package.archive),
        )?;

        let package_dir = packages_dir.join(format!("{}-{}", package.name, package.version));
        if !package_dir.join("Cargo.toml").is_file() || !package_dir.join("Cargo.lock").is_file() {
            return Err(format!(
                "{} is missing its normalized manifest or lockfile",
                package.archive
            ));
        }
        cargo_config.push_str(&format!(
            "{} = {{ path = {:?} }}\n",
            package.name,
            package_dir.to_string_lossy()
        ));
        inventory.push_str(&format!(
            "{}\t{}\t{}\n",
            package.name, package.version, package.archive
        ));
    }

    fs::write(output_dir.join("cargo-config.toml"), cargo_config)
        .map_err(|error| format!("write rehearsal Cargo config: {error}"))?;
    fs::write(output_dir.join("packages.tsv"), inventory)
        .map_err(|error| format!("write rehearsal package inventory: {error}"))?;
    fs::copy(&manifest_path, output_dir.join("manifest.json"))
        .map_err(|error| format!("copy package manifest: {error}"))?;
    println!("{}", output_dir.display());
    Ok(())
}

fn publish(
    metadata: &CargoMetadata,
    graph: &ReleaseGraph,
    requested_version: &str,
) -> Result<(), String> {
    ensure_clean_release_tree(&metadata.workspace_root)?;
    if graph.version != requested_version {
        return Err(format!(
            "requested version {requested_version} does not match workspace version {}",
            graph.version
        ));
    }
    ensure_release_tag(&metadata.workspace_root, requested_version)?;

    let manifest_path = env::var_os("QUASAR_RELEASE_MANIFEST")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            metadata
                .workspace_root
                .join("target/release-packages/manifest.json")
        });
    let manifest = read_package_manifest(&manifest_path)?;
    let archive_dir = manifest_path.parent().unwrap_or(Path::new("."));
    verify_package_manifest(graph, &manifest, archive_dir)?;

    for tier in &graph.tiers {
        for name in tier {
            let packaged = manifest
                .packages
                .iter()
                .find(|package| &package.name == name)
                .expect("verified package manifest");
            if let Some(checksum) = published_checksum(name, requested_version)? {
                if checksum == packaged.sha256 {
                    println!("{name}@{requested_version} already published with matching contents");
                    continue;
                }
                return Err(format!(
                    "{name}@{requested_version} is already published with checksum {checksum}, \
                     expected {}",
                    packaged.sha256
                ));
            }
            let package = graph
                .packages
                .iter()
                .find(|package| &package.name == name)
                .expect("verified release graph");
            let repackaged = create_cargo_archive(metadata, graph, package)?;
            let repackaged_checksum = sha256_file(&repackaged)?;
            if repackaged_checksum != packaged.sha256 {
                return Err(format!(
                    "workspace repackaged {} as {repackaged_checksum}, but rehearsal approved {}",
                    package.name, packaged.sha256
                ));
            }
            let mut command = Command::new("cargo");
            command
                .current_dir(&metadata.workspace_root)
                .args(["publish", "--locked", "-p", name]);
            command_status(&mut command, &format!("publish {name}"))?;
        }
        for name in tier {
            let expected = manifest
                .packages
                .iter()
                .find(|package| &package.name == name)
                .expect("verified package manifest");
            wait_for_crate(name, requested_version, &expected.sha256)?;
        }
    }
    Ok(())
}

fn read_package_manifest(path: &Path) -> Result<PackageManifest, String> {
    serde_json::from_slice(
        &fs::read(path)
            .map_err(|error| format!("read package manifest {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("parse package manifest {}: {error}", path.display()))
}

fn verify_package_manifest(
    graph: &ReleaseGraph,
    manifest: &PackageManifest,
    archive_dir: &Path,
) -> Result<(), String> {
    if manifest.version != graph.version {
        return Err(format!(
            "package manifest version {} does not match graph version {}",
            manifest.version, graph.version
        ));
    }
    let expected = graph
        .packages
        .iter()
        .map(|package| package.name.as_str())
        .collect::<BTreeSet<_>>();
    let actual = manifest
        .packages
        .iter()
        .map(|package| package.name.as_str())
        .collect::<BTreeSet<_>>();
    if expected != actual || actual.len() != manifest.packages.len() {
        return Err(
            "package manifest does not contain every publishable crate exactly once".into(),
        );
    }
    for package in &manifest.packages {
        let expected_package = graph
            .packages
            .iter()
            .find(|expected| expected.name == package.name)
            .expect("package names already verified");
        let expected_tier = graph
            .tiers
            .iter()
            .position(|tier| tier.contains(&package.name))
            .expect("graph package belongs to a tier");
        if package.version != expected_package.version
            || package.dependencies != expected_package.dependencies
            || package.tier != expected_tier
            || package.archive != format!("{}-{}.crate", package.name, package.version)
        {
            return Err(format!(
                "package manifest entry for `{}` does not match the release graph",
                package.name
            ));
        }
        let archive = archive_dir.join(&package.archive);
        if !archive.is_file() {
            return Err(format!("missing package archive: {}", archive.display()));
        }
        let checksum = sha256_file(&archive)?;
        if checksum != package.sha256 {
            return Err(format!(
                "package archive checksum mismatch for {}",
                archive.display()
            ));
        }
    }
    Ok(())
}

fn ensure_clean_release_tree(workspace_root: &Path) -> Result<(), String> {
    let output = command_output(
        Command::new("git").current_dir(workspace_root).args([
            "status",
            "--porcelain",
            "--untracked-files=normal",
        ]),
        "inspect release worktree",
    )?;
    if !output.stdout.is_empty() {
        return Err("release worktree is dirty".to_owned());
    }
    Ok(())
}

fn ensure_release_tag(workspace_root: &Path, version: &str) -> Result<(), String> {
    let output = command_output(
        Command::new("git")
            .current_dir(workspace_root)
            .args(["tag", "--points-at", "HEAD"]),
        "inspect release tag",
    )?;
    let expected = format!("v{version}");
    let tags = String::from_utf8(output.stdout)
        .map_err(|error| format!("release tag output is not UTF-8: {error}"))?;
    if !tags.lines().any(|tag| tag == expected) {
        return Err(format!(
            "release HEAD must have tag `{expected}` before publishing"
        ));
    }
    Ok(())
}

fn published_checksum(name: &str, version: &str) -> Result<Option<String>, String> {
    let url = format!("https://crates.io/api/v1/crates/{name}/{version}");
    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--output",
            "-",
            "--write-out",
            "\n%{http_code}",
            &url,
        ])
        .output()
        .map_err(|error| format!("query crates.io for {name}@{version}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "query crates.io for {name}@{version} failed with {}",
            output.status
        ));
    }
    let response = String::from_utf8(output.stdout).map_err(|error| {
        format!("crates.io response for {name}@{version} is not UTF-8: {error}")
    })?;
    let (body, status) = response
        .rsplit_once('\n')
        .ok_or_else(|| format!("crates.io response for {name}@{version} omitted HTTP status"))?;
    if status == "404" {
        return Ok(None);
    }
    if status != "200" {
        return Err(format!(
            "crates.io returned HTTP {status} for {name}@{version}"
        ));
    }
    let response: serde_json::Value = serde_json::from_str(body)
        .map_err(|error| format!("parse crates.io response for {name}@{version}: {error}"))?;
    Ok(response
        .get("version")
        .and_then(|version| version.get("checksum"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned))
}

fn wait_for_crate(name: &str, version: &str, expected_checksum: &str) -> Result<(), String> {
    for _ in 0..60 {
        if let Some(checksum) = published_checksum(name, version)? {
            return if checksum == expected_checksum {
                Ok(())
            } else {
                Err(format!(
                    "{name}@{version} was published with checksum {checksum}, expected \
                     {expected_checksum}"
                ))
            };
        }
        thread::sleep(Duration::from_secs(5));
    }
    Err(format!(
        "{name}@{version} did not become available from crates.io"
    ))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let digest = Sha256::digest(bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn command_output(command: &mut Command, action: &str) -> Result<Output, String> {
    let output = command
        .output()
        .map_err(|error| format!("{action}: {error}"))?;
    if output.status.success() {
        return Ok(output);
    }
    Err(format!(
        "{action} failed with {}:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn command_status(command: &mut Command, action: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("{action}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{action} failed with {status}"))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{fs, path::PathBuf, process::Command},
    };

    fn package(name: &str, dependencies: &[(&str, &str)], publishable: bool) -> Candidate {
        Candidate {
            name: name.to_owned(),
            version: "0.1.0".to_owned(),
            manifest_path: PathBuf::from(name).join("Cargo.toml"),
            publishable,
            dependencies: dependencies
                .iter()
                .map(|(name, requirement)| CandidateDependency {
                    name: (*name).to_owned(),
                    requirement: (*requirement).to_owned(),
                    is_dev: false,
                })
                .collect(),
        }
    }

    #[test]
    fn chains_are_published_dependency_first() {
        let graph = build_graph(&[
            package("leaf", &[], true),
            package("middle", &[("leaf", "=0.1.0")], true),
            package("root", &[("middle", "=0.1.0")], true),
        ])
        .unwrap();
        assert_eq!(graph.tiers, [["leaf"], ["middle"], ["root"]]);
    }

    #[test]
    fn diamonds_share_a_tier_and_independent_roots_are_discovered() {
        let graph = build_graph(&[
            package("base", &[], true),
            package("left", &[("base", "=0.1.0")], true),
            package("right", &[("base", "=0.1.0")], true),
            package("top", &[("left", "=0.1.0"), ("right", "=0.1.0")], true),
            package("independent", &[], true),
        ])
        .unwrap();
        assert_eq!(
            graph.tiers,
            [
                vec!["base", "independent"],
                vec!["left", "right"],
                vec!["top"]
            ]
        );
    }

    #[test]
    fn cycles_are_rejected() {
        let error = build_graph(&[
            package("left", &[("right", "=0.1.0")], true),
            package("right", &[("left", "=0.1.0")], true),
        ])
        .unwrap_err();
        assert!(error.contains("cycle"), "{error}");
    }

    #[test]
    fn unpublished_members_are_excluded_but_cannot_be_release_dependencies() {
        let graph = build_graph(&[
            package("published", &[], true),
            package("fixture", &[], false),
        ])
        .unwrap();
        assert_eq!(graph.packages.len(), 1);
        assert_eq!(graph.packages[0].name, "published");

        let error = build_graph(&[
            package("published", &[("fixture", "=0.1.0")], true),
            package("fixture", &[], false),
        ])
        .unwrap_err();
        assert!(error.contains("unpublished workspace package"), "{error}");
    }

    #[test]
    fn internal_dependencies_require_exact_lockstep_pins() {
        let error = build_graph(&[
            package("base", &[], true),
            package("root", &[("base", "^0.1.0")], true),
        ])
        .unwrap_err();
        assert!(error.contains("exact pin `=0.1.0`"), "{error}");
    }

    #[test]
    fn cargo_metadata_is_the_only_package_inventory() {
        let root = PathBuf::from("/workspace");
        let package = |name: &str, publish: Option<Vec<String>>| CargoPackage {
            id: format!("{name} 0.1.0 (path+file:///workspace/{name})"),
            name: name.to_owned(),
            version: "0.1.0".to_owned(),
            manifest_path: root.join(name).join("Cargo.toml"),
            publish,
            dependencies: Vec::new(),
        };
        let with_fixture = CargoMetadata {
            packages: vec![
                package("core", None),
                package("temporary-fixture", None),
                package("internal-tool", Some(Vec::new())),
            ],
            workspace_members: vec![
                "core 0.1.0 (path+file:///workspace/core)".to_owned(),
                "temporary-fixture 0.1.0 (path+file:///workspace/temporary-fixture)".to_owned(),
                "internal-tool 0.1.0 (path+file:///workspace/internal-tool)".to_owned(),
            ],
            workspace_root: root.clone(),
            target_directory: root.join("target"),
        };

        let candidates = candidates_from_metadata(&with_fixture).unwrap();
        let graph = build_graph(&candidates).unwrap();
        assert_eq!(
            graph
                .packages
                .iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>(),
            ["core", "temporary-fixture"]
        );
        assert_eq!(
            graph.packages[1].manifest_path,
            PathBuf::from("temporary-fixture/Cargo.toml")
        );

        let without_fixture = CargoMetadata {
            packages: with_fixture
                .packages
                .into_iter()
                .filter(|package| package.name != "temporary-fixture")
                .collect(),
            workspace_members: with_fixture
                .workspace_members
                .into_iter()
                .filter(|member| !member.starts_with("temporary-fixture "))
                .collect(),
            workspace_root: root.clone(),
            target_directory: root.join("target"),
        };
        let graph = build_graph(&candidates_from_metadata(&without_fixture).unwrap()).unwrap();
        assert_eq!(graph.packages.len(), 1);
        assert_eq!(graph.packages[0].name, "core");
    }

    #[test]
    fn rehearsal_preparation_uses_exact_manifest_archives() {
        let sandbox = tempfile::tempdir().unwrap();
        let input = sandbox.path().join("input");
        let source = sandbox.path().join("source");
        let output = sandbox.path().join("output");
        let package_root = source.join("base-0.1.0");
        fs::create_dir_all(&input).unwrap();
        fs::create_dir_all(&package_root).unwrap();
        fs::write(
            package_root.join("Cargo.toml"),
            "[package]\nname = \"base\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(package_root.join("Cargo.lock"), "version = 4\n").unwrap();
        let archive = input.join("base-0.1.0.crate");
        let status = Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&source)
            .arg("base-0.1.0")
            .status()
            .unwrap();
        assert!(status.success());

        let graph = build_graph(&[package("base", &[], true)]).unwrap();
        let manifest = PackageManifest {
            version: "0.1.0".to_owned(),
            packages: vec![PackagedCrate {
                name: "base".to_owned(),
                version: "0.1.0".to_owned(),
                tier: 0,
                archive: "base-0.1.0.crate".to_owned(),
                sha256: sha256_file(&archive).unwrap(),
                dependencies: Vec::new(),
            }],
        };
        fs::write(
            input.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();

        prepare_rehearsal(&graph, &input, &output).unwrap();

        assert_eq!(
            fs::read(output.join("archives/base-0.1.0.crate")).unwrap(),
            fs::read(&archive).unwrap()
        );
        assert!(output.join("packages/base-0.1.0/Cargo.toml").is_file());
        let config = fs::read_to_string(output.join("cargo-config.toml")).unwrap();
        assert!(config.contains("base = { path = "));
        let inventory = fs::read_to_string(output.join("packages.tsv")).unwrap();
        assert_eq!(
            inventory,
            "package\tversion\tarchive\nbase\t0.1.0\tbase-0.1.0.crate\n"
        );
    }
}
