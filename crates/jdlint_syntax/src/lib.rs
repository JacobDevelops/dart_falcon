//! Dart AST node types and syntax tree definitions.
//!
//! Defines the immutable AST produced by `jdlint_dart_parser` and
//! consumed by `jdlint_analyze`. Format is locked after M1.5.

pub mod ast;
pub mod token;

pub use ast::Program;
