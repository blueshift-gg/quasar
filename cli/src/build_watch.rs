use {
    crate::{error::CliResult, style},
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

pub(crate) fn watch_loop<F>(mut run_once: F) -> !
where
    F: FnMut() -> CliResult,
{
    if let Err(e) = run_once() {
        eprintln!("  {}", style::fail(&format!("{e}")));
    }

    loop {
        let baseline = collect_watch_mtimes();
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let current = collect_watch_mtimes();
            if current != baseline {
                if let Err(e) = run_once() {
                    eprintln!("  {}", style::fail(&format!("{e}")));
                }
                break;
            }
        }
    }
}

fn collect_watch_mtimes() -> Vec<(PathBuf, std::time::SystemTime)> {
    const WATCH_FILES: &[&str] = &["Cargo.toml", "Cargo.lock", "Quasar.toml", ".cargo/config.toml"];
    const WATCH_DIRS: &[&str] = &["src", "tests"];

    let mut times = Vec::new();

    for path in WATCH_FILES.iter().map(Path::new) {
        collect_watch_path_mtimes(path, &mut times);
    }
    for path in WATCH_DIRS.iter().map(Path::new) {
        collect_watch_path_mtimes(path, &mut times);
    }

    times.sort_by(|a, b| a.0.cmp(&b.0));
    times
}

fn collect_watch_path_mtimes(path: &Path, times: &mut Vec<(PathBuf, std::time::SystemTime)>) {
    let Ok(meta) = fs::metadata(path) else {
        return;
    };

    if meta.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                collect_watch_path_mtimes(&entry.path(), times);
            }
        }
        return;
    }

    if let Ok(mtime) = meta.modified() {
        times.push((path.to_path_buf(), mtime));
    }
}
