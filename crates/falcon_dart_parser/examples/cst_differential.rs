use std::any::Any;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use falcon_dart_parser::parse;
use walkdir::WalkDir;

#[derive(Default)]
struct Counts {
    discovered: usize,
    parsed: usize,
    diagnostics: usize,
    bytes: usize,
}

fn main() -> ExitCode {
    let mut counts = Counts::default();
    let result = run(&mut counts);
    if let Err(error) = &result {
        eprintln!("{error}");
    }
    println!(
        "files={} parsed={} diagnostics={} bytes={} failures={}",
        counts.discovered,
        counts.parsed,
        counts.diagnostics,
        counts.bytes,
        usize::from(result.is_err()),
    );
    if result.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn run(counts: &mut Counts) -> Result<(), Box<dyn Error>> {
    let mut roots: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if roots.is_empty() {
        roots.push(PathBuf::from("/Users/jacob/Documents/Developer/jfit"));
        let home =
            std::env::var_os("HOME").ok_or("HOME is required for the default pub-cache root")?;
        roots.push(PathBuf::from(home).join(".pub-cache"));
    }
    let mut files = Vec::new();
    for root in roots {
        for entry in WalkDir::new(&root).into_iter().filter_entry(|entry| {
            !entry.file_type().is_dir()
                || (entry.file_name() != ".dart_tool" && entry.file_name() != "build")
        }) {
            let entry = entry?;
            if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "dart")
                && (entry.file_type().is_file() || entry.path().is_file())
            {
                files.push(entry.into_path());
            }
        }
    }
    files.sort();
    files.dedup();
    counts.discovered = files.len();
    if files.is_empty() {
        return Err("no Dart files found".into());
    }
    for path in files {
        let source = std::fs::read_to_string(&path)
            .map_err(|error| std::io::Error::other(format!("{}: {error}", path.display())))?;
        let result = std::panic::catch_unwind(|| parse(&source))
            .map_err(|payload| parse_failure(&path, payload.as_ref()))?;
        counts.parsed += 1;
        counts.diagnostics += result.errors.len();
        counts.bytes += source.len();
    }
    Ok(())
}

fn parse_failure(path: &Path, payload: &(dyn Any + Send)) -> std::io::Error {
    let message = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("parser panicked with a non-string payload");
    std::io::Error::other(format!("{}:\n{message}", path.display()))
}
