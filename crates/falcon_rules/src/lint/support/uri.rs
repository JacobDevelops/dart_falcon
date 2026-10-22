//! Dart URI classification shared by import rules.
/// The scheme of `uri` as Dart's `Uri.tryParse` sees it: `Some("")` for a
/// scheme-less (relative) reference, `Some(scheme)` for an absolute URI, and
/// `None` when the text is not a parseable URI.
pub(crate) fn uri_scheme(uri: &str) -> Option<&str> {
    let end = uri.find(['/', '?', '#']).unwrap_or(uri.len());
    let scheme = match uri[..end].find(':') {
        Some(colon) => {
            let scheme = &uri[..colon];
            let mut chars = scheme.chars();
            let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
                && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
            if !valid {
                return None;
            }
            scheme
        }
        None => "",
    };
    valid_percent_escapes(uri).then_some(scheme)
}

fn valid_percent_escapes(uri: &str) -> bool {
    let bytes = uri.as_bytes();
    bytes.iter().enumerate().all(|(index, &byte)| {
        byte != b'%'
            || bytes
                .get(index + 1..index + 3)
                .is_some_and(|hex| hex.iter().all(u8::is_ascii_hexdigit))
    })
}
