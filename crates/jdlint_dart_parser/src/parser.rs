/// Parse Dart source text into an AST.
///
/// # Errors
/// Returns a parse error if the source is syntactically invalid.
pub fn parse(_source: &str) -> Result<(), ParseError> {
    todo!("Dart parser — implemented in M1")
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub offset: usize,
}
