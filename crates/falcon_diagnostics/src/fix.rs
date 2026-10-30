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
    pub title: String,
    pub kind: FixKind,
    edits: Vec<TextEdit>,
}

impl Fix {
    /// Create a fix after checking that its edits are ordered and non-overlapping.
    ///
    /// # Errors
    ///
    /// Returns an error if there are no edits, an edit has an invalid span, or
    /// edits are unsorted or overlapping.
    pub fn new(
        title: impl Into<String>,
        kind: FixKind,
        edits: Vec<TextEdit>,
    ) -> Result<Self, ApplyFixError> {
        if edits.is_empty() {
            return Err(ApplyFixError::EmptyEdits);
        }
        let mut end = 0;
        for (index, edit) in edits.iter().enumerate() {
            if edit.span.start > edit.span.end {
                return Err(ApplyFixError::InvalidSpan { index });
            }
            if edit.span.start < end {
                return Err(ApplyFixError::UnsortedOrOverlapping { index });
            }
            end = edit.span.end;
        }
        Ok(Self {
            title: title.into(),
            kind,
            edits,
        })
    }

    #[must_use]
    pub fn edits(&self) -> &[TextEdit] {
        &self.edits
    }

    /// Check the edit bounds and UTF-8 boundaries against the original source.
    /// # Errors
    ///
    /// Returns an error if an edit extends beyond `source` or splits a UTF-8
    /// character.
    pub fn validate(&self, source: &str) -> Result<(), ApplyFixError> {
        for (index, edit) in self.edits.iter().enumerate() {
            if !source.is_char_boundary(edit.span.start) || !source.is_char_boundary(edit.span.end)
            {
                return Err(ApplyFixError::InvalidSpan { index });
            }
        }
        Ok(())
    }

    /// Apply all edits to the original source, or reject the entire fix.
    ///
    /// Spans must be sorted, non-overlapping, within bounds, and on UTF-8 character
    /// boundaries. Empty spans insert text; empty replacements delete text.
    /// Adjacent edits are allowed, including insertions at the same offset, which
    /// are applied in their declared order. The caller decides which kinds to apply.
    ///
    /// # Errors
    ///
    /// Returns an error if an edit extends beyond `source` or splits a UTF-8
    /// character.
    pub fn apply(&self, source: &str) -> Result<String, ApplyFixError> {
        self.validate(source)?;

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

/// An invalid fix, with an edit index where applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyFixError {
    EmptyEdits,
    InvalidSpan { index: usize },
    UnsortedOrOverlapping { index: usize },
}

impl fmt::Display for ApplyFixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyEdits => f.write_str("a fix must contain at least one edit"),
            Self::InvalidSpan { index } => write!(f, "edit {index} has an invalid source span"),
            Self::UnsortedOrOverlapping { index } => {
                write!(f, "edit {index} is unsorted or overlaps an earlier edit")
            }
        }
    }
}

impl std::error::Error for ApplyFixError {}
