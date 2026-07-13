use {
    quasar_cli::idl,
    serde_json::Value,
    std::{
        error::Error,
        fs,
        path::{Path, PathBuf},
        process::Command,
    },
    tempfile::tempdir,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn fixture_program() -> PathBuf {
    workspace_root().join("examples/multisig")
}

fn run_command(cmd: &mut Command) -> Result<(), Box<dyn Error>> {
    let output = cmd.output()?;
    if output.status.success() {
        return Ok(());
    }

    let mut message = String::new();
    message.push_str(&format!("command failed: {:?}\n", cmd));
    if !output.stdout.is_empty() {
        message.push_str("stdout:\n");
        message.push_str(&String::from_utf8_lossy(&output.stdout));
        message.push('\n');
    }
    if !output.stderr.is_empty() {
        message.push_str("stderr:\n");
        message.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    Err(message.into())
}

fn compile_rust_client(client_dir: &Path) -> Result<(), Box<dyn Error>> {
    run_command(
        Command::new("cargo")
            .arg("check")
            .arg("--quiet")
            .current_dir(client_dir),
    )
}

fn override_rust_client_with_workspace_path(client_dir: &Path) -> Result<(), Box<dyn Error>> {
    let manifest_path = client_dir.join("Cargo.toml");
    let manifest = read_file(&manifest_path)?;
    let exact_dependency = format!("quasar-lang = \"={}\"", env!("CARGO_PKG_VERSION"));
    let path_dependency = format!(
        "quasar-lang = {{ path = \"{}\" }}",
        workspace_root().join("lang").display()
    );
    let patched = manifest.replace(&exact_dependency, &path_dependency);
    if patched == manifest {
        return Err(format!(
            "generated manifest did not contain expected dependency `{exact_dependency}`"
        )
        .into());
    }
    fs::write(manifest_path, format!("{patched}\n[workspace]\n"))?;
    Ok(())
}

fn add_package_patches(command: &mut Command, packages: &[(&str, PathBuf)]) {
    for (name, path) in packages {
        command.arg("--config").arg(format!(
            "patch.crates-io.{name}.path=\"{}\"",
            path.display()
        ));
    }
}

fn compile_rust_client_from_packages(client_dir: &Path) -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let package_target = temp.path().join("package-target");
    let unpacked_root = temp.path().join("packages");
    fs::create_dir_all(&unpacked_root)?;

    let source_packages = [
        ("quasar-schema", workspace_root().join("schema")),
        ("quasar-idl-schema", workspace_root().join("idl/schema")),
        (
            "solana-compiler-builtins",
            workspace_root().join("solana-compiler-builtins"),
        ),
        ("quasar-derive", workspace_root().join("derive")),
        ("quasar-lang", workspace_root().join("lang")),
    ];
    let mut package = Command::new("cargo");
    package
        .arg("package")
        .arg("--locked")
        .arg("--allow-dirty")
        .arg("--no-verify")
        .env("CARGO_TARGET_DIR", &package_target)
        .current_dir(workspace_root());
    for (name, _) in &source_packages {
        package.arg("-p").arg(name);
    }
    add_package_patches(&mut package, &source_packages);
    run_command(&mut package)?;

    let version = env!("CARGO_PKG_VERSION");
    let mut packaged_dependencies = Vec::new();
    for (name, _) in &source_packages {
        let archive = package_target
            .join("package")
            .join(format!("{name}-{version}.crate"));
        run_command(
            Command::new("tar")
                .arg("-xzf")
                .arg(&archive)
                .arg("-C")
                .arg(&unpacked_root),
        )?;
        let package_dir = unpacked_root.join(format!("{name}-{version}"));
        if !package_dir.join("Cargo.toml").is_file() {
            return Err(
                format!("packaged manifest missing under {}", package_dir.display()).into(),
            );
        }
        packaged_dependencies.push((*name, package_dir));
    }

    let manifest_path = client_dir.join("Cargo.toml");
    let manifest_before = fs::read(&manifest_path)?;
    let mut check = Command::new("cargo");
    check
        .arg("check")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .env("CARGO_TARGET_DIR", temp.path().join("client-target"))
        .current_dir(client_dir);
    add_package_patches(&mut check, &packaged_dependencies);
    run_command(&mut check)?;

    if fs::read(&manifest_path)? != manifest_before {
        return Err("registry-style compilation modified the generated Cargo.toml".into());
    }
    let manifest = String::from_utf8(manifest_before)?;
    assert!(manifest.contains(&format!("quasar-lang = \"={version}\"")));
    assert!(!manifest.contains("git ="));
    assert!(!manifest.contains("branch ="));

    Ok(())
}

fn compile_python_client(client_dir: &Path) -> Result<(), Box<dyn Error>> {
    let solders_version = std::env::var("SOLDERS_VERSION").unwrap_or_else(|_| "0.28.0".into());

    run_command(
        Command::new("python3")
            .arg("-m")
            .arg("py_compile")
            .arg("__init__.py")
            .arg("client.py")
            .current_dir(client_dir),
    )?;

    // `py_compile` only checks syntax. Execute the module against the pinned
    // real solders package so imports, runtime constructors, dataclass field
    // ordering, and every postponed annotation are all validated.
    run_command(
        Command::new("python3")
            .arg("-c")
            .arg(
                r#"
import dataclasses
import importlib.metadata
import importlib.util
import pathlib
import sys
import typing

from solders.instruction import AccountMeta, Instruction
from solders.pubkey import Pubkey

expected_version = sys.argv[1]
actual_version = importlib.metadata.version("solders")
assert actual_version == expected_version, (actual_version, expected_version)

spec = importlib.util.spec_from_file_location("generated_client", pathlib.Path("client.py"))
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

assert isinstance(module.PROGRAM_ID, Pubkey)
probe = Instruction(module.PROGRAM_ID, b"\x00", [])
assert probe.program_id == module.PROGRAM_ID
assert AccountMeta(module.PROGRAM_ID, False, False).pubkey == module.PROGRAM_ID

for value in vars(module).values():
    if isinstance(value, type) and dataclasses.is_dataclass(value):
        typing.get_type_hints(value, vars(module))
"#,
            )
            .arg(solders_version)
            .current_dir(client_dir),
    )
}

fn compile_go_client(client_dir: &Path) -> Result<(), Box<dyn Error>> {
    run_command(
        Command::new("go")
            .arg("mod")
            .arg("tidy")
            .current_dir(client_dir),
    )?;
    run_command(
        Command::new("go")
            .arg("build")
            .arg("./...")
            .current_dir(client_dir),
    )
}

const CARAVEL_HOST_SYSCALLS: &str = r#"#include <string.h>
#define strlen caravel_strlen
#include <caravel.h>
#undef strlen

/*
 * Caravel calls Solana syscalls for PDA derivation. The real headers and real
 * find_program_address implementation remain under test; these two symbols
 * provide the host runtime boundary that the validator normally supplies.
 */
static uint64_t caravel_find_program_address_status = SUCCESS;

uint64_t sol_sha256(
    const SignerSeed *vals,
    uint64_t vals_len,
    uint8_t result[32]
) {
    (void)vals;
    (void)vals_len;
    memset(result, 0, 32);
    return SUCCESS;
}

uint64_t sol_curve_validate_point(
    uint64_t curve_id,
    const uint8_t *point,
    uint8_t *result
) {
    (void)curve_id;
    (void)point;
    (void)result;
    return caravel_find_program_address_status == SUCCESS ? 1 : 0;
}

"#;

fn caravel_include_dir() -> Result<PathBuf, Box<dyn Error>> {
    std::env::var_os("CARAVEL_INCLUDE_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| {
            "CARAVEL_INCLUDE_DIR must point to the pinned Caravel checkout's include directory"
                .into()
        })
}

fn compile_c_client(client_dir: &Path) -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    write_file(&temp.path().join("compile.c"), r#"#include "client.h""#)?;
    let caravel_include_dir = caravel_include_dir()?;

    let cc = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
    run_command(
        Command::new(cc)
            .arg("-std=c11")
            .arg("-fsyntax-only")
            .arg("-Wall")
            .arg("-Wextra")
            .arg("-Werror")
            .arg("-DNO_HEAP")
            .arg("-DNO_SYSTEM")
            .arg("-DNO_TOKEN")
            .arg("-I")
            .arg(client_dir)
            .arg("-I")
            .arg(caravel_include_dir)
            .arg(temp.path().join("compile.c")),
    )?;

    Ok(())
}

fn run_c_sanitized_test(client_dir: &Path, source: &str) -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    write_file(
        &temp.path().join("sanitized_test.c"),
        format!("{CARAVEL_HOST_SYSCALLS}{source}"),
    )?;
    let caravel_include_dir = caravel_include_dir()?;

    let cc = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
    let binary = temp.path().join("sanitized_test");
    run_command(
        Command::new(&cc)
            .arg("-std=c11")
            .arg("-Wall")
            .arg("-Wextra")
            .arg("-Werror")
            .arg("-fsanitize=address,undefined")
            .arg("-fno-omit-frame-pointer")
            .arg("-DNO_HEAP")
            .arg("-DNO_SYSTEM")
            .arg("-DNO_TOKEN")
            .arg("-I")
            .arg(client_dir)
            .arg("-I")
            .arg(caravel_include_dir)
            .arg(temp.path().join("sanitized_test.c"))
            .arg("-o")
            .arg(&binary),
    )?;
    run_command(&mut Command::new(binary))
}

fn compile_typescript_client(client_dir: &Path) -> Result<(), Box<dyn Error>> {
    let typescript_version = std::env::var("TYPESCRIPT_VERSION").unwrap_or_else(|_| "5.9.3".into());
    let node_types_version =
        std::env::var("NODE_TYPES_VERSION").unwrap_or_else(|_| "22.13.0".into());

    fs::write(
        client_dir.join("tsconfig.json"),
        r#"{
  "compilerOptions": {
    "erasableSyntaxOnly": true,
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "noEmit": true,
    "skipLibCheck": true,
    "strict": true,
    "target": "ES2022"
  },
  "include": ["web3.ts", "kit.ts"]
}
"#,
    )?;

    run_command(
        Command::new("npm")
            .arg("install")
            .arg("--package-lock=false")
            .arg("--ignore-scripts")
            .arg("--no-audit")
            .arg("--no-fund")
            .arg(format!("typescript@{typescript_version}"))
            .arg(format!("@types/node@{node_types_version}"))
            .current_dir(client_dir),
    )?;

    run_command(
        Command::new("npx")
            .arg("tsc")
            .arg("-p")
            .arg("tsconfig.json")
            .current_dir(client_dir),
    )
}

fn write_file(path: &Path, contents: impl AsRef<str>) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents.as_ref())?;
    Ok(())
}

fn read_file(path: &Path) -> Result<String, Box<dyn Error>> {
    fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read generated file `{}`: {error}",
            path.display()
        )
        .into()
    })
}

fn read_tree_files(path: &Path, extension: &str) -> Result<String, Box<dyn Error>> {
    let mut stack = vec![path.to_path_buf()];
    let mut contents = String::new();

    while let Some(path) = stack.pop() {
        for entry in fs::read_dir(&path)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
                contents.push_str(&read_file(&path)?);
                contents.push('\n');
            }
        }
    }

    Ok(contents)
}

fn only_child_dir(path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let dirs = fs::read_dir(path)
        .map_err(|error| {
            format!(
                "failed to read generated client dir {}: {error}",
                path.display()
            )
        })?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    match dirs.as_slice() {
        [dir] => Ok(dir.clone()),
        _ => Err(format!(
            "expected exactly one generated client dir under {}",
            path.display()
        )
        .into()),
    }
}

fn assert_typescript_client_requires_address_constraint_accounts(
    client_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    let kit = fs::read_to_string(client_dir.join("kit.ts"))?;
    let web3 = fs::read_to_string(client_dir.join("web3.ts"))?;

    for source in [&kit, &web3] {
        assert!(
            !source.contains(" :: seeds("),
            "generated TypeScript client contains an unresolved typed seed expression"
        );
        assert!(
            source.contains("  config: Address;"),
            "generated TypeScript client should require the config account"
        );
        // `vault` carries a typed-seeds resolver, so the client derives it
        // instead of requiring it as an input.
        assert!(
            !source.contains("  vault: Address;"),
            "vault should be resolver-derived, not a required input"
        );
        assert!(
            source.contains("findVaultAddress"),
            "generated TypeScript client should emit the vault PDA resolver"
        );
    }

    Ok(())
}

#[test]
fn lifecycle_account_types_generate_writable_client_metas() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let program_dir = temp.path().join("programs/lifecycle-client-flags");

    write_file(
        &temp.path().join("Cargo.toml"),
        r#"[workspace]
members = ["programs/lifecycle-client-flags"]
resolver = "3"
"#,
    )?;
    write_file(
        &program_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "lifecycle-client-flags"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[features]
idl-build = ["quasar-lang/idl-build"]

[dependencies]
quasar-lang = {{ path = "{}" }}
"#,
            workspace_root().join("lang").display()
        ),
    )?;
    write_file(
        &program_dir.join("src/lib.rs"),
        r#"
use quasar_lang::prelude::*;

declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
pub struct ConfigV1 {
    pub authority: Address,
    pub value: PodU64,
}

#[account(discriminator = 2)]
pub struct ConfigV2 {
    pub authority: Address,
    pub value: PodU64,
    pub extra: PodU32,
}

#[account(discriminator = 3)]
pub struct Vault {
    pub authority: Address,
    pub value: PodU64,
}

#[derive(Accounts)]
pub struct Touch {
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
    pub config: Migration<ConfigV1, ConfigV2>,
    pub vault: Uninit<Account<Vault>>,
}

#[program]
pub mod lifecycle_client_flags {
    use super::*;

    #[instruction(discriminator = 1)]
    pub fn touch(_ctx: Ctx<Touch>) -> Result<(), ProgramError> {
        Ok(())
    }
}
"#,
    )?;

    let clients_path = temp.path().join("clients");
    idl::generate(
        &program_dir,
        &["typescript", "python", "golang", "c"],
        &clients_path,
    )?;

    let idl_json = read_file(&PathBuf::from("target/idl/lifecycle_client_flags.json"))?;
    let idl_value: Value = serde_json::from_str(&idl_json)?;
    let instructions = idl_value["instructions"]
        .as_array()
        .ok_or("IDL instructions should be an array")?;
    let touch = instructions
        .iter()
        .find(|ix| ix["name"] == "touch")
        .ok_or("touch instruction should be present in IDL")?;
    let accounts = touch["accounts"]
        .as_array()
        .ok_or("touch accounts should be an array")?;

    for name in ["config", "vault"] {
        let account = accounts
            .iter()
            .find(|account| account["name"] == name)
            .ok_or_else(|| format!("{name} account should be present in IDL"))?;
        assert_eq!(
            account["writable"],
            Value::Bool(true),
            "{name} should be emitted as writable in the IDL: {idl_json}"
        );
    }

    let rust_ix = read_tree_files(
        &only_child_dir(&clients_path.join("rust"))?.join("src"),
        "rs",
    )?;
    assert!(rust_ix.contains("AccountMeta::new(ix.config, false)"));
    assert!(rust_ix.contains("AccountMeta::new(ix.vault, false)"));

    let ts_web3 = read_file(&only_child_dir(&clients_path.join("typescript"))?.join("web3.ts"))?;
    assert!(ts_web3.contains("{ pubkey: input.config, isSigner: false, isWritable: true },"));
    assert!(ts_web3.contains("{ pubkey: input.vault, isSigner: false, isWritable: true },"));

    let py_client = read_file(&only_child_dir(&clients_path.join("python"))?.join("client.py"))?;
    assert!(py_client.contains(
        r#"accounts.append(AccountMeta(accounts_map["config"], is_signer=False, is_writable=True))"#
    ));
    assert!(py_client.contains(
        r#"accounts.append(AccountMeta(accounts_map["vault"], is_signer=False, is_writable=True))"#
    ));

    let go_client = read_file(&only_child_dir(&clients_path.join("golang"))?.join("client.go"))?;
    assert!(go_client
        .contains(r#"accounts = append(accounts, solana.Meta(accountsMap["config"]).WRITE())"#));
    assert!(go_client
        .contains(r#"accounts = append(accounts, solana.Meta(accountsMap["vault"]).WRITE())"#));

    let c_client = read_file(&only_child_dir(&clients_path.join("c"))?.join("client.h"))?;
    assert!(c_client.contains("meta_buf[2] = meta_writable(accounts->config);"));
    assert!(c_client.contains("meta_buf[3] = meta_writable(accounts->vault);"));
    compile_c_client(&only_child_dir(&clients_path.join("c"))?)?;

    Ok(())
}

#[test]
fn generated_clients_compile_from_fresh_project() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_program();

    let temp = tempdir()?;
    let clients_path = temp.path().join("clients");
    idl::generate(
        &fixture,
        &["typescript", "python", "golang", "c"],
        &clients_path,
    )?;

    // The IDL is generated relative to the workspace; find the rust client dir
    // by convention.
    let rust_client_dir = only_child_dir(&clients_path.join("rust"))?;
    if rust_client_dir.exists() {
        compile_rust_client_from_packages(&rust_client_dir)?;
    }

    let ts_dir = only_child_dir(&clients_path.join("typescript"))?;
    if ts_dir.exists() {
        assert_typescript_client_requires_address_constraint_accounts(&ts_dir)?;
        let kit = read_file(&ts_dir.join("kit.ts"))?;
        assert!(
            kit.contains("from \"@solana/kit/program-client-core\""),
            "Kit client should import program plugin helpers"
        );
        assert!(
            kit.contains("export function quasarMultisigProgram()"),
            "Kit client should expose a program plugin factory"
        );
        assert!(
            kit.contains("addSelfPlanAndSendFunctions"),
            "Kit program plugin should expose self plan/send instruction helpers"
        );
        compile_typescript_client(&ts_dir)?;
    }

    let py_dir = only_child_dir(&clients_path.join("python"))?;
    if py_dir.exists() {
        compile_python_client(&py_dir)?;
    }

    let go_dir = only_child_dir(&clients_path.join("golang"))?;
    if go_dir.exists() {
        compile_go_client(&go_dir)?;
    }

    let c_dir = only_child_dir(&clients_path.join("c"))?;
    compile_c_client(&c_dir)?;
    run_c_sanitized_test(
        &c_dir,
        r#"#include <assert.h>
#include <string.h>
#include "client.h"

static void assert_untouched(
    const Instruction *ix,
    const Instruction *ix_before,
    const AccountMeta *metas,
    const AccountMeta *metas_before,
    uint64_t metas_len,
    const uint8_t *data,
    const uint8_t *data_before,
    uint64_t data_len
) {
    assert(memcmp(ix, ix_before, sizeof(*ix)) == 0);
    assert(memcmp(metas, metas_before, sizeof(*metas) * metas_len) == 0);
    assert(memcmp(data, data_before, data_len) == 0);
}

int main(void) {
    Pubkey creator = { .bytes = {1} };
    Pubkey rent = { .bytes = {2} };
    Pubkey system_program = { .bytes = {3} };
    Pubkey extra_a = { .bytes = {4} };
    Pubkey extra_b = { .bytes = {5} };
    quasar_multisig_create_accounts_t accounts = {
        .creator = &creator,
        .rent = &rent,
        .systemProgram = &system_program,
    };
    quasar_multisig_create_args_t args = { .threshold = 7 };
    AccountMeta remaining[] = {
        meta_readonly(&extra_a),
        meta_writable_signer(&extra_b),
    };
    Instruction ix;
    Instruction ix_before;
    AccountMeta metas[6];
    AccountMeta metas_before[6];
    uint8_t data[2];
    uint8_t data_before[2];

    memset(&ix, 0xa5, sizeof(ix));
    memset(metas, 0xa5, sizeof(metas));
    memset(data, 0xa5, sizeof(data));
    memcpy(&ix_before, &ix, sizeof(ix));
    memcpy(metas_before, metas, sizeof(metas));
    memcpy(data_before, data, sizeof(data));

    quasar_multisig_ix_result_t result = quasar_multisig_create_ix(
        &accounts, &args, remaining, 2, &ix, metas, 5, data, sizeof(data));
    assert(result.status == QUASAR_MULTISIG_IX_ACCOUNT_BUFFER_TOO_SMALL);
    assert(result.accounts_len == 6 && result.data_len == 2);
    assert_untouched(&ix, &ix_before, metas, metas_before, 6, data, data_before, 2);

    result = quasar_multisig_create_ix(
        &accounts, &args, remaining, 2, &ix, metas, 6, data, 1);
    assert(result.status == QUASAR_MULTISIG_IX_DATA_BUFFER_TOO_SMALL);
    assert(result.accounts_len == 6 && result.data_len == 2);
    assert_untouched(&ix, &ix_before, metas, metas_before, 6, data, data_before, 2);

    result = quasar_multisig_create_ix(
        &accounts, &args, NULL, (uint64_t)-1, &ix, metas, 6, data, sizeof(data));
    assert(result.status == QUASAR_MULTISIG_IX_LENGTH_OVERFLOW);
    assert(result.accounts_len == (uint64_t)-1);
    assert_untouched(&ix, &ix_before, metas, metas_before, 6, data, data_before, 2);

    caravel_find_program_address_status = ERROR_INVALID_PDA;
    result = quasar_multisig_create_ix(
        &accounts, &args, remaining, 2, &ix, metas, 6, data, sizeof(data));
    assert(result.status == QUASAR_MULTISIG_IX_PDA_DERIVATION_FAILED);
    assert(result.pda_status == ERROR_INVALID_PDA);
    assert(result.accounts_len == 6 && result.data_len == 2);
    assert_untouched(&ix, &ix_before, metas, metas_before, 6, data, data_before, 2);

    caravel_find_program_address_status = SUCCESS;
    result = quasar_multisig_create_ix(
        &accounts, &args, remaining, 2, &ix, metas, 6, data, sizeof(data));
    assert(result.status == QUASAR_MULTISIG_IX_OK);
    assert(result.pda_status == SUCCESS);
    assert(result.accounts_len == 6 && result.data_len == 2);
    assert(ix.program_id == (Pubkey *)&QUASAR_MULTISIG_PROGRAM_ID);
    assert(ix.accounts == metas && ix.accounts_len == 6);
    assert(ix.data == data && ix.data_len == 2);
    assert(data[0] == 0 && data[1] == 7);
    assert(metas[0].pubkey == &creator && metas[0].is_signer && metas[0].is_writable);
    assert(metas[1].pubkey != NULL && !metas[1].is_signer && metas[1].is_writable);
    assert(metas[2].pubkey == &rent && !metas[2].is_signer && !metas[2].is_writable);
    assert(metas[3].pubkey == &system_program);
    assert(metas[4].pubkey == &extra_a && metas[5].pubkey == &extra_b);
    return 0;
}
"#,
    )?;

    Ok(())
}

#[test]
fn generated_decoders_reject_malformed_bytes_in_every_language() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let program_dir = temp.path().join("programs/decoder-total");

    write_file(
        &temp.path().join("Cargo.toml"),
        r#"[workspace]
members = ["programs/decoder-total"]
resolver = "3"
"#,
    )?;
    write_file(
        &program_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "decoder-total"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[features]
idl-build = ["quasar-lang/idl-build"]

[dependencies]
quasar-lang = {{ path = "{}" }}
"#,
            workspace_root().join("lang").display()
        ),
    )?;
    write_file(
        &program_dir.join("src/lib.rs"),
        r#"#![no_std]
use quasar_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account(discriminator = 1)]
pub struct DecodeFixture {
    pub enabled: bool,
    pub maybe: Option<u16>,
    pub label: String<70000, 4>,
    pub bytes: Vec<u8, 70000, 4>,
}

#[derive(Accounts)]
pub struct Noop {
    pub authority: Signer,
}

#[program]
mod decoder_total {
    use super::*;

    #[instruction(discriminator = 9)]
    pub fn noop(_ctx: Ctx<Noop>) -> Result<(), ProgramError> {
        Ok(())
    }
}
"#,
    )?;

    let clients_path = temp.path().join("clients");
    idl::generate(
        &program_dir,
        &["typescript", "python", "golang", "c"],
        &clients_path,
    )?;

    // Account discriminator + fixed fields + both u32 prefixes + both tails.
    let rust_dir = only_child_dir(&clients_path.join("rust"))?;
    override_rust_client_with_workspace_path(&rust_dir)?;
    write_file(
        &rust_dir.join("tests/decoder.rs"),
        r#"use decoder_total_client::state::{decode_account, ProgramAccount};

const VALID: &[u8] = &[
    1,
    1, 1, 0x34, 0x12,
    2, 0, 0, 0,
    3, 0, 0, 0,
    b'o', b'k', 1, 2, 3,
];

fn reject(bytes: &[u8]) {
    let result = std::panic::catch_unwind(|| decode_account(bytes));
    assert!(result.is_ok(), "decoder panicked for {bytes:?}");
    assert!(result.unwrap().is_none(), "decoder accepted {bytes:?}");
}

#[test]
fn malformed_inputs_are_total() {
    for end in 0..VALID.len() { reject(&VALID[..end]); }

    let mut malformed = VALID.to_vec();
    malformed[1] = 2;
    reject(&malformed);
    malformed.copy_from_slice(VALID);
    malformed[2] = 2;
    reject(&malformed);
    malformed.copy_from_slice(VALID);
    malformed[5..9].fill(0xff);
    reject(&malformed);
    malformed.copy_from_slice(VALID);
    malformed[13] = 0xff;
    reject(&malformed);
    malformed.copy_from_slice(VALID);
    malformed.push(0);
    reject(&malformed);

    // Deterministic fuzz-style coverage of forged prefixes and arbitrary bytes.
    for len in 0..4096usize {
        let bytes = (0..len)
            .map(|index| ((index.wrapping_mul(31) ^ len.wrapping_mul(17)) & 0xff) as u8)
            .collect::<Vec<_>>();
        assert!(std::panic::catch_unwind(|| decode_account(&bytes)).is_ok());
    }

    let Some(ProgramAccount::DecodeFixture(value)) = decode_account(VALID) else {
        panic!("valid vector was rejected");
    };
    assert!(value.enabled);
    assert_eq!(value.maybe, Some(0x1234));
    assert_eq!(value.label.as_bytes(), b"ok");
    assert_eq!(value.bytes.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3]);
}
"#,
    )?;
    run_command(
        Command::new("cargo")
            .arg("test")
            .arg("--quiet")
            .current_dir(&rust_dir),
    )?;

    let python_dir = only_child_dir(&clients_path.join("python"))?;
    compile_python_client(&python_dir)?;
    write_file(
        &python_dir.join("decoder_test.py"),
        r#"import importlib.util
import pathlib
import sys

spec = importlib.util.spec_from_file_location("client", pathlib.Path("client.py"))
client = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = client
spec.loader.exec_module(client)

valid = bytes([1, 1, 0x34, 0x12, 2, 0, 0, 0, 3, 0, 0, 0, 111, 107, 1, 2, 3])

def reject(data):
    try: client.DecodeFixture.decode(data)
    except (ValueError, UnicodeDecodeError): return
    raise AssertionError(f"accepted malformed bytes: {data!r}")

for end in range(len(valid)): reject(valid[:end])
for index, value in [(0, 2), (1, 2), (12, 0xff)]:
    malformed = bytearray(valid); malformed[index] = value; reject(bytes(malformed))
malformed = bytearray(valid); malformed[4:8] = b"\xff" * 4; reject(bytes(malformed))
reject(valid + b"\x00")

value = client.DecodeFixture.decode(valid)
assert value.enabled is True
assert value.maybe == 0x1234
assert value.label == "ok"
assert value.bytes == [1, 2, 3]
"#,
    )?;
    run_command(
        Command::new("python3")
            .arg("decoder_test.py")
            .current_dir(&python_dir),
    )?;

    let go_dir = only_child_dir(&clients_path.join("golang"))?;
    write_file(
        &go_dir.join("decoder_test.go"),
        r#"package decoder_total

import "testing"

var validDecodeFixture = []byte{
    1, 1, 0x34, 0x12,
    2, 0, 0, 0,
    3, 0, 0, 0,
    'o', 'k', 1, 2, 3,
}

func reject(t *testing.T, data []byte) {
    t.Helper()
    if _, err := DecodeDecodeFixture(data); err == nil { t.Fatalf("accepted malformed bytes: %v", data) }
}

func TestMalformedDecodeFixture(t *testing.T) {
    for end := 0; end < len(validDecodeFixture); end++ { reject(t, validDecodeFixture[:end]) }
    for index, value := range map[int]byte{0: 2, 1: 2, 12: 0xff} {
        malformed := append([]byte(nil), validDecodeFixture...); malformed[index] = value; reject(t, malformed)
    }
    malformed := append([]byte(nil), validDecodeFixture...)
    copy(malformed[4:8], []byte{0xff, 0xff, 0xff, 0xff})
    reject(t, malformed)
    reject(t, append(append([]byte(nil), validDecodeFixture...), 0))

    value, err := DecodeDecodeFixture(validDecodeFixture)
    if err != nil { t.Fatal(err) }
    if !value.Enabled || value.Maybe == nil || *value.Maybe != 0x1234 || value.Label != "ok" {
        t.Fatalf("unexpected value: %#v", value)
    }
    if len(value.Bytes) != 3 || value.Bytes[0] != 1 || value.Bytes[1] != 2 || value.Bytes[2] != 3 {
        t.Fatalf("unexpected bytes: %v", value.Bytes)
    }
}
"#,
    )?;
    compile_go_client(&go_dir)?;
    run_command(
        Command::new("go")
            .arg("test")
            .arg("./...")
            .current_dir(&go_dir),
    )?;

    let ts_dir = only_child_dir(&clients_path.join("typescript"))?;
    compile_typescript_client(&ts_dir)?;
    write_file(
        &ts_dir.join("decoder_test.ts"),
        r#"import { DecoderTotalClient } from "./web3.ts";

const client = new DecoderTotalClient();
const valid = Uint8Array.from([
  1,
  1, 1, 0x34, 0x12,
  2, 0, 0, 0,
  3, 0, 0, 0,
  111, 107, 1, 2, 3,
]);

function reject(data: Uint8Array): void {
  try { client.decodeDecodeFixture(data); }
  catch { return; }
  throw new Error(`accepted malformed bytes: ${data}`);
}

for (let end = 0; end < valid.length; end++) reject(valid.slice(0, end));
for (const [index, byte] of [[1, 2], [2, 2], [13, 0xff]] as const) {
  const malformed = valid.slice(); malformed[index] = byte; reject(malformed);
}
const forged = valid.slice(); forged.fill(0xff, 5, 9); reject(forged);
const trailing = Uint8Array.from([...valid, 0]); reject(trailing);

const value = client.decodeDecodeFixture(valid);
if (!value.enabled || value.maybe !== 0x1234 || value.label !== "ok") throw new Error("invalid value");
if (value.bytes.length !== 3 || value.bytes[0] !== 1 || value.bytes[1] !== 2 || value.bytes[2] !== 3) {
  throw new Error("invalid bytes");
}
"#,
    )?;
    run_command(
        Command::new("node")
            .arg("--experimental-strip-types")
            .arg("decoder_test.ts")
            .current_dir(&ts_dir),
    )?;

    let c_dir = only_child_dir(&clients_path.join("c"))?;
    compile_c_client(&c_dir)?;
    run_c_sanitized_test(
        &c_dir,
        r#"#include <assert.h>
#include <string.h>
#include "client.h"

static const uint8_t valid[] = {
    1, 1, 0x34, 0x12,
    2, 0, 0, 0,
    3, 0, 0, 0,
    'o', 'k', 1, 2, 3,
};

static void reject(const uint8_t *data, uint64_t len) {
    decoder_total_decode_fixture_t out;
    assert(!decoder_total_decode_fixture_decode(data, len, &out));
}

int main(void) {
    for (uint64_t len = 0; len < sizeof(valid); len++) reject(valid, len);

    uint8_t malformed[sizeof(valid) + 1];
    memcpy(malformed, valid, sizeof(valid)); malformed[0] = 2; reject(malformed, sizeof(valid));
    memcpy(malformed, valid, sizeof(valid)); malformed[1] = 2; reject(malformed, sizeof(valid));
    memcpy(malformed, valid, sizeof(valid)); memset(&malformed[4], 0xff, 4); reject(malformed, sizeof(valid));
    memcpy(malformed, valid, sizeof(valid)); malformed[12] = 0xff; reject(malformed, sizeof(valid));
    memcpy(malformed, valid, sizeof(valid)); malformed[sizeof(valid)] = 0; reject(malformed, sizeof(malformed));

    decoder_total_decode_fixture_t out;
    assert(decoder_total_decode_fixture_decode(valid, sizeof(valid), &out));
    assert(out.enabled && out.maybe_present && out.maybe == 0x1234);
    assert(out.label_len == 2 && memcmp(out.label, "ok", 2) == 0);
    assert(out.bytes_len == 3 && out.bytes[0] == 1 && out.bytes[1] == 2 && out.bytes[2] == 3);
    return 0;
}
"#,
    )?;

    Ok(())
}

#[test]
fn generated_typescript_client_encodes_fixed_byte_array_args() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let program_dir = temp.path().join("programs/fixed-array-args");

    write_file(
        &temp.path().join("Cargo.toml"),
        r#"[workspace]
members = ["programs/fixed-array-args"]
resolver = "3"
"#,
    )?;
    write_file(
        &program_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "fixed-array-args"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[features]
idl-build = ["quasar-lang/idl-build"]

[dependencies]
quasar-lang = {{ path = "{}" }}
"#,
            workspace_root().join("lang").display()
        ),
    )?;
    write_file(
        &program_dir.join("src/lib.rs"),
        r#"#![no_std]
use quasar_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
mod fixed_array_args {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn submit(_ctx: Ctx<Submit>, payload_hash: [u8; 32]) -> Result<(), ProgramError> {
        let _ = payload_hash;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Submit {
    pub authority: Signer,
}
"#,
    )?;

    let clients_path = temp.path().join("clients");
    idl::generate(&program_dir, &["typescript"], &clients_path)?;
    let ts_dir = only_child_dir(&clients_path.join("typescript"))?;
    let kit = fs::read_to_string(ts_dir.join("kit.ts"))?;
    let web3 = fs::read_to_string(ts_dir.join("web3.ts"))?;

    for source in [&kit, &web3] {
        assert!(
            source.contains("fixCodecSize(getBytesCodec(), 32)"),
            "fixed [u8; 32] arg should use a fixed-size bytes codec"
        );
        assert!(
            !source.contains("/* unknown: bytes */"),
            "fixed [u8; 32] arg should not fall back to an unknown bytes codec"
        );
    }

    Ok(())
}

#[test]
fn generated_typescript_client_lowers_pod_vec_args_to_builtin_codecs() -> Result<(), Box<dyn Error>>
{
    let temp = tempdir()?;
    let program_dir = temp.path().join("programs/pod-vec-args");

    write_file(
        &temp.path().join("Cargo.toml"),
        r#"[workspace]
members = ["programs/pod-vec-args"]
resolver = "3"
"#,
    )?;
    write_file(
        &program_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "pod-vec-args"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[features]
idl-build = ["quasar-lang/idl-build"]

[dependencies]
quasar-lang = {{ path = "{}" }}
"#,
            workspace_root().join("lang").display()
        ),
    )?;
    write_file(
        &program_dir.join("src/lib.rs"),
        r#"#![no_std]
use quasar_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
mod pod_vec_args {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn submit(_ctx: Ctx<Submit>, nums: Vec<PodU64, 16>) -> Result<(), ProgramError> {
        let _ = nums;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Submit {
    pub authority: Signer,
}
"#,
    )?;

    let clients_path = temp.path().join("clients");
    idl::generate(&program_dir, &["typescript"], &clients_path)?;
    let ts_dir = only_child_dir(&clients_path.join("typescript"))?;
    let kit = read_file(&ts_dir.join("kit.ts"))?;

    assert!(
        kit.contains("nums: Array<bigint>;"),
        "PodU64 vec args should surface as bigint arrays"
    );
    assert!(
        kit.contains("getArrayCodec(getU64Codec(), { size: input.nums.length })"),
        "PodU64 vec args should encode through the builtin u64 codec"
    );
    assert!(
        !kit.contains("PodU64Codec"),
        "PodU64 vec args should not reference an undefined PodU64Codec"
    );
    assert!(
        !kit.contains("Array<PodU64>"),
        "PodU64 vec args should not expose undefined PodU64 types"
    );

    compile_typescript_client(&ts_dir)?;

    Ok(())
}

#[test]
fn kit_program_plugin_exposes_only_supported_accounts_and_instructions(
) -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let program_dir = temp.path().join("programs/kit-plugin-boundary");

    write_file(
        &temp.path().join("Cargo.toml"),
        r#"[workspace]
members = ["programs/kit-plugin-boundary"]
resolver = "3"
"#,
    )?;
    write_file(
        &program_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "kit-plugin-boundary"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[features]
idl-build = ["quasar-lang/idl-build"]

[dependencies]
quasar-lang = {{ path = "{}" }}
"#,
            workspace_root().join("lang").display()
        ),
    )?;
    write_file(
        &program_dir.join("src/lib.rs"),
        r#"#![no_std]
use quasar_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
mod kit_plugin_boundary {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn simple(_ctx: Ctx<Simple>) -> Result<(), ProgramError> {
        Ok(())
    }

    #[instruction(discriminator = 1)]
    pub fn resolver_heavy(_ctx: Ctx<ResolverHeavy>) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[account(discriminator = 1, set_inner)]
pub struct StaticAccount {
    pub authority: Address,
    pub count: u32,
}

#[account(discriminator = 2, set_inner)]
pub struct DynamicAccount {
    pub authority: Address,
    pub label: String<32>,
}

#[account(discriminator = 3, set_inner)]
#[seeds(b"config", authority: Address)]
pub struct NamespaceConfig {
    pub authority: Address,
    pub namespace: u32,
    pub bump: u8,
}

#[account(discriminator = 4, set_inner)]
#[seeds(b"scoped", namespace: u32)]
pub struct ScopedItem {
    pub namespace: u32,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct Simple {
    pub authority: Signer,
    #[account(mut)]
    pub state: Account<StaticAccount>,
}

#[derive(Accounts)]
pub struct ResolverHeavy {
    pub authority: Signer,
    #[account(address = NamespaceConfig::seeds(authority.address()))]
    pub config: Account<NamespaceConfig>,
    #[account(address = ScopedItem::seeds(config.namespace.into()))]
    pub scoped_item: Account<ScopedItem>,
}
"#,
    )?;

    let clients_path = temp.path().join("clients");
    idl::generate(&program_dir, &["typescript"], &clients_path)?;

    let ts_dir = only_child_dir(&clients_path.join("typescript"))?;
    let kit = read_file(&ts_dir.join("kit.ts"))?;

    assert!(
        kit.contains("from \"@solana/kit/program-client-core\""),
        "Kit client should import program plugin helpers"
    );
    assert!(
        kit.contains("export function kitPluginBoundaryProgram()"),
        "Kit client should expose a program plugin factory"
    );
    assert!(
        kit.contains("staticAccount: addSelfFetchFunctions(client, StaticAccountCodec),"),
        "static account codecs should be exposed through plugin account fetch helpers"
    );
    assert!(
        !kit.contains("dynamicAccount: addSelfFetchFunctions"),
        "dynamic account codecs should not be exposed through plugin account fetch helpers"
    );
    assert!(
        kit.contains(
            "simple: (input: SimpleInstructionInput) => addSelfPlanAndSendFunctions(client, \
             __client.createSimpleInstruction(input)),"
        ),
        "simple instructions should be exposed through plugin plan/send helpers"
    );
    assert!(
        !kit.contains("resolverHeavy:"),
        "instructions requiring AccountDataResolver should stay off the plugin surface"
    );

    compile_typescript_client(&ts_dir)?;

    Ok(())
}

#[test]
fn idl_lowers_typed_pda_seed_account_fields() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let program_dir = temp.path().join("programs/account-field-seeds");

    write_file(
        &temp.path().join("Cargo.toml"),
        r#"[workspace]
members = ["programs/account-field-seeds"]
resolver = "3"
"#,
    )?;
    write_file(
        &program_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "account-field-seeds"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[features]
idl-build = ["quasar-lang/idl-build"]

[dependencies]
quasar-lang = {{ path = "{}" }}
"#,
            workspace_root().join("lang").display()
        ),
    )?;
    write_file(
        &program_dir.join("src/lib.rs"),
        r#"#![no_std]
use quasar_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
mod account_field_seeds {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn use_scoped(_ctx: Ctx<UseScoped>) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[account(discriminator = 1, set_inner)]
#[seeds(b"config", authority: Address)]
pub struct NamespaceConfig {
    pub authority: Address,
    pub namespace: u32,
    pub bump: u8,
}

#[account(discriminator = 2, set_inner)]
#[seeds(b"scoped", namespace: u32)]
pub struct ScopedItem {
    pub namespace: u32,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct UseScoped {
    #[account(mut)]
    pub authority: Signer,
    #[account(address = NamespaceConfig::seeds(authority.address()))]
    pub config: Account<NamespaceConfig>,
    #[account(address = ScopedItem::seeds(config.namespace.into()))]
    pub scoped_item: Account<ScopedItem>,
}
"#,
    )?;

    let clients_path = temp.path().join("clients");
    idl::generate(
        &program_dir,
        &["typescript", "python", "golang", "c"],
        &clients_path,
    )?;

    let idl_json = read_file(&PathBuf::from("target/idl/account_field_seeds.json"))?;
    assert!(
        idl_json.contains(r#""kind": "pda""#),
        "typed seeds address constraints should be emitted as PDA resolvers: {idl_json}"
    );
    assert!(
        idl_json.contains(r#""kind": "accountField""#),
        "account field PDA seeds should be represented explicitly in the IDL: {idl_json}"
    );
    assert!(
        idl_json.contains(r#""account": "NamespaceConfig""#),
        "account field seed should include the source account type: {idl_json}"
    );
    assert!(
        idl_json.contains(r#""field": "namespace""#),
        "account field seed should include the source field path: {idl_json}"
    );

    let ts_dir = only_child_dir(&clients_path.join("typescript"))?;
    let kit = read_file(&ts_dir.join("kit.ts"))?;
    let web3 = read_file(&ts_dir.join("web3.ts"))?;

    for source in [&kit, &web3] {
        assert!(
            source.contains("scopedItem"),
            "generated client should still emit scoped item account handling"
        );
        assert!(
            !source.contains("scopedItem: Address;"),
            "account-field PDA account should be resolved instead of required as input"
        );
        assert!(
            !source.contains(" :: seeds("),
            "generated client should never stringify Rust seed expressions"
        );
    }

    compile_typescript_client(&ts_dir)?;
    compile_python_client(&only_child_dir(&clients_path.join("python"))?)?;
    compile_go_client(&only_child_dir(&clients_path.join("golang"))?)?;

    let c_header = read_file(&only_child_dir(&clients_path.join("c"))?.join("client.h"))?;
    assert!(
        c_header.contains("config_namespace_seed"),
        "C client should expose account-field PDA seeds as explicit bytes"
    );
    compile_c_client(&only_child_dir(&clients_path.join("c"))?)?;

    Ok(())
}

#[test]
fn generated_clients_encode_optional_dynamic_args_as_compact_tags_then_tails(
) -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let program_dir = temp.path().join("programs/optional-dynamic-args");

    write_file(
        &temp.path().join("Cargo.toml"),
        r#"[workspace]
members = ["programs/optional-dynamic-args"]
resolver = "3"
"#,
    )?;
    write_file(
        &program_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "optional-dynamic-args"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[features]
idl-build = ["quasar-lang/idl-build"]

[dependencies]
quasar-lang = {{ path = "{}" }}
"#,
            workspace_root().join("lang").display()
        ),
    )?;
    write_file(
        &program_dir.join("src/lib.rs"),
        r#"#![no_std]
use quasar_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
mod optional_dynamic_args {
    use super::*;

    #[instruction(discriminator = 7)]
    pub fn submit(
        _ctx: Ctx<Submit>,
        maybe_name: Option<String<32>>,
        maybe_addrs: Option<Vec<Address, 4>>,
    ) -> Result<(), ProgramError> {
        let _ = (maybe_name, maybe_addrs);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Submit {
    pub authority: Signer,
}
"#,
    )?;

    let clients_path = temp.path().join("clients");
    idl::generate(
        &program_dir,
        &["typescript", "python", "golang", "c"],
        &clients_path,
    )?;

    let rust_root = clients_path.join("rust");
    let rust_client_dir = fs::read_dir(&rust_root)?
        .next()
        .ok_or_else(|| format!("no rust client generated under `{}`", rust_root.display()))??
        .path();
    let rust_ix_path = rust_client_dir.join("src/instructions/submit.rs");
    let rust_ix = read_file(&rust_ix_path)?;
    assert!(rust_ix.contains("pub maybe_name: Option<DynString<u8>>"));
    assert!(rust_ix.contains("pub maybe_addrs: Option<DynVec<Address, u16>>"));
    assert!(rust_ix.contains("data.push(u8::from(ix.maybe_name.is_some()))"));
    assert!(rust_ix.contains("data.push(u8::from(ix.maybe_addrs.is_some()))"));
    override_rust_client_with_workspace_path(&rust_client_dir)?;
    compile_rust_client(&rust_client_dir)?;

    let ts_root = clients_path.join("typescript");
    let ts_dir = fs::read_dir(&ts_root)?
        .next()
        .ok_or_else(|| {
            format!(
                "no TypeScript client generated under `{}`",
                ts_root.display()
            )
        })??
        .path();
    for file in ["web3.ts", "kit.ts"] {
        let source = read_file(&ts_dir.join(file))?;
        assert!(source.contains(
            "const maybe_nameTag = getU8Codec().encode(input.maybe_name === null ? 0 : 1);"
        ));
        assert!(source.contains(
            "const maybe_addrsTag = getU8Codec().encode(input.maybe_addrs === null ? 0 : 1);"
        ));
        assert!(source.contains("...maybe_nameTag"));
        assert!(source.contains("...maybe_addrsTag"));
        assert!(source.contains("...maybe_nameBytes"));
        assert!(source.contains("...maybe_addrsBytes"));
    }

    let python_dir = only_child_dir(&clients_path.join("python"))?;
    let python_source = read_file(&python_dir.join("client.py"))?;
    assert!(python_source.contains("maybe_name: Optional[str]"));
    assert!(python_source.contains("maybe_addrs: Optional[list[Pubkey]]"));
    assert!(python_source.contains("data.append(0 if input.maybe_name is None else 1)"));
    assert!(python_source.contains("data.append(0 if input.maybe_addrs is None else 1)"));
    assert!(python_source.contains("if input.maybe_name is not None:"));
    assert!(python_source.contains("if input.maybe_addrs is not None:"));
    compile_python_client(&python_dir)?;

    let go_dir = only_child_dir(&clients_path.join("golang"))?;
    let go_source = read_file(&go_dir.join("client.go"))?;
    assert!(go_source.contains("MaybeName *string"));
    assert!(go_source.contains("MaybeAddrs *[]solana.PublicKey"));
    assert!(go_source.contains("if input.MaybeName == nil"));
    assert!(go_source.contains("if input.MaybeAddrs == nil"));
    assert!(go_source.contains("_MaybeNameBytes := []byte(*input.MaybeName)"));
    assert!(go_source.contains("for _, item := range *input.MaybeAddrs"));
    compile_go_client(&go_dir)?;

    let c_dir = only_child_dir(&clients_path.join("c"))?;
    let c_header = read_file(&c_dir.join("client.h"))?;
    assert!(c_header.contains("bool maybe_name_present;"));
    assert!(c_header.contains("const uint8_t *maybe_name;"));
    assert!(c_header.contains("bool maybe_addrs_present;"));
    assert!(c_header.contains("const Pubkey *maybe_addrs;"));
    assert!(c_header.contains("data_buf[off++] = args->maybe_name_present ? 1 : 0;"));
    assert!(c_header.contains("if (args->maybe_name_present)"));
    assert!(c_header.contains("data_buf[off++] = args->maybe_addrs_present ? 1 : 0;"));
    assert!(c_header.contains("if (args->maybe_addrs_present)"));
    compile_c_client(&c_dir)?;
    run_c_sanitized_test(
        &c_dir,
        r#"#include <assert.h>
#include <string.h>
#include "client.h"

int main(void) {
    Pubkey authority = { .bytes = {1} };
    Pubkey address = { .bytes = {2} };
    const uint8_t name[] = {'o', 'k'};
    optional_dynamic_args_submit_accounts_t accounts = { .authority = &authority };
    optional_dynamic_args_submit_args_t args = {
        .maybe_name_present = true,
        .maybe_name = name,
        .maybe_name_len = 2,
        .maybe_addrs_present = true,
        .maybe_addrs = &address,
        .maybe_addrs_len = 1,
    };
    Instruction ix;
    Instruction ix_before;
    AccountMeta meta;
    AccountMeta meta_before;
    uint8_t data[40];
    uint8_t data_before[40];

    memset(&ix, 0xa5, sizeof(ix));
    memset(&meta, 0xa5, sizeof(meta));
    memset(data, 0xa5, sizeof(data));
    memcpy(&ix_before, &ix, sizeof(ix));
    memcpy(&meta_before, &meta, sizeof(meta));
    memcpy(data_before, data, sizeof(data));

    optional_dynamic_args_ix_result_t result = optional_dynamic_args_submit_ix(
        &accounts, &args, &ix, &meta, 1, data, sizeof(data) - 1);
    assert(result.status == OPTIONAL_DYNAMIC_ARGS_IX_DATA_BUFFER_TOO_SMALL);
    assert(result.accounts_len == 1 && result.data_len == sizeof(data));
    assert(memcmp(&ix, &ix_before, sizeof(ix)) == 0);
    assert(memcmp(&meta, &meta_before, sizeof(meta)) == 0);
    assert(memcmp(data, data_before, sizeof(data)) == 0);

    result = optional_dynamic_args_submit_ix(
        &accounts, &args, &ix, &meta, 1, data, sizeof(data));
    assert(result.status == OPTIONAL_DYNAMIC_ARGS_IX_OK);
    assert(result.accounts_len == 1 && result.data_len == sizeof(data));
    assert(ix.accounts == &meta && ix.accounts_len == 1);
    assert(ix.data == data && ix.data_len == sizeof(data));
    assert(data[0] == 7 && data[1] == 1 && data[2] == 1);
    assert(data[3] == 2 && data[4] == 'o' && data[5] == 'k');
    assert(data[6] == 1 && data[7] == 0);
    assert(memcmp(&data[8], address.bytes, 32) == 0);
    return 0;
}
"#,
    )?;

    Ok(())
}

#[test]
fn generated_clients_render_optional_accounts_with_program_id_sentinel(
) -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let program_dir = temp.path().join("programs/optional-accounts");

    write_file(
        &temp.path().join("Cargo.toml"),
        r#"[workspace]
members = ["programs/optional-accounts"]
resolver = "3"
"#,
    )?;
    write_file(
        &program_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "optional-accounts"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[features]
idl-build = ["quasar-lang/idl-build"]

[dependencies]
quasar-lang = {{ path = "{}" }}
"#,
            workspace_root().join("lang").display()
        ),
    )?;
    write_file(
        &program_dir.join("src/lib.rs"),
        r#"#![no_std]
use quasar_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account(discriminator = 1, set_inner)]
#[seeds(b"thing", owner: Address)]
pub struct Thing {
    pub owner: Address,
    pub value: u64,
}

#[program]
mod optional_accounts {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn touch(_ctx: Ctx<Touch>, _value: u64) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Touch {
    #[account(mut)]
    pub authority: Signer,
    #[account(mut)]
    pub required: Account<Thing>,
    #[account(mut, address = Thing::seeds(authority.address()))]
    pub maybe: Option<Account<Thing>>,
}
"#,
    )?;

    let clients_path = temp.path().join("clients");
    idl::generate(
        &program_dir,
        &["typescript", "python", "golang", "c"],
        &clients_path,
    )?;

    // Rust: optional account is an `Option<Address>` input; an absent (None)
    // slot is encoded as the program id sentinel (`ID`) while a present slot
    // passes the provided address through.
    let rust_root = clients_path.join("rust");
    let rust_client_dir = fs::read_dir(&rust_root)?
        .next()
        .ok_or_else(|| format!("no rust client generated under `{}`", rust_root.display()))??
        .path();
    let rust_ix = read_file(&rust_client_dir.join("src/instructions/touch.rs"))?;
    assert!(
        rust_ix.contains("pub maybe: Option<Address>"),
        "optional account should be an Option<Address> input"
    );
    assert!(
        rust_ix.contains("pub required: Address,"),
        "required account should stay a plain Address input"
    );
    assert!(
        rust_ix.contains("AccountMeta::new(ix.maybe.unwrap_or(ID), false)"),
        "absent optional account should default to the program id sentinel; present passes through"
    );
    override_rust_client_with_workspace_path(&rust_client_dir)?;
    compile_rust_client(&rust_client_dir)?;

    // TypeScript (web3.js + kit): optional account is an optional `?` input;
    // absent defaults to the program address, present passes through.
    let ts_dir = only_child_dir(&clients_path.join("typescript"))?;
    let web3 = read_file(&ts_dir.join("web3.ts"))?;
    let kit = read_file(&ts_dir.join("kit.ts"))?;
    for source in [&web3, &kit] {
        assert!(
            source.contains("  maybe?: Address;"),
            "optional account should be an optional TS input"
        );
        assert!(
            source.contains("  required: Address;"),
            "required account should stay a mandatory TS input"
        );
        assert!(
            !source.contains("accountsMap[\"maybe\"] ="),
            "optional resolved accounts must remain caller-controlled"
        );
    }
    assert!(
        web3.contains("pubkey: (input.maybe ?? "),
        "web3.js should default an absent optional account to the program id"
    );
    assert!(
        web3.contains("programId), isSigner: false, isWritable: true }"),
        "web3.js optional sentinel should use the program id with declared flags"
    );
    assert!(
        kit.contains("address: (input.maybe ?? PROGRAM_ADDRESS), role:"),
        "kit should default an absent optional account to the program address"
    );
    compile_typescript_client(&ts_dir)?;

    // Python / Go / C: optional inputs are None-default / pointer / nullable
    // pointer respectively, each defaulting an absent slot to the program id.
    let py_dir = only_child_dir(&clients_path.join("python"))?;
    let py = read_file(&py_dir.join("client.py"))?;
    assert!(py.contains("from typing import Optional"));
    assert!(py.contains("value: int\n    maybe: Optional[Pubkey] = None"));
    assert!(py.contains("maybe: Optional[Pubkey] = None"));
    assert!(py.contains("input.maybe if input.maybe is not None else PROGRAM_ID"));
    assert!(!py.contains("accounts_map[\"maybe\"] = Pubkey.find_program_address"));
    compile_python_client(&py_dir)?;

    let go_dir = only_child_dir(&clients_path.join("golang"))?;
    let go = read_file(&go_dir.join("client.go"))?;
    assert!(go.contains("Maybe *solana.PublicKey"));
    assert!(go.contains("if input.Maybe != nil { return *input.Maybe }; return ProgramID"));
    compile_go_client(&go_dir)?;

    let c_dir = only_child_dir(&clients_path.join("c"))?;
    let c = read_file(&c_dir.join("client.h"))?;
    assert!(c.contains("accounts->maybe ? accounts->maybe : (Pubkey *)&"));
    compile_c_client(&c_dir)?;

    Ok(())
}
