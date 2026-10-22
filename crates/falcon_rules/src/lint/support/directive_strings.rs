use crate::lint::string_lit::{decode_escapes, decoded_text, literal_parts};
use falcon_syntax::cst::StringLitNode;

/// Decode each adjacent literal independently: one raw segment does not make
/// its non-raw neighbours raw.
pub(crate) fn uri_value(uri: StringLitNode<'_>) -> String {
    if !uri.is_adjacent() {
        if let Some(literal) = uri.delimiter()
            && literal.triple
        {
            return decoded_text(&literal, &literal_parts(&literal));
        }
        return if uri.has_raw_segment() {
            uri.value().into_owned()
        } else {
            decode_escapes(&uri.value()).into_owned()
        };
    }
    let mut value = String::with_capacity(
        uri.segments()
            .map(|(literal, _)| literal.content.len())
            .sum(),
    );
    for (literal, _) in uri.segments() {
        if literal.triple {
            value.push_str(&decoded_text(&literal, &literal_parts(&literal)));
        } else if literal.is_raw {
            value.push_str(literal.content);
        } else {
            value.push_str(&decode_escapes(literal.content));
        }
    }
    value
}
