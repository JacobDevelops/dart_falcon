pub(crate) fn is_lower_camel_case(name: &str) -> bool {
    if name == "_" || (name.len() == 1 && matches!(name.as_bytes()[0], b'@'..=b'Z')) {
        return true;
    }
    let rest = name.trim_start_matches('_').as_bytes();
    if !rest
        .first()
        .is_some_and(|first| first.is_ascii_lowercase() || matches!(first, b'$' | b'?'))
    {
        return false;
    }
    rest.iter().enumerate().all(|(index, byte)| {
        byte.is_ascii_alphanumeric()
            || matches!(byte, b'$' | b'?')
            || (*byte == b'_' && (index + 1 == rest.len() || rest[index + 1].is_ascii_digit()))
    })
}

pub(crate) fn is_upper_camel_case(name: &str) -> bool {
    let mut rest = name.trim_start_matches('_');
    loop {
        let after_dollars = rest.trim_start_matches('$');
        if after_dollars.len() == rest.len() || !after_dollars.starts_with('_') {
            break;
        }
        rest = after_dollars.trim_start_matches('_');
    }
    rest.as_bytes()
        .first()
        .is_some_and(|first| first.is_ascii_uppercase() || matches!(first, b'$' | b'?'))
        && rest
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'$' | b'?'))
}

pub(crate) fn is_snake_case(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|first| first.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

pub(crate) fn has_leading_underscore(name: &str) -> bool {
    name.starts_with('_') && !name.bytes().all(|b| b == b'_')
}

#[cfg(test)]
mod tests {
    use super::{is_lower_camel_case, is_upper_camel_case};

    #[test]
    fn lower_camel_matches_upstream_word_boundaries() {
        for name in ["_", "A", "a", "_value_", "value_4", "a_1B_2$", "$", "?"] {
            assert!(is_lower_camel_case(name), "{name}");
        }
        for name in ["", "__", "_4", "bad_name", "a__", "a__4", "AB", "a_$_"] {
            assert!(!is_lower_camel_case(name), "{name}");
        }
    }

    #[test]
    fn upper_camel_matches_upstream_generated_prefixes() {
        for name in [
            "A",
            "_A",
            "$",
            "?",
            "_$Generated",
            "_$_Generated",
            "$_A",
            "$$_$__A",
        ] {
            assert!(is_upper_camel_case(name), "{name}");
        }
        for name in ["", "_", "__", "$_", "a", "_$_lower", "Bad_Name", "A_"] {
            assert!(!is_upper_camel_case(name), "{name}");
        }
    }
}
