use std::{fs, path::Path};

/// Generate `schema/falcon.schema.json` from the rule metadata, or with
/// `--check` verify the committed file matches (CI drift guard).
pub(crate) fn run(check: bool) {
    let path = Path::new("schema/falcon.schema.json");
    let generated = falcon_rules::schema::config_schema_string();

    if check {
        let current = fs::read_to_string(path).unwrap_or_default();
        if current == generated {
            println!("schema up to date: {}", path.display());
        } else {
            eprintln!(
                "error: {} is out of date; run `cargo xtask schema`",
                path.display()
            );
            std::process::exit(1);
        }
        return;
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create schema dir");
    }
    fs::write(path, &generated).expect("write schema");
    println!("wrote {}", path.display());
}
