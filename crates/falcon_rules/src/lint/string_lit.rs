use falcon_syntax::ast::{StringDelimiter, StringLitNode};

pub(crate) fn interpolated_literal(node: &StringLitNode) -> Option<StringDelimiter<'_>> {
    node.delimiter().filter(|lit| lit.terminated && !lit.is_raw)
}

// Excluding `$` prevents adjacent interpolations from merging into one identifier.
pub(crate) fn is_simple_identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(is_ident_body)
}

pub(crate) fn is_ident_body(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

pub(crate) fn matching_brace(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 1,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}
