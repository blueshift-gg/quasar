//! Doc comments, rendered per language.
//!
//! The derive already collects doc comments and the schema carries them on
//! instructions, accounts, types, and fields. Backends render them here so a
//! generated client documents itself in the consumer's editor.

use std::fmt::Write;

/// Strip anything that would end the comment early or break out of it. Docs
/// come from an IDL that a generator has no reason to trust with delimiters.
fn sanitize(line: &str) -> String {
    line.replace("*/", "*\u{200b}/")
        .replace(['\r', '\n'], " ")
        .trim_end()
        .to_owned()
}

/// Rust / Go / C++-style line comments: `/// text` or `// text`.
pub fn line_comments(out: &mut String, docs: &[String], indent: &str, marker: &str) {
    for line in docs {
        let text = sanitize(line);
        if text.is_empty() {
            writeln!(out, "{indent}{marker}").expect("write to String");
        } else {
            writeln!(out, "{indent}{marker} {text}").expect("write to String");
        }
    }
}

/// A JSDoc block, omitted entirely when there is nothing to say.
pub fn jsdoc(out: &mut String, docs: &[String], indent: &str) {
    if docs.is_empty() {
        return;
    }
    if docs.len() == 1 {
        writeln!(out, "{indent}/** {} */", sanitize(&docs[0])).expect("write to String");
        return;
    }
    writeln!(out, "{indent}/**").expect("write to String");
    for line in docs {
        writeln!(out, "{indent} * {}", sanitize(line)).expect("write to String");
    }
    writeln!(out, "{indent} */").expect("write to String");
}

/// A C block comment.
pub fn c_block(out: &mut String, docs: &[String], indent: &str) {
    if docs.is_empty() {
        return;
    }
    for line in docs {
        writeln!(out, "{indent}/* {} */", sanitize(line)).expect("write to String");
    }
}

/// A Python docstring body, already indented, or `None` when there are no docs.
pub fn py_docstring(docs: &[String], indent: &str) -> Option<String> {
    if docs.is_empty() {
        return None;
    }
    let mut out = String::new();
    if docs.len() == 1 {
        writeln!(out, "{indent}\"\"\"{}\"\"\"", sanitize(&docs[0])).expect("write to String");
    } else {
        writeln!(out, "{indent}\"\"\"").expect("write to String");
        for line in docs {
            writeln!(out, "{indent}{}", sanitize(line)).expect("write to String");
        }
        writeln!(out, "{indent}\"\"\"").expect("write to String");
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_terminators_cannot_escape_the_comment() {
        let mut out = String::new();
        jsdoc(&mut out, &["ends here */ and code()".to_owned()], "");
        assert!(!out.contains("*/ and"));
        assert!(out.trim_end().ends_with("*/"));
    }

    #[test]
    fn newlines_never_break_a_line_comment() {
        let mut out = String::new();
        line_comments(&mut out, &["first\nsecond".to_owned()], "", "///");
        assert_eq!(out.lines().count(), 1);
    }

    #[test]
    fn empty_docs_emit_nothing() {
        let mut out = String::new();
        jsdoc(&mut out, &[], "  ");
        c_block(&mut out, &[], "  ");
        line_comments(&mut out, &[], "  ", "//");
        assert!(out.is_empty());
        assert!(py_docstring(&[], "    ").is_none());
    }
}
