/// Return the newline index or `end`, leaving the newline unconsumed.
pub(crate) fn skip_line(bytes: &[u8], mut pos: usize, end: usize) -> usize {
    while pos < end && !matches!(bytes[pos], b'\n' | b'\r') {
        pos += 1;
    }
    pos
}

/// From `/` at `open`, return past the nested comment's close, or `None` before `end`.
pub(crate) fn skip_block_comment(bytes: &[u8], open: usize, end: usize) -> Option<usize> {
    let mut pos = open + 2;
    let mut depth = 1usize;
    while pos < end {
        if pos + 1 < end && bytes[pos] == b'/' && bytes[pos + 1] == b'*' {
            depth += 1;
            pos += 2;
        } else if pos + 1 < end && bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
            depth -= 1;
            pos += 2;
            if depth == 0 {
                return Some(pos);
            }
        } else {
            pos += 1;
        }
    }
    None
}

/// From `{` at `open_brace`, return its closing `}` index, or `None` before exclusive `end`.
pub(crate) fn interp_close(bytes: &[u8], open_brace: usize, end: usize) -> Option<usize> {
    let mut depth = 1usize;
    let mut pos = open_brace + 1;
    while pos < end {
        match bytes[pos] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
            }
            b'/' if pos + 1 < end && bytes[pos + 1] == b'/' => {
                pos = skip_line(bytes, pos + 2, end);
                continue;
            }
            b'/' if pos + 1 < end && bytes[pos + 1] == b'*' => {
                pos = skip_block_comment(bytes, pos, end)?;
                continue;
            }
            b'\'' | b'"' => {
                pos = skip_nested_string(bytes, pos, end)?;
                continue;
            }
            _ => {}
        }
        pos += 1;
    }
    None
}

/// From a quote at `open`, return past its matching delimiter, or `None` before `end`.
fn skip_nested_string(bytes: &[u8], open: usize, end: usize) -> Option<usize> {
    let quote = bytes[open];
    let raw = open > 0 && bytes[open - 1] == b'r';
    let triple = open + 2 < end && bytes[open + 1] == quote && bytes[open + 2] == quote;
    let mut pos = open + if triple { 3 } else { 1 };
    // Only triple-quoted strings can cross a physical newline.
    while pos < end {
        if !triple && matches!(bytes[pos], b'\n' | b'\r') {
            return None;
        }
        if bytes[pos] == b'\\' && !raw {
            if !triple && pos + 1 < end && matches!(bytes[pos + 1], b'\n' | b'\r') {
                return None;
            }
            pos += 2;
        } else if bytes[pos] == quote
            && (!triple || (pos + 2 < end && bytes[pos + 1] == quote && bytes[pos + 2] == quote))
        {
            return Some(pos + if triple { 3 } else { 1 });
        } else {
            pos += 1;
        }
    }
    None
}
