//! Shared schema utilities consumed by multiple Quasar crates.
//!
//! Keep this crate narrow: only shared case-conversion utilities belong here.
//! The canonical IDL type definitions live in `quasar-idl-schema`.

/// Convert `PascalCase` to `snake_case`. Handles acronyms (e.g.
/// "HTTPServer" becomes "http_server") by checking adjacent character case.
pub fn pascal_to_snake(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    let mut prev: Option<char> = None;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_uppercase() && prev.is_some() {
            let prev_lower = prev.is_some_and(|p| p.is_lowercase());
            let next_lower = chars.peek().is_some_and(|n| n.is_lowercase());
            if prev_lower || next_lower {
                result.push('_');
            }
        }
        result.push(c.to_ascii_lowercase());
        prev = Some(c);
    }
    result
}

/// Convert `snake_case` to `PascalCase`.
pub fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

/// Convert `snake_case` to `camelCase`.
pub fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert `camelCase` to `snake_case` (inverse of `to_camel_case`).
///
/// Uses the simple rule of inserting `_` before every uppercase character.
/// Not suitable for acronym-heavy input like "HTTPServer"; use
/// `pascal_to_snake` for that.
pub fn camel_to_snake(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_ascii_lowercase());
    }
    result
}

/// Convert `PascalCase` or `camelCase` to `SCREAMING_SNAKE_CASE`.
pub fn to_screaming_snake(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_ascii_uppercase());
    }
    result
}

/// Capitalize first character of a `camelCase` string to get `PascalCase`.
pub fn camel_to_pascal(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_to_snake_words_and_acronyms() {
        assert_eq!(pascal_to_snake(""), "");
        assert_eq!(pascal_to_snake("A"), "a");
        assert_eq!(pascal_to_snake("Simple"), "simple");
        assert_eq!(pascal_to_snake("TwoWords"), "two_words");
        // acronym handling: separators only at case boundaries
        assert_eq!(pascal_to_snake("HTTPServer"), "http_server");
        assert_eq!(pascal_to_snake("MyHTTPServer"), "my_http_server");
        assert_eq!(pascal_to_snake("ABC"), "abc");
        assert_eq!(pascal_to_snake("parseURL"), "parse_url");
    }

    #[test]
    fn snake_to_pascal_words() {
        assert_eq!(snake_to_pascal(""), "");
        assert_eq!(snake_to_pascal("word"), "Word");
        assert_eq!(snake_to_pascal("two_words"), "TwoWords");
        assert_eq!(snake_to_pascal("a_b_c"), "ABC");
        // empty segments collapse rather than panic
        assert_eq!(snake_to_pascal("double__underscore"), "DoubleUnderscore");
    }

    #[test]
    fn to_camel_case_words() {
        assert_eq!(to_camel_case(""), "");
        assert_eq!(to_camel_case("word"), "word");
        assert_eq!(to_camel_case("two_words"), "twoWords");
        assert_eq!(to_camel_case("a_b_c"), "aBC");
        // a leading underscore capitalizes the first letter and is dropped
        assert_eq!(to_camel_case("_x"), "X");
    }

    #[test]
    fn camel_to_snake_simple_rule() {
        assert_eq!(camel_to_snake(""), "");
        assert_eq!(camel_to_snake("abc"), "abc");
        assert_eq!(camel_to_snake("camelCase"), "camel_case");
        // no separator before index 0
        assert_eq!(camel_to_snake("Upper"), "upper");
        assert_eq!(camel_to_snake("aB"), "a_b");
    }

    #[test]
    fn camel_and_snake_round_trip() {
        assert_eq!(to_camel_case(&camel_to_snake("fooBarBaz")), "fooBarBaz");
        assert_eq!(camel_to_snake(&to_camel_case("foo_bar_baz")), "foo_bar_baz");
    }

    #[test]
    fn to_screaming_snake_cases() {
        assert_eq!(to_screaming_snake(""), "");
        assert_eq!(to_screaming_snake("abc"), "ABC");
        assert_eq!(to_screaming_snake("camelCase"), "CAMEL_CASE");
        assert_eq!(to_screaming_snake("Pascal"), "PASCAL");
    }

    #[test]
    fn camel_to_pascal_first_char_only() {
        assert_eq!(camel_to_pascal(""), "");
        assert_eq!(camel_to_pascal("x"), "X");
        assert_eq!(camel_to_pascal("camelCase"), "CamelCase");
    }
}
