//! Dependency-free semantic indexing baseline; use release builds for comparisons.
//!
//! Run from the repository root:
//! `numen run -- mbx test -p falcon_analyze --release --lib inference_index_baseline -- --ignored --nocapture --test-threads=1`
//!
//! Keep this corpus and the generated fixture unchanged when comparing algorithms.
//! Timing includes fresh SignatureIndex construction and declaration inference,
//! but excludes source generation, I/O, parsing, identity/type indexes, checksums,
//! printing, destruction, and per-sample identity cloning (which resets lookup
//! and alias-template caches). No external SDK/package sources are loaded; curated
//! fallback signatures are used. The rule corpus is a collection of independent
//! libraries, not one runnable Dart package. Parse-recovery inputs are retained.

use std::fmt::Write as _;
use std::hint::black_box;
use std::time::Instant;

use super::*;
use crate::resolve::{IdentitySource, LibrarySource, group_libraries};

const SAMPLES: usize = 5;
const CHAIN_LENGTH: usize = 256;
const INDEPENDENT_DECLARATIONS: usize = 2_048;

// Fixed FNV-1a rather than a randomized/process-specific standard hasher.
fn checksum(bytes: &[u8], state: &mut u64) {
    for byte in bytes {
        *state ^= u64::from(*byte);
        *state = state.wrapping_mul(0x100000001b3);
    }
    *state ^= 0xff;
    *state = state.wrapping_mul(0x100000001b3);
}

fn dart_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).expect("read rule corpus directory") {
        let entry = entry.expect("read rule corpus entry");
        let kind = entry.file_type().expect("read rule corpus file type");
        if kind.is_dir() {
            dart_files(&entry.path(), files);
        } else if kind.is_file() && entry.path().extension().is_some_and(|ext| ext == "dart") {
            files.push(entry.path());
        }
    }
}

// ResolvedType's Hash follows structural equality and excludes alias provenance.
// Comparisons require unchanged structural hashing, Rust toolchain, and rustc_hash.
struct Fact<T>(T, bool);

impl<T: Hash + std::fmt::Debug> std::fmt::Debug for Fact<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.1 {
            let mut hash = rustc_hash::FxHasher::default();
            self.0.hash(&mut hash);
            write!(formatter, "{:016x}", hash.finish())
        } else {
            self.0.fmt(formatter)
        }
    }
}

// Sort complete fact rows to remove map iteration order. Only semantic output is
// included: no NodeRef/arena addresses, caches, or elapsed-time-dependent state.
fn fact_summary(index: &SignatureIndex, provenance_free: bool) -> (usize, usize, usize, u64) {
    let members: usize = index
        .member_entries
        .iter()
        .map(|(_, entries)| entries.len())
        .sum();
    let extension_rows: usize = index
        .extensions
        .iter()
        .map(|extension| extension.methods.len() + extension.getters.len())
        .sum();
    let mut rows = Vec::with_capacity(
        index.functions.len() + index.value_types.len() + members + extension_rows,
    );
    for (identity, signature) in index.functions.iter() {
        rows.push(format!(
            "function {identity:?} {:?}",
            Fact(signature.function_type(), provenance_free)
        ));
    }
    for (identity, ty) in index.value_types.iter() {
        rows.push(format!(
            "value {identity:?} {:?}",
            Fact(ty, provenance_free)
        ));
    }
    for (identity, entries) in index.member_entries.iter() {
        for (name, entry) in entries {
            rows.push(format!(
                "member {identity:?} {name:?} {:?} {:?} {:?} {:?}",
                Fact(
                    entry
                        .signature
                        .as_ref()
                        .map(ResolvedSignature::function_type),
                    provenance_free,
                ),
                Fact(&entry.field, provenance_free),
                Fact(&entry.read, provenance_free),
                Fact(&entry.write, provenance_free),
            ));
        }
    }
    for (ordinal, extension) in index.extensions.iter().enumerate() {
        for (name, signature) in &extension.methods {
            rows.push(format!(
                "extension {ordinal} method {name:?} {:?}",
                Fact(signature.function_type(), provenance_free)
            ));
        }
        for (name, ty) in &extension.getters {
            rows.push(format!(
                "extension {ordinal} getter {name:?} {:?}",
                Fact(ty, provenance_free),
            ));
        }
    }
    rows.sort_unstable();
    let mut hash = 0xcbf29ce484222325;
    for row in rows {
        checksum(row.as_bytes(), &mut hash);
    }
    (
        index.functions.len(),
        index.value_types.len(),
        members,
        hash,
    )
}

fn benchmark(label: &str, sources: Vec<(PathBuf, String)>, require_clean_parse: bool) {
    let mut input_hash = 0xcbf29ce484222325;
    let mut parse_errors = Vec::with_capacity(sources.len());
    let files: Vec<_> = sources
        .into_iter()
        .map(|(path, source)| {
            checksum(path.to_string_lossy().as_bytes(), &mut input_hash);
            checksum(source.as_bytes(), &mut input_hash);
            let parsed = falcon_dart_parser::parse(&source);
            if require_clean_parse {
                assert!(parsed.errors.is_empty(), "{path:?}: {:?}", parsed.errors);
            }
            parse_errors.push(!parsed.errors.is_empty());
            (
                path,
                Arc::new(crate::ParsedFile {
                    source: source.into(),
                    tree: parsed.tree,
                }),
            )
        })
        .collect();
    assert!(!files.is_empty(), "benchmark corpus must not be empty");
    let declarations: usize = files
        .iter()
        .map(|(_, parsed)| parsed.program().declarations().count())
        .sum();
    let identity_sources: Vec<_> = files
        .iter()
        .zip(&parse_errors)
        .map(|((path, parsed), &has_parse_errors)| IdentitySource {
            path,
            parsed,
            has_parse_errors,
        })
        .collect();
    let identities = IdentityIndex::from_project_files(&identity_sources, &[]);
    let project_files: Vec<_> = files
        .iter()
        .map(|(path, parsed)| (path.clone(), parsed.program()))
        .collect();
    let grouping = group_libraries(&project_files);
    let types = TypeIndex::from_resolved_files(
        identity_sources
            .iter()
            .enumerate()
            .map(|(ordinal, source)| {
                let program = source.parsed.program();
                (
                    source.path,
                    LibrarySource {
                        program,
                        has_parse_errors: source.has_parse_errors,
                        has_unresolved_parts: grouping.is_unresolved(ordinal),
                    },
                )
            }),
        &identities,
    );
    println!(
        "{label}: files={} top_level_declarations={declarations} parse_recovery_files={} input_fnv1a={input_hash:016x}",
        files.len(),
        parse_errors.iter().filter(|&&errors| errors).count(),
    );
    let mut samples = Vec::with_capacity(SAMPLES);
    let mut expected = None;
    for sample in 1..=SAMPLES {
        let sample_identities = identities.clone();
        let started = Instant::now();
        let index = black_box(SignatureIndex::from_project_files(
            black_box(&files),
            black_box(&sample_identities),
            black_box(&types),
        ));
        let elapsed = started.elapsed();
        if require_clean_parse {
            assert_chain_endpoints(&index, &files, &sample_identities, &types);
        }
        let summary = (fact_summary(&index, false), fact_summary(&index, true));
        if let Some(expected) = expected {
            assert_eq!(summary, expected, "semantic facts changed between samples");
        } else {
            expected = Some(summary);
        }
        println!(
            "{label}: sample={sample} index_ms={:.3}",
            elapsed.as_secs_f64() * 1_000.0
        );
        samples.push(elapsed);
        // Drop the complete fresh index outside the measured interval.
        drop(index);
    }
    samples.sort_unstable();
    let ((functions, values, members, debug_hash), (_, _, _, hash)) =
        expected.expect("five samples");
    println!(
        "{label}: median_ms={:.3} functions={functions} values={values} members={members} provenance_free_fnv1a={hash:016x} debug_fnv1a={debug_hash:016x}",
        samples[SAMPLES / 2].as_secs_f64() * 1_000.0,
    );
}

fn assert_chain_endpoints(
    index: &SignatureIndex,
    files: &[(PathBuf, Arc<crate::ParsedFile>)],
    identities: &IdentityIndex,
    types: &TypeIndex,
) {
    let (path, parsed) = &files[0];
    let model = SemanticModel::new(path, parsed, identities, Some(types));
    for name in ["value0".to_owned(), format!("value{CHAIN_LENGTH}")] {
        let ty = index.value_type(
            &model
                .resolve_value(name.as_str())
                .expect("chain value identity"),
        );
        assert!(
            ty.is_some_and(|ty| ty.interface("dart:core", "int")),
            "{name} must infer int, got {ty:?}",
        );
    }
    let chain = model.resolve_name("Chain").expect("Chain class identity");
    for name in ["field0".to_owned(), format!("field{CHAIN_LENGTH}")] {
        let ty = index
            .member_entry(&chain, &name)
            .and_then(|entry| entry.field.as_ref())
            .map(|(_, ty)| ty);
        assert!(
            ty.is_some_and(|ty| ty.interface("dart:core", "int")),
            "Chain.{name} must infer int, got {ty:?}",
        );
    }
}

#[test]
#[ignore = "manual release inference baseline; run with --release --ignored --nocapture"]
fn inference_index_baseline() {
    println!(
        "build_profile={} debug_assertions={}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        cfg!(debug_assertions),
    );
    println!(
        "command: numen run -- mbx test -p falcon_analyze --release --lib inference_index_baseline -- --ignored --nocapture --test-threads=1"
    );
    println!(
        "five fresh-index samples; identity lookup/alias caches reset outside each timer; no warmup; parsing/identity/type prerequisites and output hashing excluded; curated fallback-only SDK signatures; no external SDK/packages; library grouping resolves available parts"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../falcon_rules/tests/corpus");
    let mut paths = Vec::new();
    dart_files(&root, &mut paths);
    paths.sort_unstable();
    let sources = paths
        .into_iter()
        .map(|path| {
            let source = std::fs::read_to_string(&path).expect("read Dart corpus file");
            // Relative names keep identities and fingerprints checkout-independent.
            (
                Path::new("corpus").join(path.strip_prefix(&root).unwrap()),
                source,
            )
        })
        .collect();
    benchmark("rule_corpus", sources, false);

    let mut source = String::with_capacity((CHAIN_LENGTH * 2 + INDEPENDENT_DECLARATIONS + 2) * 48);
    // Consumers precede their prerequisites in source order. Both top-level
    // values and member fields participate in declaration inference.
    for ordinal in 0..CHAIN_LENGTH {
        writeln!(source, "final value{ordinal} = value{};", ordinal + 1).unwrap();
    }
    writeln!(source, "final value{CHAIN_LENGTH} = 1;").unwrap();
    writeln!(source, "class Chain {{").unwrap();
    for ordinal in 0..CHAIN_LENGTH {
        writeln!(source, "late final field{ordinal} = field{};", ordinal + 1).unwrap();
    }
    writeln!(source, "late final field{CHAIN_LENGTH} = 1; }}").unwrap();
    for ordinal in 0..INDEPENDENT_DECLARATIONS {
        writeln!(source, "final independent{ordinal} = {ordinal};").unwrap();
    }
    println!(
        "reverse_chain: dependency_edges_per_chain={CHAIN_LENGTH} independent_declarations={INDEPENDENT_DECLARATIONS}"
    );
    benchmark(
        "reverse_chain",
        vec![("reverse_chain.dart".into(), source)],
        true,
    );
}
