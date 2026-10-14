pub(crate) fn is_lower_camel_case(name: &str) -> bool {
    if name.len() == 1 && name.as_bytes()[0].is_ascii_uppercase() {
        return true;
    }
    let rest = name.trim_start_matches('_');
    let Some(first) = rest.chars().next() else {
        return true;
    };
    (first.is_ascii_lowercase() || first == '$')
        && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '$')
}

pub(crate) fn is_upper_camel_case(name: &str) -> bool {
    let rest = name.trim_start_matches('_');
    let Some(first) = rest.chars().next() else {
        return true;
    };
    (first.is_ascii_uppercase() || first == '$')
        && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '$')
}

pub(crate) fn is_snake_case(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|first| first.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

pub(crate) fn has_leading_underscore(name: &str) -> bool {
    name.starts_with('_') && !name.bytes().all(|b| b == b'_')
}
