//! Byte-offset ↔ line/column mapping.

pub struct LineIndex {
    line_starts: Vec<u32>,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0u32];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self { line_starts }
    }

    /// `(line, column)` in UTF-8 bytes, zero-indexed.
    pub fn position(&self, byte_offset: u32) -> (u32, u32) {
        let idx = self
            .line_starts
            .partition_point(|&start| start <= byte_offset)
            .saturating_sub(1);
        let line = idx as u32;
        let col = byte_offset - self.line_starts[idx];
        (line, col)
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
}
