//! Don't reassign parameters of functions, methods, or constructors.
//!
//! Rebinding a parameter makes the body harder to follow: the name no longer
//! means what the caller passed. Assignments (`x = ...`, `x += ...`),
//! increments and decrements, and pattern assignments (`(x, y) = ...`) that
//! write a parameter are flagged, including writes from closures inside the
//! body. One exception mirrors the analyzer: an optional parameter without a
//! default value (so it defaults to `null`) may be given a default once with
//! `x ??= value`; a second `??=` or any other assignment is still flagged.
//! The analyzer also exempts increments/decrements of optional parameters
//! without defaults, and this rule retains that exception.
//!
//! Like the analyzer's visitor, the search does not descend into an assignment
//! expression unless that assignment itself writes the parameter and the
//! parameter defaults to `null`: writes nested in any other assignment
//! (`target[offset++] = v`, `x = p++`, `c.onCancel = () { p = 0; }`,
//! `a..v = (p = 2)`) are not reported. A constructor's parameters are searched
//! for in its body only, not in its initializer list.
//!
//! ```dart
//! void f(int x) { x = 2; } // flagged
//! void g([int? x]) { x ??= 2; } // ok
//! ```
//!
//! Adopted from the official `parameter_assignments` lint.

use std::collections::HashMap;

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use falcon_analyze::resolve::lexical_usage::{LocalWrite, LocalWriteKind};

const RULE: &str = "parameter-assignments";

#[derive(Default)]
pub struct ParameterAssignments;

impl Rule for ParameterAssignments {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut writes: HashMap<usize, Vec<&LocalWrite>> = HashMap::new();
        let mut assignment_targets = HashMap::new();
        for write in ctx.local_writes(program) {
            if let LocalWriteKind::Assign(_) = write.kind {
                assignment_targets.insert(write.span, write.declaration);
            }
            writes.entry(write.declaration).or_default().push(write);
        }
        if writes.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            writes,
            assignments: Vec::new(),
            assignment_targets,
            parameters: Vec::new(),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    writes: HashMap<usize, Vec<&'a LocalWrite>>,
    /// Spans of every assignment expression, sorted by start.
    assignments: Vec<Span>,
    /// The local declaration each assignment writes, when its target is one.
    assignment_targets: HashMap<Span, usize>,
    diagnostics: Vec<Diagnostic>,
    parameters: Vec<(Span, bool, Option<Span>)>,
}

impl Collector<'_, '_> {
    /// Whether the analyzer's per-parameter visitor never reaches `write`: an
    /// enclosing assignment inside the parameter's function stops its descent
    /// unless it writes the parameter itself and the parameter defaults to
    /// `null`.
    fn is_hidden(&self, write: &LocalWrite, defaults_to_null: bool) -> bool {
        let first = self
            .assignments
            .partition_point(|span| span.start <= write.declaration);
        let last = self
            .assignments
            .partition_point(|span| span.start <= write.span.start);
        self.assignments[first..last].iter().any(|span| {
            *span != write.span
                && span.end >= write.span.end
                && !(defaults_to_null
                    && self.assignment_targets.get(span) == Some(&write.declaration))
        })
    }

    /// Reports the writes of parameter `name` inside `scope` (the whole
    /// function when `None`).
    fn report_param(&mut self, name: Span, defaults_to_null: bool, scope: Option<Span>) {
        let Some(writes) = self.writes.remove(&name.start) else {
            return;
        };
        let mut defaulted = false;
        for write in writes {
            let outside = scope
                .is_some_and(|scope| write.span.start < scope.start || write.span.end > scope.end);
            if outside || self.is_hidden(write, defaults_to_null) {
                continue;
            }
            let report = match write.kind {
                LocalWriteKind::Assign(AssignOp::NullCoalesceEq) if defaults_to_null => {
                    std::mem::replace(&mut defaulted, true)
                }
                LocalWriteKind::Assign(_) => true,
                LocalWriteKind::IncDec => !defaults_to_null,
                LocalWriteKind::Pattern { direct } => direct,
                // The analyzer's rule does not visit for-in loop variables.
                LocalWriteKind::ForIn => false,
            };
            if report {
                self.diagnostics.push(self.ctx.diagnostic(
                    RULE,
                    format!(
                        "Invalid assignment to the parameter '{}'.",
                        &self.ctx.source[name.start..name.end]
                    ),
                    write.span,
                ));
            }
        }
    }

    fn check_param(&mut self, name: &Identifier, defaults_to_null: bool, scope: Option<Span>) {
        if self.writes.contains_key(&name.span.start) {
            self.parameters.push((name.span, defaults_to_null, scope));
        }
    }

    fn check(&mut self, params: &FormalParamList, scope: Option<Span>) {
        for param in &params.params {
            let optional = match param.kind {
                ParamKind::Positional | ParamKind::Named { required: true } => false,
                ParamKind::OptionalPositional | ParamKind::Named { required: false } => true,
            };
            self.check_param(
                &param.name,
                optional && param.default_value.is_none(),
                scope,
            );
        }
    }
}

structural_visitor! {
impl StructuralRuleVisitor for Collector<'_, '_> {
    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        self.assignments.sort_unstable_by_key(|span| span.start);
        for (name, defaults_to_null, scope) in std::mem::take(&mut self.parameters) {
            self.report_param(name, defaults_to_null, scope);
        }
        self.diagnostics
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) -> StructuralTraversal {
        if let Some(params) = node.params() {
            self.check(params, None);
        }
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) -> StructuralTraversal {
        self.check(&node.params, None);
        StructuralTraversal::Continue
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) -> StructuralTraversal {
        self.check_param(&node.param, false, None);
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        if let ClassMember::Operator(operator) = node {
            self.check(&operator.params, None);
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'_>) -> StructuralTraversal {
        let body = node
            .body
            .map_or(Span::new(node.span.end, node.span.end), |body| *body.span());
        self.check(node.params, Some(body));
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node {
            self.check(&function.params, None);
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        match node {
            Expr::Assign { span, .. } => self.assignments.push(*span),
            Expr::Cascade { sections, .. } => self.assignments.extend(
                sections.iter().filter(|section| matches!(
                    section.ops.last(),
                    Some(CascadeOp::Assign(..) | CascadeOp::IndexAssign(..))
                )).map(|section| section.span),
            ),
            _ => {}
        }
        if let Expr::FuncExpr { params, .. } = node {
            self.check(params, None);
        }
        StructuralTraversal::Continue
    }
}
}
