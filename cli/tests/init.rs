use {
    std::{fs, process::Command},
    tempfile::tempdir,
};

#[test]
fn rejects_keyword_names_before_writing_project_files() {
    let temp = tempdir().unwrap();
    for name in ["type", "async", "try", "gen"] {
        let current_dir = temp.path().join(name);
        fs::create_dir(&current_dir).unwrap();

        for target in [name, "."] {
            let output = Command::new(env!("CARGO_BIN_EXE_quasar"))
                .args(["init", target, "--yes", "--no-git"])
                .current_dir(&current_dir)
                .output()
                .unwrap();

            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "init {target} in {name} must fail");
            assert!(
                stderr.contains(&format!("invalid project name: \"{name}\"")),
                "init {target} in {name}: {stderr}"
            );
            assert!(fs::read_dir(&current_dir).unwrap().next().is_none());
        }
    }
}
