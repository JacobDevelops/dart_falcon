//! Parse a Dart file and print parse errors. Usage:
//!   cargo run -p falcon_dart_parser --example parse_probe -- <file.dart> [--ast]
fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: parse_probe <file.dart> [--ast]");
    let want_ast = args.next().as_deref() == Some("--ast");
    let src = std::fs::read_to_string(&path).expect("read file");
    let (program, errors) = falcon_dart_parser::parse(&src);
    println!("errors: {}", errors.len());
    for e in &errors {
        println!("  {e:?}");
    }
    if want_ast {
        println!("{program:#?}");
    }
    if !errors.is_empty() {
        std::process::exit(1);
    }
}
