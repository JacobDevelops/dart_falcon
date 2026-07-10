// Header regular comments are ignored (only_doc_comments); the doc comments
// below are the real fixtures.

/// lowercase start makes this an invalid sentence. /* expect: format-comment */
void badLower() {}

/// Missing terminal punctuation /* expect: format-comment */
void badNoPunct() {}

/// A leading sentence that wraps but the whole block never /* expect: format-comment */
/// terminates with any punctuation
void badWrapped() {}
