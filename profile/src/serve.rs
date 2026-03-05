use std::ffi::CString;
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

pub fn is_port_listening(host: &str, port: u16) -> bool {
    let addr_str = format!("{}:{}", host, port);
    let Ok(addr) = addr_str.parse::<SocketAddr>() else {
        return false;
    };

    TcpStream::connect_timeout(&addr, Duration::from_millis(80)).is_ok()
}

pub fn spawn_server_process(root: &Path, port: u16) -> io::Result<()> {
    let root_string = root.to_string_lossy().into_owned();

    unsafe {
        let pid = libc::fork();
        if pid < 0 {
            return Err(io::Error::last_os_error());
        }

        if pid > 0 {
            return Ok(());
        }

        if libc::setsid() < 0 {
            libc::_exit(1);
        }

        let devnull = CString::new("/dev/null").expect("valid /dev/null");
        let null_fd = libc::open(devnull.as_ptr(), libc::O_RDWR);
        if null_fd >= 0 {
            libc::dup2(null_fd, 0);
            libc::dup2(null_fd, 1);
            libc::dup2(null_fd, 2);
            if null_fd > 2 {
                libc::close(null_fd);
            }
        }

        let rc = tinysrv::serve(root_string.as_bytes(), port);
        if rc < 0 {
            libc::_exit(1);
        }
        libc::_exit(0);
    }
}
