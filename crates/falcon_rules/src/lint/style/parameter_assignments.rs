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
use falcon_syntax::cst::*;

use falcon_analyze::{LocalWrite, LocalWriteKind};


#[derive(Default)]
pub struct ParameterAssignments;

impl Rule for ParameterAssignments {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut writes: HashMap<NodeId, Vec<&LocalWrite>> = HashMap::new();
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

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    writes: HashMap<NodeId, Vec<&'a LocalWrite>>,
    /// Spans of every assignment expression, sorted by start.
    assignments: Vec<Span>,
    /// The local declaration each assignment writes, when its target is one.
    assignment_targets: HashMap<Span, NodeId>,
    diagnostics: Vec<Diagnostic>,
    parameters: Vec<(Identifier<'a>, bool, Option<Span>)>,
}

impl<'a> Collector<'a> {
    /// Whether the analyzer's per-parameter visitor never reaches `write`: an
    /// enclosing assignment inside the parameter's function stops its descent
    /// unless it writes the parameter itself and the parameter defaults to
    /// `null`.
    fn is_hidden(&self, write: &LocalWrite, defaults_to_null: bool) -> bool {
        let declaration_start = self
            .ctx
            .tree()
            .node(self.ctx.source(), write.declaration)
            .unwrap()
            .span()
            .start;
        let first = self
            .assignments
            .partition_point(|span| span.start <= declaration_start);
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
    fn report_param(
        &mut self,
        parameter: Identifier<'a>,
        defaults_to_null: bool,
        scope: Option<Span>,
    ) {
        let name = parameter.span();
        let Some(writes) = self.writes.remove(&parameter.syntax().id()) else {
            return;
        };
        let mut defaulted = false;
        for write in writes {
            let outside = scope
                .is_some_and(|scope| !scope.contains(write.span));
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
                self.diagnostics.push(self.ctx.diagnostic(format!(
                    "Avoid assigning to the parameter `{}`.",
                    &self.ctx.source()[name.start..name.end]
                ), write.span));
            }
        }
    }

    fn check_param(&mut self, name: Identifier<'a>, defaults_to_null: bool, scope: Option<Span>) {
        if self.writes.contains_key(&name.syntax().id()) {
            self.parameters.push((name, defaults_to_null, scope));
        }
    }

    fn check(&mut self, params: FormalParamList<'a>, scope: Option<Span>) {
        for param in params.params() {
            let optional = match param.kind() {
                ParamKind::Positional | ParamKind::Named { required: true } => false,
                ParamKind::OptionalPositional | ParamKind::Named { required: false } => true,
            };
            self.check_param(
                param.name(),
                optional && param.default_value().is_none(),
                scope,
            );
        }
    }
}

structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        self.assignments.sort_unstable_by_key(|span| span.start);
        for (name, defaults_to_null, scope) in std::mem::take(&mut self.parameters) {
            self.report_param(name, defaults_to_null, scope);
        }
        self.diagnostics
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        if let Some(params) = node.params() {
            self.check(params, None);
        }
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) -> StructuralTraversal {
        self.check(node.params(), None);
        StructuralTraversal::Continue
    }

    fn visit_setter_decl(&mut self, node: SetterDecl<'a>) -> StructuralTraversal {
        self.check_param(node.param(), false, None);
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        if let ClassMember::Operator(operator) = node {
            self.check(operator.params(), None);
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'a>) -> StructuralTraversal {
        let body = node
            .body
            .map_or(Span::new(node.span.end, node.span.end), FunctionBody::span);
        self.check(node.params, Some(body));
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node {
            self.check(function.params(), None);
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        match node {
            Expr::Assign(assignment) => self.assignments.push(assignment.span()),
            Expr::Cascade(cascade) => self.assignments.extend(
                cascade.sections().filter(|section| matches!(
                    section.ops().last(),
                    Some(CascadeOp::Assign(_) | CascadeOp::IndexAssign(_))
                )).map(CascadeSection::span),
            ),
            _ => {}
        }
        if let Expr::FuncExpr(function) = node {
            self.check(function.params(), None);
        }
        StructuralTraversal::Continue
    }
}
}
