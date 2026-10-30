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
    #[must_use]
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub fn contains(&self, other: Span) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    #[must_use]
    pub fn contains_offset(&self, offset: usize) -> bool {
        self.start <= offset && offset < self.end
    }

    #[must_use]
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

    #[test]
    fn containment_respects_half_open_offsets_and_empty_ranges() {
        let span = Span::new(3, 7);
        assert!(span.contains(span));
        assert!(span.contains(Span::new(4, 6)));
        assert!(span.contains(Span::new(7, 7)));
        assert!(!span.contains(Span::new(2, 7)));
        assert!(!span.contains(Span::new(3, 8)));
        assert!(span.contains_offset(3));
        assert!(span.contains_offset(6));
        assert!(!span.contains_offset(2));
        assert!(!span.contains_offset(7));
        assert!(!Span::new(3, 3).contains_offset(3));
    }
}
