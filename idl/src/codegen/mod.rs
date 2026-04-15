pub mod golang;
pub mod python;
pub mod rust;
pub mod typescript;

use std::fmt::Write;

/// Format discriminator bytes as a comma-separated decimal list: `1, 2, 3`.
pub fn format_disc_decimal(disc: &[u8]) -> String {
    let mut s = String::with_capacity(disc.len() * 4);
    for (i, b) in disc.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        write!(s, "{}", b).expect("write to String");
    }
    s
}

/// Format discriminator bytes as a comma-separated hex list: `0x01, 0x02,
/// 0x03`.
pub fn format_disc_hex(disc: &[u8]) -> String {
    disc.iter()
        .map(|b| format!("0x{:02x}", b))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format discriminator bytes as a bracketed decimal array: `[1, 2, 3]`.
pub fn format_disc_array_decimal(disc: &[u8]) -> String {
    let mut s = String::with_capacity(disc.len() * 4 + 2);
    s.push('[');
    for (i, b) in disc.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        write!(s, "{}", b).expect("write to String");
    }
    s.push(']');
    s
}
