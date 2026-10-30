use falcon_syntax::cst::{Span, StringDelimiter, StringLitNode};

pub(crate) fn interpolated_literals(
    node: StringLitNode<'_>,
) -> impl Iterator<Item = (StringDelimiter<'_>, Span)> {
    node.segments()
        .filter(|(lit, _)| lit.terminated && !lit.is_raw)
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

/// One piece of a single string literal's undecoded body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LiteralPart<'a> {
    /// Literal characters with escape sequences left undecoded.
    Text(&'a str),
    /// A `$name` or `${expr}` interpolation, including the `$`.
    Interpolation(&'a str),
}

/// Splits a literal body into text and interpolation parts. Raw literals are one
/// text part; an unclosed `${` stays text.
pub(crate) fn literal_parts<'a>(lit: &StringDelimiter<'a>) -> Vec<LiteralPart<'a>> {
    let content = lit.content;
    if lit.is_raw {
        return vec![LiteralPart::Text(content)];
    }
    let bytes = content.as_bytes();
    let mut parts = Vec::new();
    let mut text_start = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'$' => {
                let end = match bytes.get(i + 1) {
                    Some(b'{') => matching_brace(bytes, i + 1).map(|close| close + 1),
                    Some(&next) if next.is_ascii_alphabetic() || next == b'_' => {
                        let mut end = i + 2;
                        while end < bytes.len() && is_ident_body(bytes[end]) {
                            end += 1;
                        }
                        Some(end)
                    }
                    _ => None,
                };
                let Some(end) = end else {
                    i += 1;
                    continue;
                };
                if text_start < i {
                    parts.push(LiteralPart::Text(&content[text_start..i]));
                }
                parts.push(LiteralPart::Interpolation(&content[i..end]));
                i = end;
                text_start = end;
            }
            _ => i += 1,
        }
    }
    if text_start < bytes.len() {
        parts.push(LiteralPart::Text(&content[text_start..]));
    }
    parts
}

/// Decodes Dart escape sequences in undecoded, non-raw literal text.
pub(crate) fn decode_escapes(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains('\\') {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(escaped) = chars.next() else {
            out.push('\\');
            break;
        };
        match escaped {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            'f' => out.push('\u{0C}'),
            'b' => out.push('\u{08}'),
            't' => out.push('\t'),
            'v' => out.push('\u{0B}'),
            'x' | 'u' => {
                let braced = escaped == 'u' && chars.peek() == Some(&'{');
                if braced {
                    chars.next();
                }
                let max = match (escaped, braced) {
                    ('x', _) => 2,
                    (_, true) => 6,
                    _ => 4,
                };
                let mut value = 0u32;
                let mut count = 0;
                while count < max
                    && let Some(digit) = chars.peek().and_then(|d| d.to_digit(16))
                {
                    value = value * 16 + digit;
                    count += 1;
                    chars.next();
                }
                if braced && chars.peek() == Some(&'}') {
                    chars.next();
                }
                if count > 0
                    && let Some(decoded) = char::from_u32(value)
                {
                    out.push(decoded);
                }
            }
            other => out.push(other),
        }
    }
    std::borrow::Cow::Owned(out)
}

/// The leading line Dart drops from a multiline literal when that line holds
/// only whitespace (optionally backslash-escaped) before its line break.
pub(crate) fn multiline_lead_len(content: &str) -> usize {
    let bytes = content.as_bytes();
    let mut i = 0;
    while matches!(
        bytes.get(i..),
        Some([b' ' | b'\t', ..] | [b'\\', b' ' | b'\t' | b'\r' | b'\n', ..])
    ) {
        i += 1;
    }
    match bytes.get(i..) {
        Some([b'\r', b'\n', ..]) => i + 2,
        Some([b'\n' | b'\r', ..]) => i + 1,
        _ => 0,
    }
}

/// The decoded value of a literal's text parts, concatenated, with the
/// interpolations left out. Mirrors the analyzer's `value` /
/// `InterpolationString.value`, including the dropped leading line of a
/// multiline literal.
pub(crate) fn decoded_text(lit: &StringDelimiter<'_>, parts: &[LiteralPart<'_>]) -> String {
    let mut out = String::with_capacity(lit.content.len());
    for (index, part) in parts.iter().enumerate() {
        let LiteralPart::Text(text) = *part else {
            continue;
        };
        let text = if index == 0 && lit.triple {
            &text[multiline_lead_len(text)..]
        } else {
            text
        };
        if lit.is_raw {
            out.push_str(text);
        } else {
            out.push_str(&decode_escapes(text));
        }
    }
    out
}

/// Whether the literal body has any `$name` / `${expr}` interpolation.
pub(crate) fn has_interpolation(parts: &[LiteralPart<'_>]) -> bool {
    parts
        .iter()
        .any(|part| matches!(part, LiteralPart::Interpolation(_)))
}
