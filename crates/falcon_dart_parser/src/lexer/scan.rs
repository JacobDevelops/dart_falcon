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
    enum Frame {
        Interpolation(usize),
        String { quote: u8, raw: bool, triple: bool },
    }
    let mut frames = Vec::new();
    let mut frame = Frame::Interpolation(1);
    let mut pos = open_brace + 1;
    while pos < end {
        match &mut frame {
            Frame::Interpolation(depth) => match bytes[pos] {
                b'{' => *depth += 1,
                b'}' => {
                    *depth -= 1;
                    if *depth == 0 {
                        if let Some(parent) = frames.pop() {
                            frame = parent;
                        } else {
                            return Some(pos);
                        }
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
                quote @ (b'\'' | b'"') => {
                    let raw = pos > 0 && bytes[pos - 1] == b'r';
                    let triple =
                        pos + 2 < end && bytes[pos + 1] == quote && bytes[pos + 2] == quote;
                    frames.push(std::mem::replace(
                        &mut frame,
                        Frame::String { quote, raw, triple },
                    ));
                    pos += if triple { 3 } else { 1 };
                    continue;
                }
                _ => {}
            },
            Frame::String { quote, raw, triple } => {
                if !*triple && matches!(bytes[pos], b'\n' | b'\r') {
                    return None;
                }
                if bytes[pos] == b'\\' && !*raw {
                    if !*triple && pos + 1 < end && matches!(bytes[pos + 1], b'\n' | b'\r') {
                        return None;
                    }
                    pos += 2;
                    continue;
                }
                if !*raw && bytes[pos] == b'$' && pos + 1 < end && bytes[pos + 1] == b'{' {
                    frames.push(std::mem::replace(&mut frame, Frame::Interpolation(1)));
                    pos += 2;
                    continue;
                }
                if bytes[pos] == *quote
                    && (!*triple
                        || (pos + 2 < end && bytes[pos + 1] == *quote && bytes[pos + 2] == *quote))
                {
                    pos += if *triple { 3 } else { 1 };
                    frame = frames.pop()?;
                    continue;
                }
            }
        }
        pos += 1;
    }
    None
}
