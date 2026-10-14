pub fn errs(src: &str) -> usize {
    falcon_dart_parser::parser::parse(src).1.len()
}
