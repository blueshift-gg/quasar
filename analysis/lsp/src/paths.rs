//! File URI ⇄ filesystem path conversion.
//!
//! `file://` URIs are percent-encoded and (on Windows) embed a drive letter
//! after the leading slash. Naively stripping `file://` breaks on any path with
//! a space, non-ASCII character, or Windows drive — the file is gated out and
//! the server appears silently dead. These helpers round-trip correctly so
//! activation and goto work wherever the project lives.

use {
    lsp_types::Uri,
    percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS},
    std::{
        path::{Path, PathBuf},
        str::FromStr,
    },
};

/// Characters encoded when building a file URI. We deliberately leave `/`
/// (separators) and `:` (Windows drive) intact and encode the rest of the
/// unsafe/reserved set, including `%` so round-tripping is unambiguous.
const PATH_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'%');

/// `file://` URI → filesystem path. Percent-decodes the path and, on Windows,
/// strips the leading slash from a `/C:/…` drive form. Returns `None` for
/// non-`file` URIs or undecodable input.
pub fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    file_str_to_path(uri.as_str())
}

/// Same as [`uri_to_path`] but from a raw URI string (used before a `Uri` has
/// been parsed, e.g. the `initialize` workspace folders).
pub fn file_str_to_path(s: &str) -> Option<PathBuf> {
    let rest = s.strip_prefix("file://")?;
    // After the scheme comes an optional authority (host) then the absolute
    // path. For local files the authority is empty → `file:///path`. If a host
    // is present (`file://host/path`), keep only the path component.
    let path_part = match rest.find('/') {
        Some(0) => rest,
        Some(idx) => &rest[idx..],
        None => return None,
    };
    let decoded = percent_decode_str(path_part).decode_utf8().ok()?;

    #[cfg(windows)]
    {
        if let Some(stripped) = strip_windows_drive_prefix(&decoded) {
            return Some(PathBuf::from(stripped.replace('/', "\\")));
        }
    }

    Some(PathBuf::from(decoded.as_ref()))
}

/// Filesystem path → `file://` URI, percent-encoding unsafe characters. On
/// Windows, backslashes are normalized to `/` and an absolute path gains the
/// leading `/` of the URI path. Returns `None` if the path isn't valid UTF-8.
pub fn path_to_uri(path: &Path) -> Option<Uri> {
    let raw = path.to_str()?;

    #[cfg(windows)]
    let normalized = {
        let forward = raw.replace('\\', "/");
        if forward.starts_with('/') {
            forward
        } else {
            format!("/{forward}")
        }
    };
    #[cfg(not(windows))]
    let normalized = raw.to_string();

    let encoded = utf8_percent_encode(&normalized, PATH_ENCODE_SET).to_string();
    Uri::from_str(&format!("file://{encoded}")).ok()
}

#[cfg(windows)]
fn strip_windows_drive_prefix(p: &str) -> Option<&str> {
    let b = p.as_bytes();
    if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
        Some(&p[1..])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_plain_unix_path() {
        let p = Path::new("/Users/me/project/src/lib.rs");
        let uri = path_to_uri(p).unwrap();
        assert_eq!(uri.as_str(), "file:///Users/me/project/src/lib.rs");
        assert_eq!(uri_to_path(&uri).unwrap(), p);
    }

    #[test]
    fn round_trips_path_with_space_and_unicode() {
        let p = Path::new("/Users/me/My Project/café/lib.rs");
        let uri = path_to_uri(p).unwrap();
        // Space and non-ASCII must be percent-encoded in the URI…
        assert!(
            uri.as_str().contains("My%20Project"),
            "got {}",
            uri.as_str()
        );
        assert!(!uri.as_str().contains(' '));
        // …and decode back to the original path.
        assert_eq!(uri_to_path(&uri).unwrap(), p);
    }

    #[test]
    fn decodes_client_encoded_uri() {
        let uri = Uri::from_str("file:///tmp/a%20b/c.rs").unwrap();
        assert_eq!(uri_to_path(&uri).unwrap(), Path::new("/tmp/a b/c.rs"));
    }

    #[test]
    fn rejects_non_file_uri() {
        let uri = Uri::from_str("https://example.com/x").unwrap();
        assert_eq!(uri_to_path(&uri), None);
    }
}
