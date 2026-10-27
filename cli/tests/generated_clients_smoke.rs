use {
    quasar_cli::idl,
    std::{
        error::Error,
        fs,
        path::{Path, PathBuf},
        process::Command,
    },
    tempfile::tempdir,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root").to_path_buf()
}

fn fixture_program() -> PathBuf {
    workspace_root().join("examples/multisig")
}

fn run_command(cmd: &mut Command) -> Result<(), Box<dyn Error>> {
    let output = cmd.output().map_err(|error| format!("could not run {cmd:?}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }

    let mut message = String::new();
    message.push_str(&format!("command failed: {cmd:?}\n"));
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
    run_command(Command::new("cargo").arg("check").arg("--quiet").current_dir(client_dir))
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
    run_command(Command::new("go").arg("mod").arg("tidy").current_dir(client_dir))?;
    run_command(Command::new("go").arg("build").arg("./...").current_dir(client_dir))
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
        Command::new("npx").arg("tsc").arg("-p").arg("tsconfig.json").current_dir(client_dir),
    )
}

fn write_file(path: &Path, contents: impl AsRef<str>) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents.as_ref())?;
    Ok(())
}

#[test]
fn generated_clients_compile_from_rewrite_multisig() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let clients = temp.path().join("clients");
    let interface =
        idl::generate(&fixture_program(), &["typescript", "python", "golang", "c"], &clients)?;
    assert_eq!(interface.instructions[0].discriminator, [0]);
    assert!(interface.instructions[0].remaining_accounts.is_some());
    let rust = clients.join("rust/multisig-client");
    assert!(rust.exists());
    compile_rust_client(&rust)?;
    let manifest = rust.join("Cargo.toml");
    let mut source = fs::read_to_string(&manifest)?;
    source.push_str(&format!(
        "\n[dev-dependencies]\nmultisig = {{ path = {:?} }}\nquasar-lang = {{ path = {:?} }}\n",
        fixture_program(),
        workspace_root().join("lang")
    ));
    fs::write(manifest, source)?;
    write_file(&rust.join("tests/wire.rs"), include_str!("fixtures/generated-wire.rs"))?;
    run_command(Command::new("cargo").arg("test").arg("--quiet").current_dir(&rust))?;
    for target in ["web3.ts", "kit.ts"] {
        let source = fs::read_to_string(clients.join("typescript/multisig").join(target))?;
        assert!(source.contains("  config: Address;"));
        assert!(source.contains("  vault: Address;"));
        assert!(!source.contains(" :: seeds("));
        if target == "kit.ts" {
            assert!(source.contains("export function multisigProgram()"));
            assert!(source.contains("addSelfPlanAndSendFunctions"));
        }
    }
    compile_typescript_client(&clients.join("typescript/multisig"))?;
    compile_python_client(&clients.join("python/multisig"))?;
    compile_go_client(&clients.join("golang/multisig"))?;
    compile_c_client(&clients.join("c/multisig"))?;
    Ok(())
}

#[test]
fn lifecycle_accounts_keep_writable_flags_through_idl_and_all_clients() -> Result<(), Box<dyn Error>>
{
    let temp = tempdir()?;
    let program = temp.path().join("program");
    write_file(
        &program.join("Cargo.toml"),
        format!(
            "[package]\nname = \"client_lifecycle\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\nquasar-lang = {{ path = {:?} }}\n",
            workspace_root().join("lang"),
        ),
    )?;
    write_file(&program.join("src/lib.rs"), include_str!("fixtures/client-lifecycle.rs"))?;
    let clients = temp.path().join("clients");
    let interface = idl::generate(&program, &["typescript", "python", "golang", "c"], &clients)?;
    let ix = &interface.instructions[0];
    assert_eq!(ix.name, "touch");
    assert_eq!(
        ix.accounts.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
        ["payer", "system_program", "config", "vault"]
    );
    for name in ["config", "vault"] {
        let meta = ix.accounts.iter().find(|a| a.name == name).unwrap();
        assert!(meta.writable.is_true(), "{name} must be writable");
        assert!(meta.signer.is_false(), "{name} must not require a signature");
    }
    assert_eq!(
        ix.args[0].ty,
        quasar_idl::types::IdlType::Array {
            array: (Box::new(quasar_idl::types::IdlType::Primitive("u8".into())), 32)
        }
    );
    let vault = interface.types.iter().find(|ty| ty.name == "Vault").unwrap();
    assert_eq!(
        vault.fields.iter().find(|field| field.name == "nums").unwrap().ty,
        quasar_idl::types::IdlType::Vec {
            vec: Box::new(quasar_idl::types::IdlType::Primitive("u64".into()))
        }
    );
    let rust =
        fs::read_to_string(clients.join("rust/client_lifecycle-client/src/instructions/touch.rs"))?;
    let web3 = fs::read_to_string(clients.join("typescript/client_lifecycle/web3.ts"))?;
    let kit = fs::read_to_string(clients.join("typescript/client_lifecycle/kit.ts"))?;
    let python = fs::read_to_string(clients.join("python/client_lifecycle/client.py"))?;
    let go = fs::read_to_string(clients.join("golang/client_lifecycle/client.go"))?;
    let c = fs::read_to_string(clients.join("c/client_lifecycle/client.h"))?;
    for (index, name) in [(2, "config"), (3, "vault")] {
        assert!(rust.contains(&format!("AccountMeta::new(ix.{name}, false)")));
        assert!(
            web3.contains(&format!(
                "{{ pubkey: input.{name}, isSigner: false, isWritable: true }},"
            ))
        );
        assert!(python.contains(&format!(
            "AccountMeta(accounts_map[\"{name}\"], is_signer=False, is_writable=True)"
        )));
        assert!(go.contains(&format!("solana.Meta(accountsMap[\"{name}\"]).WRITE()")));
        assert!(c.contains(&format!("meta_buf[{index}] = meta_writable(accounts->{name});")));
    }
    for source in [&web3, &kit] {
        assert!(source.contains("fixCodecSize(getBytesCodec(), 32)"));
        assert!(source.contains("nums: Array<bigint>;"));
        assert!(source.contains("getArrayCodec(getU64Codec(), { size: getU32Codec() })"));
    }
    compile_c_client(&clients.join("c/client_lifecycle"))?;
    Ok(())
}

#[test]
fn public_schema_regression_clients_compile() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let interface =
        quasar_idl::read(include_str!("../../idl/tests/fixtures/client-regressions.json"))?;
    let clients = temp.path();
    quasar_cli::client::generate_clients(
        &interface,
        &["typescript", "python", "golang", "c"],
        clients,
    )?;
    let rust = clients.join("rust");
    write_file(
        &rust.join("Cargo.toml"),
        quasar_idl::codegen::rust::generate_cargo_toml("client_regressions", "0.1.0", true),
    )?;
    for (path, source) in quasar_idl::codegen::rust::generate_client(&interface) {
        write_file(&rust.join("src").join(path), source)?;
    }
    write_file(&rust.join("tests/compact.rs"), include_str!("fixtures/client-compact-wire.rs"))?;
    run_command(Command::new("cargo").arg("test").arg("--quiet").current_dir(&rust))?;
    compile_typescript_client(&clients.join("typescript/client_regressions"))?;
    compile_python_client(&clients.join("python/client_regressions"))?;
    compile_go_client(&clients.join("golang/client_regressions"))?;
    compile_c_client(&clients.join("c/client_regressions"))?;
    Ok(())
}
