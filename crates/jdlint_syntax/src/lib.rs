pub mod ast;
pub mod syntax_kind;
pub mod token;
pub mod visitor;

pub use ast::Program;
pub use syntax_kind::SyntaxKind;
pub use token::{Token, TokenKind};

/// Locked AST format version for Phase 1.
/// Any breaking change to AST enum shapes requires bumping MAJOR.
pub const JDLINT_AST_FORMAT_VERSION: &str = "1.0";
