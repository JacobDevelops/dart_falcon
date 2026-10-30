//! Parse a Dart file and print parse errors. Usage:
//!   cargo run -p `falcon_dart_parser` --example `parse_probe` -- <file.dart>
fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: parse_probe <file.dart>");
    assert!(args.next().is_none(), "usage: parse_probe <file.dart>");
    let src = std::fs::read_to_string(&path).expect("read file");
    let errors = falcon_dart_parser::parse(&src).errors;
    println!("errors: {}", errors.len());
    for e in &errors {
        println!("  {e:?}");
    }
    if !errors.is_empty() {
        std::process::exit(1);
    }
}
