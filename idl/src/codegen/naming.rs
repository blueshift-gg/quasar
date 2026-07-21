use heck::{ToLowerCamelCase, ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase};

pub(super) fn pascal_to_snake(value: &str) -> String {
    value.to_snake_case()
}

pub(super) fn snake_to_pascal(value: &str) -> String {
    value.to_upper_camel_case()
}

pub(super) fn to_camel_case(value: &str) -> String {
    value.to_lower_camel_case()
}

pub(super) fn camel_to_snake(value: &str) -> String {
    value.to_snake_case()
}

pub(super) fn to_screaming_snake(value: &str) -> String {
    value.to_shouty_snake_case()
}

pub(super) fn camel_to_pascal(value: &str) -> String {
    value.to_upper_camel_case()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_quasar_idl_names() {
        assert_eq!(pascal_to_snake("HTTPServer"), "http_server");
        assert_eq!(snake_to_pascal("two_words"), "TwoWords");
        assert_eq!(to_camel_case("two_words"), "twoWords");
        assert_eq!(camel_to_snake("twoWords"), "two_words");
        assert_eq!(to_screaming_snake("TwoWords"), "TWO_WORDS");
        assert_eq!(camel_to_pascal("twoWords"), "TwoWords");
    }
}
