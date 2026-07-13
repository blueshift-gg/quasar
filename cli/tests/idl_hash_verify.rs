use {
    quasar_idl::types::{
        canonical_json_pretty, compute_abi_hash, compute_idl_hash, Idl, IdlHashes,
    },
    std::{fs, process::Command},
    tempfile::tempdir,
};

fn extension_idl() -> Idl {
    serde_json::from_str(
        r#"{
            "spec": "quasar-idl/1.1.0",
            "name": "verify_extension",
            "version": "0.1.0",
            "address": "11111111111111111111111111111111",
            "extensions": {
                "vendor.example": {
                    "enabled": true,
                    "limits": { "daily": 7, "monthly": 30 }
                }
            }
        }"#,
    )
    .unwrap()
}

fn run_verify(path: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_quasar"))
        .arg("idl")
        .arg("verify")
        .arg(path)
        .output()
        .unwrap()
}

#[test]
fn verify_preserves_extensions_and_detects_their_mutation() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("extension.idl.json");
    let mut idl = extension_idl();
    idl.hashes = Some(IdlHashes {
        idl: compute_idl_hash(&idl),
        abi: compute_abi_hash(&idl),
    });
    fs::write(&path, canonical_json_pretty(&idl).unwrap()).unwrap();

    let valid = run_verify(&path);
    assert!(
        valid.status.success(),
        "extension-bearing IDL should verify\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&valid.stdout),
        String::from_utf8_lossy(&valid.stderr)
    );
    assert!(
        String::from_utf8_lossy(&valid.stdout).contains("IDL hashes verified"),
        "successful verification should report the checked IDL"
    );

    idl.extensions.as_mut().unwrap()["vendor.example"]["limits"]["daily"] = serde_json::json!(8);
    fs::write(&path, canonical_json_pretty(&idl).unwrap()).unwrap();

    let mutated = run_verify(&path);
    assert!(!mutated.status.success(), "mutated extension must fail");
    let stderr = String::from_utf8_lossy(&mutated.stderr);
    assert!(stderr.contains("IDL hash mismatch"), "{stderr}");
    assert!(stderr.contains("  idl: stored"), "{stderr}");
    assert!(
        !stderr.contains("  abi: stored"),
        "opaque extensions are inside the full hash but outside the ABI hash: {stderr}"
    );
}
