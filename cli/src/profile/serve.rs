//! Local preview server for profiler reports.

use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

/// Check if something is already listening on the given port.
pub(crate) fn is_alive(port: u16) -> bool {
    let Ok(addr) = format!("127.0.0.1:{port}").parse() else {
        return false;
    };

    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}

/// Start a background HTTP server that serves static files from `root`.
///
/// Forks a child process that serves files and auto-shuts down after
/// `idle_secs` of inactivity (no new connections = browser tab closed).
///
/// Returns the URL on success.
pub(crate) fn serve_background(root: &Path, port: u16, program: &str) -> std::io::Result<String> {
    // Try to bind first to fail fast if port is busy
    let listener = TcpListener::bind(format!("127.0.0.1:{port}"))?;
    let url = format!("http://127.0.0.1:{port}/?program={program}");

    let root = root.to_path_buf();

    // SAFETY: no Rust threads are started before this call; both branches
    // immediately restrict themselves to process-safe listener ownership, and
    // the child detaches before entering the blocking server loop.
    unsafe {
        let pid = libc::fork();
        if pid < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if pid > 0 {
            // Parent drops its listener copy and returns.
            drop(listener);
            return Ok(url);
        }
        // Child detaches from the parent's process group.
        libc::setsid();
    }

    // Child process: serve until idle
    run_server(listener, &root, 30);
    std::process::exit(0);
}

/// Blocking serve (for --diff mode which needs to stay alive until Ctrl-C).
pub(crate) fn serve_blocking(root: &Path, port: u16) -> std::io::Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{port}"))?;
    let root = root.to_path_buf();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let root = root.clone();
                thread::spawn(move || handle_connection(stream, &root));
            }
            Err(e) => {
                eprintln!("Connection error: {e}");
            }
        }
    }
    Ok(())
}

/// Run the server loop. Shuts down after `idle_secs` of no connections.
fn run_server(listener: TcpListener, root: &Path, idle_secs: u64) {
    listener.set_nonblocking(true).ok();
    let idle_timeout = Duration::from_secs(idle_secs);
    let mut last_conn = Instant::now();
    let mut had_connection = false;

    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                last_conn = Instant::now();
                had_connection = true;
                let root = root.to_path_buf();
                thread::spawn(move || handle_connection(stream, &root));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Idle check: if we've had at least one connection and
                // no new ones for idle_timeout, shut down
                if had_connection && last_conn.elapsed() > idle_timeout {
                    return;
                }
                // If no connection ever came, give up after 60s
                if !had_connection && last_conn.elapsed() > Duration::from_secs(60) {
                    return;
                }
                thread::sleep(Duration::from_millis(200));
            }
            Err(_) => return,
        }
    }
}

fn handle_connection(mut stream: TcpStream, root: &Path) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));

    let buf_reader = BufReader::new(&stream);
    let request_line = match buf_reader.lines().next() {
        Some(Ok(line)) => line,
        _ => return,
    };

    // Parse "GET /path HTTP/1.1"
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_string();

    // Strip query string
    let path = path.split('?').next().unwrap_or("/");

    match resolve_request_path(root, path) {
        Some(file_path) => serve_file(&mut stream, &file_path),
        None => serve_not_found(&mut stream),
    }
}

fn resolve_request_path(root: &Path, request_path: &str) -> Option<PathBuf> {
    let raw_path = request_path.strip_prefix('/').unwrap_or(request_path);
    if raw_path.is_empty() || raw_path == "index.html" {
        return Some(root.join("index.html"));
    }

    let mut relative = PathBuf::new();
    for component in Path::new(raw_path).components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    let mut components = relative.components();
    match components.next() {
        Some(Component::Normal(name)) if name == "profiles" => {}
        _ => return None,
    }
    if components.any(|component| !matches!(component, Component::Normal(_))) {
        return None;
    }

    Some(root.join(relative))
}

fn serve_file(stream: &mut TcpStream, path: &Path) {
    let content_type = match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("css") => "text/css",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    };

    match std::fs::read(path) {
        Ok(body) => {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: \
                 {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(&body);
        }
        Err(_) => {
            // Try directory listing for /profiles/
            if path.is_dir() {
                serve_directory_listing(stream, path);
                return;
            }
            serve_not_found(stream);
        }
    }
}

fn serve_not_found(stream: &mut TcpStream) {
    let body = "404 Not Found";
    let response = format!(
        "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: \
         close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn directory_listing_body(dir: &Path) -> Vec<u8> {
    let mut entries = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                entries.push(name.to_string());
            }
        }
    }

    serde_json::to_vec(&entries).unwrap_or_else(|_| b"[]".to_vec())
}

fn serve_directory_listing(stream: &mut TcpStream, dir: &Path) {
    let body = directory_listing_body(dir);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: \
         {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(&body);
}

#[cfg(test)]
mod tests {
    use {
        super::{directory_listing_body, resolve_request_path},
        std::{
            fs,
            path::Path,
            time::{SystemTime, UNIX_EPOCH},
        },
    };

    fn temp_test_dir(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("quasar-profile-{name}-{unique}"));
        fs::create_dir_all(&dir).expect("create temp test dir");
        dir
    }

    #[test]
    fn request_path_defaults_to_index() {
        let root = Path::new("/tmp/quasar-profile-root");
        assert_eq!(
            resolve_request_path(root, "/"),
            Some(root.join("index.html"))
        );
        assert_eq!(
            resolve_request_path(root, ""),
            Some(root.join("index.html"))
        );
    }

    #[test]
    fn request_path_rejects_traversal() {
        let root = Path::new("/tmp/quasar-profile-root");
        assert_eq!(resolve_request_path(root, "/../secret.txt"), None);
        assert_eq!(
            resolve_request_path(root, "/profiles/../../secret.txt"),
            None
        );
        assert_eq!(resolve_request_path(root, "../secret.txt"), None);
    }

    #[test]
    fn request_path_only_serves_frontend_and_profiles() {
        let root = Path::new("/tmp/quasar-profile-root");
        assert_eq!(
            resolve_request_path(root, "/index.html"),
            Some(root.join("index.html"))
        );
        assert_eq!(
            resolve_request_path(root, "/profiles/demo.profile.json"),
            Some(root.join("profiles/demo.profile.json"))
        );
        assert_eq!(
            resolve_request_path(root, "/profiles"),
            Some(root.join("profiles"))
        );
        assert_eq!(resolve_request_path(root, "/src/lib.rs"), None);
        assert_eq!(resolve_request_path(root, "/Cargo.toml"), None);
    }

    #[test]
    fn directory_listing_body_is_valid_json() {
        let dir = temp_test_dir("listing");
        let quoted = dir.join("with\"quote.json");
        let slashy = dir.join("with\\slash.json");
        fs::write(&quoted, []).expect("write quoted test file");
        fs::write(&slashy, []).expect("write slash test file");

        let body = directory_listing_body(&dir);
        let entries: Vec<String> = serde_json::from_slice(&body).expect("parse directory JSON");
        assert!(entries.contains(&"with\"quote.json".to_string()));
        assert!(entries.contains(&"with\\slash.json".to_string()));

        fs::remove_dir_all(dir).expect("remove temp test dir");
    }
}
