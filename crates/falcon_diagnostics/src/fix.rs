use std::fmt;

use serde::Serialize;

use crate::Span;

/// A rule's assessment of whether a fix preserves program behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FixKind {
    /// Preserves program behaviour.
    Safe,
    /// May change program behaviour, even when the diagnostic is correct.
    Unsafe,
}

/// A byte-span replacement in the original source, not in previously edited text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextEdit {
    pub span: Span,
    pub replacement: String,
}

impl TextEdit {
    pub fn new(span: impl Into<Span>, replacement: impl Into<String>) -> Self {
        Self {
            span: span.into(),
            replacement: replacement.into(),
        }
    }
}

/// One atomic fix, with edits in ascending source order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Fix {
    pub kind: FixKind,
    pub edits: Vec<TextEdit>,
}

impl Fix {
    pub fn new(kind: FixKind, edits: Vec<TextEdit>) -> Self {
        Self { kind, edits }
    }

    /// Apply all edits to the original source, or reject the entire fix.
    ///
    /// Spans must be sorted, non-overlapping, within bounds, and on UTF-8 character
    /// boundaries. Empty spans insert text; empty replacements delete text.
    /// Adjacent edits are allowed, including insertions at the same offset, which
    /// are applied in their declared order. The caller decides which kinds to apply.
    pub fn apply(&self, source: &str) -> Result<String, ApplyFixError> {
        let mut end = 0;
        for (index, edit) in self.edits.iter().enumerate() {
            let span = edit.span;
            if span.start > span.end
                || !source.is_char_boundary(span.start)
                || !source.is_char_boundary(span.end)
            {
                return Err(ApplyFixError::InvalidSpan { index });
            }
            if span.start < end {
                return Err(ApplyFixError::UnsortedOrOverlapping { index });
            }
            end = span.end;
        }

        let mut result = String::with_capacity(source.len());
        let mut end = 0;
        for edit in &self.edits {
            result.push_str(&source[end..edit.span.start]);
            result.push_str(&edit.replacement);
            end = edit.span.end;
        }
        result.push_str(&source[end..]);
        Ok(result)
    }
}

/// The index identifies the invalid edit in [`Fix::edits`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyFixError {
    InvalidSpan { index: usize },
    UnsortedOrOverlapping { index: usize },
}

impl fmt::Display for ApplyFixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSpan { index } => write!(f, "edit {index} has an invalid source span"),
            Self::UnsortedOrOverlapping { index } => {
                write!(f, "edit {index} is unsorted or overlaps an earlier edit")
            }
        }
    }
}

impl std::error::Error for ApplyFixError {}
