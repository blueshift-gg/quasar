use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

/// Check if something is already listening on the given port.
pub fn is_alive(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().unwrap(),
        Duration::from_millis(200),
    )
    .is_ok()
}

/// Start a background HTTP server that serves static files from `root`.
///
/// Forks a child process that serves files and auto-shuts down after
/// `idle_secs` of inactivity (no new connections = browser tab closed).
///
/// Returns the URL on success.
pub fn serve_background(root: &Path, port: u16, program: &str) -> std::io::Result<String> {
    // Try to bind first to fail fast if port is busy
    let listener = TcpListener::bind(format!("127.0.0.1:{port}"))?;
    let url = format!("http://127.0.0.1:{port}/?program={program}");

    let root = root.to_path_buf();

    // Fork: child process runs the server, parent returns immediately
    unsafe {
        let pid = libc::fork();
        if pid < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if pid > 0 {
            // Parent — drop our copy of the listener and return
            drop(listener);
            return Ok(url);
        }
        // Child — detach from parent's process group
        libc::setsid();
    }

    // Child process: serve until idle
    run_server(listener, &root, 30);
    std::process::exit(0);
}

/// Blocking serve (for --diff mode which needs to stay alive until Ctrl-C).
pub fn serve_blocking(root: &Path, port: u16) -> std::io::Result<()> {
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

    let file_path = if path == "/" {
        root.join("index.html")
    } else {
        root.join(path.trim_start_matches('/'))
    };

    serve_file(&mut stream, &file_path);
}

fn serve_file(stream: &mut TcpStream, path: &PathBuf) {
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
            let body = "404 Not Found";
            let response = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: \
                 {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    }
}

fn serve_directory_listing(stream: &mut TcpStream, dir: &Path) {
    let mut entries: Vec<String> = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                entries.push(format!("\"{name}\""));
            }
        }
    }
    let body = format!("[{}]", entries.join(","));
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: \
         {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}
