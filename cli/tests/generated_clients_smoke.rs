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

fn compile_python_client(client_dir: &Path) -> Result<(), Box<dyn Error>> {
    run_command(
        Command::new("python3")
            .arg("-m")
            .arg("py_compile")
            .arg("__init__.py")
            .arg("client.py")
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

fn compile_c_client(client_dir: &Path) -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    write_file(
        &temp.path().join("caravel.h"),
        r#"#ifndef CARAVEL_H
#define CARAVEL_H

#include <stdbool.h>
#include <stdint.h>

typedef struct {
    uint8_t bytes[32];
} Pubkey;

typedef struct {
    Pubkey *pubkey;
    bool is_signer;
    bool is_writable;
} AccountMeta;

typedef struct {
    Pubkey *program_id;
    AccountMeta *accounts;
    uint64_t accounts_len;
    uint8_t *data;
    uint64_t data_len;
} Instruction;

typedef struct {
    const uint8_t *addr;
    uint64_t len;
} SignerSeed;

static inline AccountMeta meta_readonly(Pubkey *pubkey) {
    return (AccountMeta){ .pubkey = pubkey, .is_signer = false, .is_writable = false };
}

static inline AccountMeta meta_readonly_signer(Pubkey *pubkey) {
    return (AccountMeta){ .pubkey = pubkey, .is_signer = true, .is_writable = false };
}

static inline AccountMeta meta_writable(Pubkey *pubkey) {
    return (AccountMeta){ .pubkey = pubkey, .is_signer = false, .is_writable = true };
}

static inline AccountMeta meta_writable_signer(Pubkey *pubkey) {
    return (AccountMeta){ .pubkey = pubkey, .is_signer = true, .is_writable = true };
}

static inline bool find_program_address(
    const SignerSeed *seeds,
    uint64_t seeds_len,
    Pubkey *program_id,
    Pubkey *out,
    uint8_t *bump
) {
    (void)seeds;
    (void)seeds_len;
    (void)program_id;
    *out = (Pubkey){ .bytes = {0} };
    *bump = 255;
    return true;
}

#endif
"#,
    )?;
    write_file(&temp.path().join("compile.c"), r#"#include "client.h""#)?;

    let cc = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
    run_command(
        Command::new(cc)
            .arg("-std=c11")
            .arg("-fsyntax-only")
            .arg("-I")
            .arg(client_dir)
            .arg("-I")
            .arg(temp.path())
            .arg(temp.path().join("compile.c")),
    )
}

fn compile_typescript_client(client_dir: &Path) -> Result<(), Box<dyn Error>> {
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
            .arg("typescript")
            .arg("@types/node")
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
        assert!(
            source.contains("  vault: Address;"),
            "generated TypeScript client should require the vault account"
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
        &clients_path
            .join("rust")
            .join("lifecycle_client_flags-client")
            .join("src"),
        "rs",
    )?;
    assert!(rust_ix.contains("AccountMeta::new(ix.config, false)"));
    assert!(rust_ix.contains("AccountMeta::new(ix.vault, false)"));

    let ts_web3 = read_file(
        &clients_path
            .join("typescript")
            .join("lifecycle_client_flags")
            .join("web3.ts"),
    )?;
    assert!(ts_web3.contains("{ pubkey: input.config, isSigner: false, isWritable: true },"));
    assert!(ts_web3.contains("{ pubkey: input.vault, isSigner: false, isWritable: true },"));

    let py_client = read_file(
        &clients_path
            .join("python")
            .join("lifecycle_client_flags")
            .join("client.py"),
    )?;
    assert!(py_client.contains(
        r#"accounts.append(AccountMeta(accounts_map["config"], is_signer=False, is_writable=True))"#
    ));
    assert!(py_client.contains(
        r#"accounts.append(AccountMeta(accounts_map["vault"], is_signer=False, is_writable=True))"#
    ));

    let go_client = read_file(
        &clients_path
            .join("golang")
            .join("lifecycle_client_flags")
            .join("client.go"),
    )?;
    assert!(go_client
        .contains(r#"accounts = append(accounts, solana.Meta(accountsMap["config"]).WRITE())"#));
    assert!(go_client
        .contains(r#"accounts = append(accounts, solana.Meta(accountsMap["vault"]).WRITE())"#));

    let c_client = read_file(
        &clients_path
            .join("c")
            .join("lifecycle_client_flags")
            .join("client.h"),
    )?;
    assert!(c_client.contains("meta_buf[2] = meta_writable(accounts->config);"));
    assert!(c_client.contains("meta_buf[3] = meta_writable(accounts->vault);"));
    compile_c_client(&clients_path.join("c").join("lifecycle_client_flags"))?;

    Ok(())
}

#[test]
fn generated_clients_compile_from_fresh_project() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_program();

    let temp = tempdir()?;
    let clients_path = temp.path().join("clients");
    idl::generate(&fixture, &["typescript", "python", "golang"], &clients_path)?;

    // The IDL is generated relative to the workspace; find the rust client dir
    // by convention.
    let rust_client_dir = clients_path.join("rust").join("quasar-multisig-client");
    if rust_client_dir.exists() {
        // Patch the generated Cargo.toml to use the local workspace `quasar-lang`
        // instead of the GitHub remote, so the smoke test validates against the
        // current (possibly unreleased) source.
        let cargo_toml_path = rust_client_dir.join("Cargo.toml");
        let cargo_toml = fs::read_to_string(&cargo_toml_path)?;
        let patched = cargo_toml.replace(
            "quasar-lang = { git = \"https://github.com/blueshift-gg/quasar\", branch = \
             \"master\" }",
            &format!(
                "quasar-lang = {{ path = \"{}\" }}",
                workspace_root().join("lang").display()
            ),
        );
        fs::write(&cargo_toml_path, &patched)?;
        compile_rust_client(&rust_client_dir)?;
    }

    let ts_dir = clients_path.join("typescript").join("quasar-multisig");
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

    let py_dir = clients_path.join("python").join("quasar-multisig");
    if py_dir.exists() {
        compile_python_client(&py_dir)?;
    }

    let go_dir = clients_path.join("golang").join("quasar_multisig");
    if go_dir.exists() {
        compile_go_client(&go_dir)?;
    }

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
    let ts_dir = clients_path.join("typescript").join("fixed_array_args");
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
    let ts_dir = clients_path.join("typescript").join("pod_vec_args");
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

    let ts_dir = clients_path.join("typescript").join("kit_plugin_boundary");
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

    let ts_dir = clients_path.join("typescript").join("account_field_seeds");
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
    let cargo_toml_path = rust_client_dir.join("Cargo.toml");
    let cargo_toml = read_file(&cargo_toml_path)?;
    let patched = cargo_toml.replace(
        "quasar-lang = { git = \"https://github.com/blueshift-gg/quasar\", branch = \"master\" }",
        &format!(
            "quasar-lang = {{ path = \"{}\" }}",
            workspace_root().join("lang").display()
        ),
    );
    fs::write(&cargo_toml_path, patched + "\n[workspace]\n")?;
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

    Ok(())
}
