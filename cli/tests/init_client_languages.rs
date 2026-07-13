use {
    std::{
        error::Error,
        fs,
        process::{Command, Output},
    },
    tempfile::tempdir,
};

fn assert_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn generated_config_only_lists_additional_client_languages() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let home = temp.path().join("home");
    fs::create_dir(&home)?;

    for (test_language, expected) in [
        ("none", Vec::<&str>::new()),
        ("rust", Vec::new()),
        ("typescript", vec!["typescript"]),
    ] {
        let name = format!("clients-{test_language}");
        let init = Command::new(env!("CARGO_BIN_EXE_quasar"))
            .arg("init")
            .arg(&name)
            .arg("--yes")
            .arg("--no-git")
            .arg("--test-language")
            .arg(test_language)
            .arg("--template")
            .arg("minimal")
            .arg("--toolchain")
            .arg("solana")
            .env("HOME", &home)
            .current_dir(temp.path())
            .output()?;
        assert_success(
            &format!("quasar init --test-language {test_language}"),
            &init,
        );

        let config = fs::read_to_string(temp.path().join(name).join("Quasar.toml"))?;
        let config: toml::Value = toml::from_str(&config)?;
        let languages = config["clients"]["languages"]
            .as_array()
            .expect("clients.languages should be an array")
            .iter()
            .map(|language| language.as_str().expect("language should be a string"))
            .collect::<Vec<_>>();
        assert_eq!(languages, expected, "test language: {test_language}");
    }

    Ok(())
}
