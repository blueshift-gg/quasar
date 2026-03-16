use std::sync::atomic::{AtomicBool, Ordering};

static COLOR_ENABLED: AtomicBool = AtomicBool::new(true);

/// Initialize style settings from global config. Call once at startup.
pub fn init(color: bool) {
    COLOR_ENABLED.store(color, Ordering::Relaxed);
}

fn use_color() -> bool {
    COLOR_ENABLED.load(Ordering::Relaxed)
}

/// Colored checkmark (green).
pub fn success(msg: &str) -> String {
    if use_color() {
        format!("\x1b[38;5;83m\u{2714}\x1b[0m {msg}")
    } else {
        format!("[ok] {msg}")
    }
}

/// Colored X (red).
pub fn fail(msg: &str) -> String {
    if use_color() {
        format!("\x1b[38;5;196m\u{2718}\x1b[0m {msg}")
    } else {
        format!("[error] {msg}")
    }
}

/// Colored arrow (cyan) for in-progress steps.
pub fn step(msg: &str) -> String {
    if use_color() {
        format!("\x1b[38;5;45m\u{279c}\x1b[0m {msg}")
    } else {
        format!("[..] {msg}")
    }
}

/// Warning triangle (yellow).
pub fn warn(msg: &str) -> String {
    if use_color() {
        format!("\x1b[38;5;208m\u{26a0}\x1b[0m {msg}")
    } else {
        format!("[warn] {msg}")
    }
}

/// Bold text.
pub fn bold(s: &str) -> String {
    if use_color() {
        format!("\x1b[1m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

/// Dim text.
pub fn dim(s: &str) -> String {
    if use_color() {
        format!("\x1b[2m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

/// 256-color text.
pub fn color(code: u8, s: &str) -> String {
    if use_color() {
        format!("\x1b[38;5;{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

/// Format a byte size in a human-readable way.
pub fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Create a space-themed cyan spinner with a message.
pub fn spinner(msg: &str) -> indicatif::ProgressBar {
    use {
        indicatif::{ProgressBar, ProgressStyle},
        std::time::Duration,
    };

    let sp = ProgressBar::new_spinner();
    sp.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&[
                "✦ ·  · ",
                " ✦ ·  ·",
                "·  ✦ · ",
                " ·  ✦ ·",
                "·  · ✦ ",
                " ·  · ✦",
                "·  · ✦ ",
                " ·  ✦ ·",
                "·  ✦ · ",
                " ✦ ·  ·",
                "✦ ·  · ",
            ])
            .template("  {spinner:.cyan} {msg}")
            .unwrap(),
    );
    sp.enable_steady_tick(Duration::from_millis(80));
    sp.set_message(msg.to_string());
    sp
}

/// Format a duration in a human-readable way.
pub fn human_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs_f64();
    if secs < 0.1 {
        format!("{:.0}ms", secs * 1000.0)
    } else if secs < 60.0 {
        format!("{secs:.1}s")
    } else {
        let mins = secs as u64 / 60;
        let rem = secs as u64 % 60;
        format!("{mins}m {rem}s")
    }
}
