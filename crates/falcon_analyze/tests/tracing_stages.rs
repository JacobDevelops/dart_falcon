use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation, RuleRegistry};
use falcon_config::FalconConfig;
use falcon_syntax::cst::Program;
use tracing::{Subscriber, span::Attributes};
use tracing_subscriber::{EnvFilter, Layer, layer::Context, prelude::*, registry::LookupSpan};

const FILE_COUNT: usize = 64;

#[derive(Clone, Debug)]
struct Record {
    name: &'static str,
    parent: Option<&'static str>,
    thread: ThreadId,
}

#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<Record>>>);

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Capture {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &tracing::Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).unwrap();
        self.0.lock().unwrap().push(Record {
            name: attrs.metadata().name(),
            parent: span.parent().map(|parent| parent.name()),
            thread: std::thread::current().id(),
        });
    }
}

struct EmptyRule;

struct Observer;
impl falcon_analyze::SemanticRuleVisitor<'_> for Observer {}

impl Rule for EmptyRule {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        for _ in 0..2 {
            ctx.function_bodies(program);
            ctx.const_contexts(program);
            ctx.local_declarations(program);
            ctx.local_writes(program);
            ctx.mutated_declarations(program);
            ctx.flow_writes(program);
            ctx.globals(program);
        }
        RulePreparation::Semantic(Box::new(Observer))
    }
}

fn capture(filter: &str, parallel: bool, resolve: bool, parent: bool) -> Vec<Record> {
    let records = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::new(filter))
        .with(Capture(records.clone()));
    tracing::subscriber::with_default(subscriber, || {
        let _analysis =
            parent.then(|| tracing::debug_span!(target: "falcon::stages", "analysis").entered());
        let mut registry = RuleRegistry::new();
        registry.register("empty", Box::new(EmptyRule));
        let mut declarations = String::new();
        for n in 0..32 {
            writeln!(
                declarations,
                "int f{n}(int v) {{ var x = v; x += 1; return x; }}"
            )
            .unwrap();
        }
        let files: Vec<_> = (0..FILE_COUNT)
            .map(|index| (format!("file{index}.dart").into(), declarations.clone()))
            .collect();
        let config = FalconConfig::default();
        let analyze = if parallel {
            falcon_analyze::analyze_parallel_collecting_resolving
        } else {
            falcon_analyze::analyze_sequential_collecting_resolving
        };
        let (_, retained, indexes) = analyze(
            &registry,
            &files,
            &config,
            true,
            resolve,
            falcon_analyze::Workspace::default(),
        )
        .unwrap();
        if let Some(mut indexes) = indexes {
            let file = &retained[0];
            let id = indexes.identities.file(file.path()).unwrap();
            indexes.refresh_file_body(id, file.parsed());
        }
        falcon_analyze::CrossFileRuleRegistry::new()
            .run_all(
                &falcon_analyze::AnalysisEnvironment::default(),
                &[],
                &config,
                None,
            )
            .unwrap();
    });
    records.lock().unwrap().clone()
}

fn run_in_own_process(name: &str) -> bool {
    let name = format!("tracing_stages::{name}");
    if std::env::var_os("FALCON_TRACING_TEST_CHILD").as_deref() == Some(std::ffi::OsStr::new(&name))
    {
        return false;
    }
    // Scoped test subscribers must not observe unrelated jobs stolen by shared Rayon workers.
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &name, "--nocapture"])
        .env("FALCON_TRACING_TEST_CHILD", &name)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.lines().any(|line| line == "running 1 test"),
        "{name} must run exactly once successfully:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    true
}

#[test]
fn stages_keep_their_parent_across_stack_and_rayon_workers() {
    if run_in_own_process("stages_keep_their_parent_across_stack_and_rayon_workers") {
        return;
    }
    let origin = std::thread::current().id();
    for parallel in [false, true] {
        for resolve in [false, true] {
            let records = capture("off,falcon::stages=debug", parallel, resolve, true);
            for name in [
                "parse",
                "registry_preparation",
                "shared_structural_traversal",
                "registry_reporting",
                "semantic_state",
                "shared_semantic_traversal",
                "registry_finish",
            ] {
                let spans: Vec<_> = records.iter().filter(|span| span.name == name).collect();
                let expected = if name == "shared_semantic_traversal" && !resolve {
                    0
                } else {
                    FILE_COUNT
                };
                assert_eq!(spans.len(), expected, "lost worker spans for {name}");
                if parallel
                    && matches!(name, "parse" | "registry_preparation")
                    && falcon_analyze::with_rules_stack(rayon::current_num_threads).unwrap() > 1
                {
                    let workers: std::collections::HashSet<_> =
                        spans.iter().map(|span| span.thread).collect();
                    assert!(workers.len() > 1, "{name} did not exercise work stealing");
                }
                for span in spans {
                    assert_eq!(span.parent, Some("analysis"), "{span:?}");
                    assert_ne!(span.thread, origin, "stage escaped worker attribution");
                }
            }
            let lazy: Vec<_> = records
                .iter()
                .filter(|span| span.name == "lazy_file_index")
                .collect();
            assert_eq!(
                lazy.len(),
                FILE_COUNT * 6,
                "indexes must initialize once per file"
            );
            assert!(
                lazy.iter()
                    .all(|span| span.parent == Some("registry_preparation"))
            );
            if resolve {
                assert!(
                    records
                        .iter()
                        .any(|span| span.name == "semantic_index_build")
                );
                assert!(records.iter().any(|span| {
                    span.name == "semantic_index_body_refresh" && span.parent == Some("analysis")
                }));
            }
            assert!(records.iter().any(|span| {
                span.name == "cross_file_rules" && span.parent == Some("analysis")
            }));
            assert!(
                !records
                    .iter()
                    .any(|span| { matches!(span.name, "analyze_file" | "prepare_rule") })
            );
        }
    }
}

#[test]
fn detailed_spans_require_opt_in_and_preparation_does_not_enclose_traversal() {
    if run_in_own_process(
        "detailed_spans_require_opt_in_and_preparation_does_not_enclose_traversal",
    ) {
        return;
    }
    let records = capture(
        "off,falcon::stages=debug,falcon::detail=debug",
        true,
        true,
        true,
    );
    for (name, parent) in [
        ("prepare_rule", "registry_preparation"),
        ("analyze_file", "analysis"),
    ] {
        let spans: Vec<_> = records.iter().filter(|span| span.name == name).collect();
        assert_eq!(
            spans.len(),
            FILE_COUNT,
            "lost detailed worker spans for {name}"
        );
        assert!(spans.iter().all(|span| span.parent == Some(parent)));
    }
    for span in records.iter().filter(|span| {
        matches!(
            span.name,
            "registry_preparation"
                | "shared_structural_traversal"
                | "semantic_state"
                | "shared_semantic_traversal"
                | "registry_finish"
                | "registry_reporting"
        )
    }) {
        assert_eq!(span.parent, Some("analyze_file"), "{span:?}");
    }
    assert!(
        records
            .iter()
            .filter(|span| span.name == "lazy_file_index")
            .all(|span| span.parent == Some("prepare_rule"))
    );
}

#[test]
fn stage_subscriber_survives_workers_without_an_entered_parent() {
    if run_in_own_process("stage_subscriber_survives_workers_without_an_entered_parent") {
        return;
    }
    let origin = std::thread::current().id();
    let records = capture("off,falcon::stages=debug", true, true, false);
    for name in [
        "parse",
        "semantic_index_build",
        "registry_preparation",
        "cross_file_rules",
    ] {
        let spans: Vec<_> = records.iter().filter(|span| span.name == name).collect();
        let expected = if matches!(name, "parse" | "registry_preparation") {
            FILE_COUNT
        } else {
            1
        };
        assert_eq!(spans.len(), expected, "lost worker spans for {name}");
        for span in spans {
            assert_eq!(span.parent, None, "{span:?}");
            assert_ne!(span.thread, origin, "subscriber lost on worker");
        }
    }
}

#[test]
fn ordinary_info_logging_has_no_stage_or_detail_spans() {
    if run_in_own_process("ordinary_info_logging_has_no_stage_or_detail_spans") {
        return;
    }
    assert!(capture("info", true, true, true).is_empty());
}
