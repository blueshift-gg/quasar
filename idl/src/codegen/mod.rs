pub mod golang;
pub mod model;
pub mod python;
pub mod rust;
pub mod typescript;

/// Parse the size from a fixed-size array primitive like `"[u8; 8]"` → `8`.
pub fn parse_fixed_array_size(p: &str) -> Option<usize> {
    let inner = p.strip_prefix('[')?.strip_suffix(']')?;
    let (_, size_str) = inner.split_once(';')?;
    size_str.trim().parse().ok()
}

/// Format discriminator bytes as a decimal comma-separated list (no brackets).
pub fn format_disc_decimal(disc: &[u8]) -> String {
    disc.iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format discriminator bytes as a hex comma-separated list (no brackets).
pub fn format_disc_hex(disc: &[u8]) -> String {
    disc.iter()
        .map(|b| format!("0x{:02x}", b))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format discriminator bytes as a decimal array with brackets: `[1, 2, 3]`.
pub fn format_disc_array(disc: &[u8]) -> String {
    use std::fmt::Write;
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
