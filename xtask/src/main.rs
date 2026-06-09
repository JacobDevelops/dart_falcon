fn main() {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("codegen") => codegen(),
        _ => {
            eprintln!("Available tasks: codegen");
            std::process::exit(1);
        }
    }
}

fn codegen() {
    println!("codegen: rule visitor stubs — not yet implemented");
}
