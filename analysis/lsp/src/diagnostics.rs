//! Converts `quasar-hir`'s `HirDiagnostic` to `lsp_types::Diagnostic` and
//! translates byte offsets to LSP `Position`s using a [`LineIndex`].

use lsp_types::{
    Diagnostic as LspDiagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location,
    NumberOrString, Position, Range as LspRange, Uri,
};
use quasar_hir::HirDiagnostic;
use quasar_syntax::diagnostics::Severity;
use quasar_syntax::LineIndex;

pub fn convert(d: &HirDiagnostic, text: &str, line_index: &LineIndex, uri: &Uri) -> LspDiagnostic {
    let related: Vec<DiagnosticRelatedInformation> = d
        .labels
        .iter()
        .map(|label| DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range: range_for(line_index, text, label.range.start, label.range.end),
            },
            message: label.message.clone(),
        })
        .collect();

    LspDiagnostic {
        range: range_for(line_index, text, d.primary.start, d.primary.end),
        severity: Some(severity(d.severity)),
        code: Some(NumberOrString::String(d.code.as_str().to_string())),
        code_description: None,
        source: Some("quasar".to_string()),
        message: d.message.clone(),
        related_information: if related.is_empty() { None } else { Some(related) },
        tags: None,
        data: None,
    }
}

fn severity(s: Severity) -> DiagnosticSeverity {
    match s {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Info => DiagnosticSeverity::INFORMATION,
        Severity::Hint => DiagnosticSeverity::HINT,
    }
}

fn range_for(line_index: &LineIndex, text: &str, start: u32, end: u32) -> LspRange {
    LspRange {
        start: position_for(line_index, text, start),
        end: position_for(line_index, text, end),
    }
}

/// Convert a UTF-8 byte offset into an LSP `Position` (line + UTF-16 column).
pub fn position_for(line_index: &LineIndex, text: &str, byte_offset: u32) -> Position {
    let (line, byte_col) = line_index.position(byte_offset);
    let line_start = byte_offset - byte_col;
    let line_text = line_slice(text, line_start as usize, byte_col as usize);
    let character = line_text.encode_utf16().count() as u32;
    Position { line, character }
}

fn line_slice(text: &str, line_start: usize, byte_col: usize) -> &str {
    let end = (line_start + byte_col).min(text.len());
    // line_start should fall on a char boundary because LineIndex tracks
    // line starts (byte offsets just after '\n'). byte_col may land mid-char
    // if the diagnostic span ends mid-character — we clamp to the previous
    // boundary to avoid panicking.
    let end = text
        .get(..end)
        .map(|s| s.len())
        .unwrap_or_else(|| nearest_char_boundary(text, end));
    &text[line_start..end]
}

fn nearest_char_boundary(text: &str, mut idx: usize) -> usize {
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// Convert an LSP [`Position`] (line + UTF-16 code-unit column) into a UTF-8
/// byte offset. Returns the offset of the end of the line if the column
/// overruns.
pub fn lsp_position_to_byte_offset(text: &str, position: Position) -> u32 {
    let mut current_line = 0u32;
    let mut line_start = 0usize;

    for (i, b) in text.bytes().enumerate() {
        if current_line == position.line {
            break;
        }
        if b == b'\n' {
            current_line += 1;
            line_start = i + 1;
        }
    }
    if current_line < position.line {
        return text.len() as u32;
    }

    let line_end = text[line_start..]
        .find('\n')
        .map_or(text.len(), |i| line_start + i);
    let line_text = &text[line_start..line_end];

    let mut utf16_acc = 0u32;
    for (idx, c) in line_text.char_indices() {
        if utf16_acc >= position.character {
            return (line_start + idx) as u32;
        }
        utf16_acc += c.len_utf16() as u32;
    }
    line_end as u32
}

/// Convert a byte range to an LSP [`Range`] given the file's text.
pub fn byte_range_to_lsp_range(
    line_index: &LineIndex,
    text: &str,
    start: u32,
    end: u32,
) -> LspRange {
    range_for(line_index, text, start, end)
}
