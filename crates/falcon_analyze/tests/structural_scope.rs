use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};

use falcon_analyze::{
    AnalyzeContext, ParsedFile, Rule, RuleAnalysis, RulePreparation, RuleRegistry, StructuralNode,
    StructuralRuleVisitor, StructuralTraversal,
};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity, Span};
use falcon_syntax::cst::visit::{Visitor, walk_expr};
use falcon_syntax::cst::{Expr, FormalParamList, FunctionDecl, Program};

type Event = (usize, &'static str, usize);
type Events = Sender<Event>;

struct TraceRule {
    id: usize,
    policy: u8,
    events: Events,
}

impl Rule for TraceRule {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        _ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        self.events.send((self.id, "prepare", 0)).unwrap();
        RulePreparation::Structural(Box::new(TraceCollector {
            id: self.id,
            policy: self.policy,
            events: self.events.clone(),
            functions: Vec::new(),
        }))
    }
}

struct TraceCollector {
    id: usize,
    policy: u8,
    events: Events,
    functions: Vec<usize>,
}

impl TraceCollector {
    fn record(&self, kind: &'static str, key: usize) {
        self.events.send((self.id, kind, key)).unwrap();
    }
}

falcon_analyze::structural_visitor! {
    impl<'a> StructuralRuleVisitor<'a> for TraceCollector {
        fn visit_program(&mut self, _node: Program<'a>) -> StructuralTraversal {
            self.record("program", 0);
            StructuralTraversal::Continue
        }

        fn exit_program(&mut self, _node: Program<'a>) {
            assert!(self.functions.is_empty());
            self.record("program-exit", 0);
        }

        fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
            self.functions.push(node.span().start);
            self.record("function", node.span().start);
            if self.policy == 1 && node.name().name() == "hidden" {
                StructuralTraversal::SkipChildren
            } else {
                StructuralTraversal::Continue
            }
        }

        fn exit_function_decl(&mut self, node: FunctionDecl<'a>) {
            assert_eq!(self.functions.pop(), Some(node.span().start));
            self.record("function-exit", node.span().start);
        }

        fn visit_formal_param_list(&mut self, node: FormalParamList<'a>) -> StructuralTraversal {
            self.record("params", node.syntax().id().0 as usize);
            StructuralTraversal::Continue
        }

        fn exit_formal_param_list(&mut self, node: FormalParamList<'a>) {
            self.record("params-exit", node.syntax().id().0 as usize);
        }

        fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
            self.record("expr", node.span().start);
            self.record("expr-node-id", node.syntax().id().0 as usize);
            if self.policy == 2 && matches!(node, Expr::Binary(_)) {
                StructuralTraversal::SkipChildren
            } else {
                StructuralTraversal::Continue
            }
        }

        fn exit_expr(&mut self, node: Expr<'a>) {
            self.record("expr-exit", node.span().start);
        }

        fn route_child(&mut self, parent: StructuralNode<'a>, child: StructuralNode<'a>) -> bool {
            !(self.policy == 2
                && matches!(parent, StructuralNode::FunctionKind(_))
                && matches!(child, StructuralNode::FormalParamList(_)))
        }

        fn finish(self: Box<Self>) -> Vec<Diagnostic> {
            assert!(self.functions.is_empty());
            self.record("finish", 0);
            vec![Diagnostic::new(
                "structural-test",
                Severity::Info,
                self.id.to_string(),
                "test.dart",
                Span { start: 0, end: 0 },
            )]
        }
    }
}

fn run(source: &str, policies: &[u8]) -> (Vec<Diagnostic>, Vec<Event>) {
    let parsed = Arc::new(ParsedFile {
        source: source.into(),
        tree: falcon_dart_parser::parse(source).tree,
    });
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
    let (events, receiver) = mpsc::channel();
    let mut registry = RuleRegistry::new();
    for (id, &policy) in policies.iter().enumerate() {
        registry.register(
            "structural-test",
            Box::new(TraceRule {
                id,
                policy,
                events: events.clone(),
            }),
        );
    }
    let diagnostics = registry.run_all(parsed.program(), &context);
    (diagnostics, receiver.try_iter().collect())
}

fn keys(events: &[(usize, &'static str, usize)], id: usize, kind: &str) -> Vec<usize> {
    events
        .iter()
        .filter_map(|&(owner, event, key)| (owner == id && event == kind).then_some(key))
        .collect()
}

#[test]
fn shared_collectors_preserve_existing_source_order_and_node_identity() {
    let source = "void f() { for (var i = 0; i < 2; i += 1) { print(3); } var (a, b) = (4, 5); }";
    struct Baseline(Vec<usize>);
    impl<'a> Visitor<'a> for Baseline {
        fn visit_expr(&mut self, node: Expr<'a>) {
            self.0.push(node.span().start);
            walk_expr(self, node);
        }
    }
    let parsed = Arc::new(ParsedFile {
        source: source.into(),
        tree: falcon_dart_parser::parse(source).tree,
    });
    let mut baseline = Baseline(Vec::new());
    baseline.visit_program(parsed.program());
    let (diagnostics, events) = run(source, &[0, 0]);
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(keys(&events, 0, "expr"), baseline.0);
    assert_eq!(keys(&events, 1, "expr"), baseline.0);
    assert_eq!(
        keys(&events, 0, "expr-node-id"),
        keys(&events, 1, "expr-node-id")
    );
    let update = source.find("i += 1").unwrap();
    let body = source.find("print(3)").unwrap();
    let order = keys(&events, 0, "expr");
    assert!(
        order.iter().position(|&start| start == update).unwrap()
            < order.iter().position(|&start| start == body).unwrap()
    );
    assert_eq!(
        keys(&events, 0, "expr").len(),
        keys(&events, 0, "expr-exit").len()
    );
    let first_finish = events.iter().position(|event| event.1 == "finish").unwrap();
    assert!(
        events[..first_finish]
            .iter()
            .any(|event| event.1 == "program-exit")
    );
    assert!(
        events[first_finish..]
            .iter()
            .all(|event| event.1 == "finish")
    );
    assert_eq!(
        events[first_finish..]
            .iter()
            .map(|event| event.0)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
}

#[test]
fn pruning_and_child_routing_are_collector_local_and_restore_at_siblings() {
    let source =
        "void hidden() { var x = 1 + 2; } void shown([int p = 3]) { var y = 4 + 5; var z = 6; }";
    let (_, events) = run(source, &[0, 1, 2]);
    let hidden = source.find("1 + 2").unwrap();
    let parameter = source.find("3]").unwrap();
    let binary = source.find("4 + 5").unwrap();
    let right = source.find("5;").unwrap();
    let sibling = source.find("6;").unwrap();
    assert!(keys(&events, 0, "expr").contains(&hidden));
    assert!(!keys(&events, 1, "expr").contains(&hidden));
    assert!(keys(&events, 1, "expr").contains(&binary));
    assert_eq!(
        keys(&events, 1, "function"),
        keys(&events, 1, "function-exit")
    );
    assert!(keys(&events, 0, "expr").contains(&parameter));
    assert!(!keys(&events, 2, "expr").contains(&parameter));
    assert!(keys(&events, 2, "params").is_empty());
    assert!(keys(&events, 2, "params-exit").is_empty());
    assert!(keys(&events, 2, "expr").contains(&binary));
    assert!(keys(&events, 2, "expr-exit").contains(&binary));
    assert!(!keys(&events, 2, "expr").contains(&right));
    assert!(keys(&events, 2, "expr").contains(&sibling));
}

#[test]
fn standalone_and_registry_finalize_once_even_for_empty_programs() {
    for source in ["", "void f() { print(1); }"] {
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree: falcon_dart_parser::parse(source).tree,
        });
        let config = FalconConfig::default();
        let context = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
        let (direct_events, receiver) = mpsc::channel();
        let rule = TraceRule {
            id: 0,
            policy: 0,
            events: direct_events.clone(),
        };
        let standalone = rule.analyze(parsed.program(), &context);
        let (registered, registry_events) = run(source, &[0]);
        assert_eq!(standalone.len(), 1);
        assert_eq!(standalone[0].message, registered[0].message);
        let direct: Vec<_> = receiver.try_iter().collect();
        let registry = registry_events;
        let without_node_ids = |events: &[(usize, &'static str, usize)]| {
            events
                .iter()
                .copied()
                .filter(|event| !event.1.ends_with("node-id") && !event.1.starts_with("params"))
                .collect::<Vec<_>>()
        };
        assert_eq!(without_node_ids(&direct), without_node_ids(&registry));
        assert_eq!(keys(&direct, 0, "prepare"), vec![0]);
        assert_eq!(keys(&direct, 0, "finish"), vec![0]);
        assert_eq!(keys(&direct, 0, "program-exit"), vec![0]);
    }
}

#[test]
fn direct_trait_implementations_receive_all_hooks_without_subscriptions() {
    struct DirectRule(Events);
    struct DirectCollector(Events);
    impl<'a> StructuralRuleVisitor<'a> for DirectCollector {
        fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
            self.0.send((0, "expr", node.span().start)).unwrap();
            StructuralTraversal::Continue
        }

        fn exit_expr(&mut self, node: Expr<'a>) {
            self.0.send((0, "expr-exit", node.span().start)).unwrap();
        }

        fn finish(self: Box<Self>) -> Vec<Diagnostic> {
            Vec::new()
        }
    }
    impl Rule for DirectRule {
        fn prepare<'a>(
            &self,
            _program: Program<'a>,
            _ctx: &'a AnalyzeContext<'a>,
        ) -> RulePreparation<'a> {
            let collector = DirectCollector(self.0.clone());
            assert!(!collector.has_derived_interests());
            RulePreparation::Structural(Box::new(collector))
        }
    }
    let source = "var x = 1 + 2;";
    let parsed = Arc::new(ParsedFile {
        source: source.into(),
        tree: falcon_dart_parser::parse(source).tree,
    });
    let config = FalconConfig::default();
    let context = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
    let (events, receiver) = mpsc::channel();
    DirectRule(events.clone()).analyze(parsed.program(), &context);
    let events: Vec<_> = receiver.try_iter().collect();
    assert_eq!(keys(&events, 0, "expr").len(), 3);
    assert_eq!(keys(&events, 0, "expr-exit").len(), 3);
    let derived = TraceCollector {
        id: 0,
        policy: 0,
        events: mpsc::channel().0,
        functions: Vec::new(),
    };
    assert!(derived.has_derived_interests());
}
