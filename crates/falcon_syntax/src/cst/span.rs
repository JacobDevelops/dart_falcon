/// Byte-range span for source attribution.
///
/// Parsed syntax uses half-open byte offsets ending at the last consumed token,
/// excluding following whitespace and comments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn merge(&self, other: &Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Span;

    #[test]
    fn merging_spans_covers_both_ranges() {
        let first = Span::new(3, 7);
        let second = Span::new(1, 5);
        assert_eq!(first.merge(&second), Span::new(1, 7));
        assert_eq!(second.merge(&first), Span::new(1, 7));
        assert_eq!(first.merge(&first), first);
    }
}
