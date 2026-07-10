// Fixtures for the format-comment rule, configured with only_doc_comments:true.
// Regular // comments (like this header) are NOT checked, so lowercase and
// unterminated regular comments never fire here.

/// A correctly formatted single-line doc comment.
void single() {}

// this regular comment is lowercase and unterminated but is skipped entirely
// because only_doc_comments is enabled.
void regularIgnored() {}

/// This is a long description that wraps across two source
/// lines and still ends with proper terminal punctuation.
void wrapped() {}

/// {@template my.macro}
/// Body text goes here.
/// {@endtemplate}
void macro() {}

/// A block whose first sentence ends early. And a second wrapped
/// sentence that also terminates cleanly.
void twoSentences() {}
