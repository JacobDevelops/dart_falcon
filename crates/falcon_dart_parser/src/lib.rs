//! Hand-rolled Dart 3.x recursive-descent parser.
//!
//! Produces a [`falcon_syntax`] AST from Dart source text.
//! No external grammar dependencies (no tree-sitter, no Dart SDK).

#[cfg(feature = "cst-differential")]
mod differential;
pub mod lexer;
pub mod parser;

pub use parser::{
    ParseResult, lex_syntax, lex_tree, parse, parse_for_package, try_parse_for_package,
};
