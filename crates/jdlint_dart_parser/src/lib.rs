//! Hand-rolled Dart 3.x recursive-descent parser.
//!
//! Produces a [`jdlint_syntax`] AST from Dart source text.
//! No external grammar dependencies (no tree-sitter, no Dart SDK).

pub mod lexer;
pub mod parser;

pub use parser::parse;
