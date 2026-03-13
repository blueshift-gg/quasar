use std::{
    ffi::CString,
    io,
    net::{SocketAddr, TcpStream},
    path::Path,
    time::Duration,
};

pub fn spawn_server_process(root: &Path, port: u16) -> io::Result<()> {
    // converting path into owned UFT-8 string
    let root_bytes = root.to_string_lossy().into_owned();

    unsafe {
        match libc::fork() {
            -1 => Err(io::Error::last_os_error()),
            0 => {
                daemonize_stdio()?;
                let rc = tinysrv::serve(root_bytes.as_bytes(), port);
                libc::_exit(if rc < 0 { 1 } else { 0 });
            }
            _parent_pid => {
                // parent: return immedietly; server continues in background
                Ok(())
            }
        }
    }
}

pub fn is_port_listening(host: &str, port: u16) -> bool {
    let addr_str = format!("{}:{}", host, port);
    let Ok(addr) = addr_str.parse::<SocketAddr>() else {
        return false;
    };

    TcpStream::connect_timeout(&addr, Duration::from_millis(80)).is_ok()
}

unsafe fn daemonize_stdio() -> io::Result<()> {
    if libc::setsid() < 0 {
        return Err(io::Error::last_os_error());
    }

    let devnull = CString::new("/dev/null").expect("static string, no interior NUL");
    let fd = libc::open(devnull.as_ptr(), libc::O_RDWR);

    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    // redirect stdin, stdout, stderr to clean ux
    libc::dup2(fd, 0);
    libc::dup2(fd, 1);
    libc::dup2(fd, 2);

    if fd > 2 {
        libc::close(fd);
    }

    Ok(())
}
