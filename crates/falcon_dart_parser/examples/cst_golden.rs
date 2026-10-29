use std::fmt::Write as _;
use std::{
    collections::BTreeMap,
    env, fs,
    hash::{Hash, Hasher},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

use falcon_dart_parser::parse;
use falcon_syntax::cst::{SyntaxElement, SyntaxTree};
use walkdir::WalkDir;

struct StableHasher(u64);

impl StableHasher {
    fn new() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl Hasher for StableHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}

impl std::fmt::Write for StableHasher {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        Hasher::write(self, value.as_bytes());
        Ok(())
    }
}

fn hash<T: Hash>(value: &T) -> String {
    let mut hasher = StableHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn shape_hash(tree: &SyntaxTree, source: &str) -> String {
    let mut hasher = StableHasher::new();
    for node in tree.root(source).descendants() {
        write!(&mut hasher, "{:?}", node.kind()).unwrap();
        node.span().hash(&mut hasher);
        let children = node.children_with_tokens();
        children.clone().count().hash(&mut hasher);
        for child in children {
            match child {
                SyntaxElement::Node(child) => {
                    0u8.hash(&mut hasher);
                    write!(&mut hasher, "{:?}", child.kind()).unwrap();
                }
                SyntaxElement::Token(token) => {
                    1u8.hash(&mut hasher);
                    write!(&mut hasher, "{:?}", token.kind()).unwrap();
                    token.span().hash(&mut hasher);
                }
            }
        }
    }
    format!("{:016x}", hasher.finish())
}

fn collect(root: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in WalkDir::new(root).into_iter().filter_entry(|entry| {
        !entry.file_type().is_dir()
            || !matches!(entry.file_name().to_str(), Some(".dart_tool" | "build"))
    }) {
        let entry = entry.map_err(io::Error::other)?;
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "dart")
        {
            files.push(entry.into_path());
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const USAGE: &str = "usage: cst_golden --write|--check <file> <corpus-root>...";
    let mut args = env::args_os().skip(1);
    let mode = args.next().ok_or(USAGE)?;
    let output = PathBuf::from(args.next().ok_or(USAGE)?);
    let roots: Vec<PathBuf> = args.map(PathBuf::from).collect();
    if roots.is_empty() || (mode != "--write" && mode != "--check") {
        return Err(USAGE.into());
    }

    let mut files = Vec::new();
    for root in &roots {
        collect(root, &mut files)?;
    }
    files.sort();
    files.dedup();

    let started = Instant::now();
    let mut bytes = 0u64;
    let mut skipped = 0;
    let mut rows = BTreeMap::new();
    let mut contents = Vec::new();
    let mut debug = String::new();
    for path in files {
        contents.clear();
        fs::File::open(&path)?.read_to_end(&mut contents)?;
        let (Some(path), Ok(source)) = (path.to_str(), std::str::from_utf8(&contents)) else {
            skipped += 1;
            continue;
        };
        bytes += source.len() as u64;
        let parsed = parse(source);
        debug.clear();
        write!(&mut debug, "{:?}", parsed.tree.program(source))?;
        let debug_hash = hash(&debug);
        let mut error_hasher = StableHasher::new();
        parsed.errors.len().hash(&mut error_hasher);
        for error in &parsed.errors {
            (&error.message, error.offset).hash(&mut error_hasher);
        }
        let error_hash = format!("{:016x}", error_hasher.finish());
        let node_count = parsed.tree.root(source).descendants().count();
        let token_count = parsed.tree.tokens(source).count();
        rows.insert(
            path.to_owned(),
            format!(
                "{}\t{}\t{}\t{}\t{}",
                debug_hash,
                error_hash,
                shape_hash(&parsed.tree, source),
                node_count,
                token_count
            ),
        );
    }

    if mode == "--write" {
        let mut file = fs::File::create(output)?;
        for (path, row) in &rows {
            writeln!(file, "{path}\t{row}")?;
        }
        eprintln!(
            "wrote {} files, {} bytes in {:.3}s; skipped {} non-UTF-8 files",
            rows.len(),
            bytes,
            started.elapsed().as_secs_f64(),
            skipped
        );
        return Ok(());
    }

    check_golden(&output, &rows, bytes, started, skipped)
}

fn check_golden(
    output: &Path,
    rows: &BTreeMap<String, String>,
    bytes: u64,
    started: Instant,
    skipped: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let expected = fs::read_to_string(output)?;
    let mut expected_rows = BTreeMap::new();
    for line in expected.lines() {
        let (path, row) = line.split_once('\t').ok_or("invalid golden row")?;
        if expected_rows.insert(path, row).is_some() {
            return Err(format!("duplicate golden path: {path}").into());
        }
    }
    let mut mismatches = 0;
    for (path, expected) in &expected_rows {
        match rows.get(*path) {
            None => {
                eprintln!("missing: {path}");
                mismatches += 1;
            }
            Some(actual) if actual != expected => {
                eprintln!("mismatch: {path}\n  expected: {expected}\n  actual:   {actual}");
                mismatches += 1;
            }
            Some(_) => {}
        }
    }
    for path in rows.keys() {
        if !expected_rows.contains_key(path.as_str()) {
            eprintln!("extra: {path}");
            mismatches += 1;
        }
    }
    eprintln!(
        "checked {} files, {} bytes in {:.3}s; {} mismatches; skipped {} non-UTF-8 files",
        rows.len(),
        bytes,
        started.elapsed().as_secs_f64(),
        mismatches,
        skipped
    );
    if mismatches != 0 {
        return Err(format!("golden mismatch count: {mismatches}").into());
    }
    Ok(())
}
