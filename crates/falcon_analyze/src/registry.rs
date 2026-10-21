use crate::semantic_scope::{CascadeAssignment, SemanticRuleVisitor, SemanticState, visit_program};
use crate::structural_scope;
use crate::{AnalyzeContext, FileSuppressions, Rule, RuleLookup, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{
    ArgList, Block, CascadeOp, ConstructorDecl, ConstructorRef, Expr, Pattern, Program, Span, Stmt,
};
use std::collections::HashMap;
use tracing::{debug, debug_span};

/// Without metadata, suppression paths validate as unknown rules.
fn no_lookup(_name: &str) -> Option<(&'static str, &'static str, bool)> {
    None
}

const RULES_STACK_SIZE: usize = 256 * 1024 * 1024;

/// Deep legal ASTs can exhaust default worker stacks; all rule-execution paths need this guard.
pub fn with_rules_stack<T: Send>(f: impl FnOnce() -> T + Send) -> std::io::Result<T> {
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .stack_size(RULES_STACK_SIZE)
            .spawn_scoped(scope, f)?;
        Ok(handle.join().expect("rules thread panicked"))
    })
}

pub fn rules_thread_diagnostic(path: &std::path::Path, error: &std::io::Error) -> Diagnostic {
    Diagnostic::new(
        falcon_diagnostics::RULES_THREAD_ERROR_DIAGNOSTIC_ID,
        falcon_diagnostics::Severity::Error,
        format!("could not start rules thread: {error}"),
        path.to_string_lossy().into_owned(),
        falcon_diagnostics::Span { start: 0, end: 0 },
    )
}

#[test]
fn rules_thread_failure_has_a_distinct_error_marker() {
    let diagnostic = rules_thread_diagnostic(
        std::path::Path::new("main.dart"),
        &std::io::Error::other("thread limit"),
    );
    assert_eq!(
        diagnostic.rule,
        falcon_diagnostics::RULES_THREAD_ERROR_DIAGNOSTIC_ID
    );
    assert_eq!(diagnostic.severity, falcon_diagnostics::Severity::Error);
}

pub struct RuleRegistry {
    rules: Vec<(&'static str, Box<dyn Rule>)>,
    lookup: RuleLookup,
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleRegistry {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            lookup: no_lookup,
        }
    }

    pub fn with_lookup(lookup: RuleLookup) -> Self {
        Self {
            rules: Vec::new(),
            lookup,
        }
    }

    /// Register the canonical name supplied by the caller's metadata table.
    pub fn register(&mut self, name: &'static str, rule: Box<dyn Rule>) {
        self.rules.push((name, rule));
    }

    pub fn rules(&self) -> &[(&'static str, Box<dyn Rule>)] {
        &self.rules
    }

    pub fn run_all<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> Vec<Diagnostic> {
        with_rules_stack(|| self.run_all_inner(program, ctx))
            .unwrap_or_else(|error| vec![rules_thread_diagnostic(ctx.file_path(), &error)])
    }

    fn run_all_inner<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> Vec<Diagnostic> {
        let mut diagnostics_by_rule = Vec::with_capacity(self.rules.len());
        let mut visitors = SemanticVisitors(Vec::with_capacity(self.rules.len()));
        let mut structural_visitors = Vec::with_capacity(self.rules.len());
        let mut structural_indices = Vec::with_capacity(self.rules.len());
        for (index, (name, rule)) in self.rules.iter().enumerate() {
            let span = debug_span!("run_rule", rule = name);
            let _enter = span.enter();
            debug!(rule = name, "running rule");
            match rule.prepare(program, ctx) {
                RulePreparation::Diagnostics(diagnostics) => diagnostics_by_rule.push(diagnostics),
                RulePreparation::Semantic(visitor) => {
                    diagnostics_by_rule.push(Vec::new());
                    visitors.0.push((index, visitor));
                }
                RulePreparation::Structural(visitor) => {
                    diagnostics_by_rule.push(Vec::new());
                    structural_indices.push(index);
                    structural_visitors.push(visitor);
                }
            }
        }
        for (index, diagnostics) in
            structural_indices
                .into_iter()
                .zip(structural_scope::visit_program(
                    program,
                    structural_visitors,
                ))
        {
            diagnostics_by_rule[index].extend(diagnostics);
        }
        if !visitors.0.is_empty() {
            if let Some(state) = SemanticState::new(program, ctx) {
                visit_program(&mut visitors, program, state);
            }
            for (index, visitor) in visitors.0 {
                diagnostics_by_rule[index].extend(visitor.finish());
            }
        }
        let diagnostics: Vec<Diagnostic> = self
            .rules
            .iter()
            .zip(diagnostics_by_rule)
            .flat_map(|((name, _), diagnostics)| {
                diagnostics.into_iter().map(move |mut diagnostic| {
                    diagnostic.rule = name;
                    diagnostic
                })
            })
            .collect();

        // Parse even without rule diagnostics so malformed suppressions are surfaced.
        let mut diagnostics = diagnostics;
        let suppressions = FileSuppressions::parse(
            ctx.source(),
            &ctx.file_path().to_string_lossy(),
            self.lookup,
            ctx.tree(),
        );
        if !suppressions.is_empty() {
            diagnostics.retain(|diag| {
                let line = suppressions.line_for_offset(diag.span.start);
                !suppressions.is_suppressed(diag.rule, line)
            });
        }
        // Append after filtering so malformed suppressions cannot suppress themselves.
        diagnostics.extend(suppressions.into_diagnostics());
        let config_errors = ctx.take_config_errors();
        if !config_errors.is_empty() {
            return config_errors;
        }
        diagnostics.sort_by(|left, right| {
            left.rule
                .cmp(right.rule)
                .then(left.span.start.cmp(&right.span.start))
                .then(left.span.end.cmp(&right.span.end))
                .then(left.message.cmp(&right.message))
        });
        diagnostics.dedup_by(|left, right| {
            left.rule == right.rule && left.span == right.span && left.message == right.message
        });
        diagnostics
    }
}

/// Callback fanout only; the semantic walker owns recursion and shared state.
struct SemanticVisitors<'a>(Vec<(usize, Box<dyn SemanticRuleVisitor<'a> + 'a>)>);

impl<'a> SemanticRuleVisitor<'a> for SemanticVisitors<'a> {
    fn visit_program_end(&mut self, state: &SemanticState<'a>) {
        for (_, visitor) in &mut self.0 {
            visitor.visit_program_end(state);
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        for (_, visitor) in &mut self.0 {
            visitor.visit_expr(node, state);
        }
    }

    fn visit_call_arguments(
        &mut self,
        args: ArgList<'a>,
        positional: &[crate::ResolvedType],
        named: &HashMap<String, crate::ResolvedType>,
        state: &SemanticState<'a>,
    ) {
        for (_, visitor) in &mut self.0 {
            visitor.visit_call_arguments(args, positional, named, state);
        }
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        for (_, visitor) in &mut self.0 {
            visitor.visit_stmt(node, state);
        }
    }

    fn visit_block_end(&mut self, node: Block<'a>, state: &SemanticState<'a>) {
        for (_, visitor) in &mut self.0 {
            visitor.visit_block_end(node, state);
        }
    }

    fn visit_constructor(&mut self, node: ConstructorDecl<'a>, state: &SemanticState<'a>) {
        for (_, visitor) in &mut self.0 {
            visitor.visit_constructor(node, state);
        }
    }

    fn visit_constructor_ref(&mut self, node: ConstructorRef<'a>, state: &SemanticState<'a>) {
        for (_, visitor) in &mut self.0 {
            visitor.visit_constructor_ref(node.clone(), state);
        }
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &crate::ResolvedType,
        section: Span,
        state: &SemanticState<'a>,
    ) {
        for (_, visitor) in &mut self.0 {
            visitor.visit_cascade_operation(operation, receiver, section, state);
        }
    }

    fn visit_cascade_assignment(
        &mut self,
        assignment: CascadeAssignment<'a>,
        receiver: &crate::ResolvedType,
        section: Span,
        state: &SemanticState<'a>,
    ) {
        for (_, visitor) in &mut self.0 {
            visitor.visit_cascade_assignment(assignment, receiver, section, state);
        }
    }

    fn visit_pattern(
        &mut self,
        node: Pattern<'a>,
        matched: Option<&crate::ResolvedType>,
        state: &SemanticState<'a>,
    ) {
        for (_, visitor) in &mut self.0 {
            visitor.visit_pattern(node, matched, state);
        }
    }
}
