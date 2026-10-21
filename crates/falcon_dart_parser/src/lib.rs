//! Hand-rolled Dart 3.x recursive-descent parser.
//!
//! Produces a lossless [`falcon_syntax::cst::SyntaxTree`] from Dart source text.
//! No external grammar dependencies (no tree-sitter, no Dart SDK).

pub mod lexer;
pub mod parser;

pub use parser::{
    ParseError, ParseErrorKind, ParseResult, lex_tree, parse, parse_for_package,
    try_parse_for_package,
};
