use std::process::Command;

/// Check whether sbpf-linker is reachable on PATH.
pub fn has_sbpf_linker() -> bool {
    Command::new("sbpf-linker")
        .arg("--version")
        .output()
        .ok()
        .is_some_and(|o| o.status.success())
}

/// Detect the LLVM major version that sbpf-linker is linked against.
pub fn sbpf_linker_llvm_version() -> Option<u32> {
    // Try llvm-config (Homebrew / system LLVM)
    if let Some(v) = llvm_config_version("llvm-config") {
        return Some(v);
    }

    // Try Homebrew LLVM specifically
    if let Some(v) = llvm_config_version("/opt/homebrew/opt/llvm/bin/llvm-config") {
        return Some(v);
    }

    // Try platform-tools LLVM
    if let Some(home) = dirs::home_dir() {
        let platform_tools = home.join(".cache/solana");
        if platform_tools.exists() {
            if let Ok(entries) = std::fs::read_dir(&platform_tools) {
                for entry in entries.flatten() {
                    let llvm_config = entry.path().join("platform-tools/llvm/bin/llvm-config");
                    if llvm_config.exists() {
                        if let Some(v) =
                            llvm_config_version(llvm_config.to_str().unwrap_or_default())
                        {
                            return Some(v);
                        }
                    }
                }
            }
        }
    }

    // Last resort: map known sbpf-linker versions to LLVM versions
    let output = Command::new("sbpf-linker").arg("--version").output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    if text.contains("0.1.") {
        return Some(21);
    }

    None
}

/// Get the LLVM major version used by a specific Rust toolchain channel.
pub fn rustc_llvm_version(channel: &str) -> Option<u32> {
    let output = Command::new("rustc")
        .args([&format!("+{channel}"), "--version", "--verbose"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if let Some(ver) = line.strip_prefix("LLVM version: ") {
            return parse_llvm_major(ver);
        }
    }
    None
}

fn llvm_config_version(cmd: &str) -> Option<u32> {
    let output = Command::new(cmd).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_llvm_major(&text)
}

fn parse_llvm_major(text: &str) -> Option<u32> {
    for word in text.split_whitespace() {
        if let Some(major_str) = word.split('.').next() {
            if let Ok(n) = major_str.parse::<u32>() {
                if (10..=30).contains(&n) {
                    return Some(n);
                }
            }
        }
    }
    None
}
