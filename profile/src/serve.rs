use std::{io, path::Path};

pub fn serve(root: &Path, port: u16) -> io::Result<()> {
    let root_bytes = root.to_string_lossy().into_owned();

    unsafe {
        let rc = tinysrv::serve(root_bytes.as_bytes(), port);
        if rc < 0 {
            Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("failed to bind local profiler server on port {}", port),
            ))
        } else {
            Ok(())
        }
    }
}
