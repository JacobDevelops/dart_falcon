pub fn errs(src: &str) -> usize {
    falcon_dart_parser::parser::parse(src).errors.len()
}
