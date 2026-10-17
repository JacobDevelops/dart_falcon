//! Flags null checks of the parameter inside a custom `operator ==`.
//!
//! Dart never calls a user-defined `==` with `null`: `a == null` is answered by
//! the language before the operator runs. Declaring the parameter as nullable
//! (`Object? other`) and then comparing it with `null`, applying `??`, or using
//! `?.` on it is dead code.
//!
//! ```dart
//! class Person {
//!   final String? name;
//!   @override
//!   bool operator ==(Object? other) =>
//!       other != null && other is Person && name == other.name; // flagged
//! }
//! ```
//!
//! Only operators whose single parameter is explicitly declared nullable are
//! inspected; `dynamic` is excluded, matching the analyzer's nullability suffix
//! check. An operator body that rebinds the parameter name is skipped.
//!
//! Adopted from the official `avoid_null_checks_in_equality_operators` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

const MESSAGE: &str = "Unnecessary null comparison in implementation of '=='.";

#[derive(Default)]
pub struct AvoidNullChecksInEqualityOperators;

impl Rule for AvoidNullChecksInEqualityOperators {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            active: None,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    active: Option<OperatorChecks<'a>>,
    diags: Vec<Diagnostic>,
}

struct OperatorChecks<'a> {
    name: &'a str,
    body: Span,
    rebound: bool,
    spans: Vec<Span>,
    method_properties: Vec<Span>,
}

impl OperatorChecks<'_> {
    fn contains(&self, span: Span) -> bool {
        self.body.start <= span.start && span.end <= self.body.end
    }

    fn check(&mut self, name: &Identifier) {
        self.rebound |= name.name == self.name;
    }

    fn check_params(&mut self, params: &FormalParamList) {
        for param in &params.params {
            self.check(&param.name);
        }
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn visit_operator_decl(&mut self, node: &OperatorDecl) -> StructuralTraversal {
        let (Some(body), [param]) = (&node.body, node.params.params.as_slice()) else {
            return StructuralTraversal::SkipChildren;
        };
        if node.op != "==" || !param.param_type.as_ref().is_some_and(DartType::is_nullable) {
            return StructuralTraversal::SkipChildren;
        }
        self.active = Some(OperatorChecks {
            name: &self.ctx.source[param.name.span.start..param.name.span.end],
            body: *body.span(),
            rebound: false,
            spans: Vec::new(),
            method_properties: Vec::new(),
        });
        StructuralTraversal::Continue
    }

    fn exit_operator_decl(&mut self, _node: &OperatorDecl) {
        if let Some(checks) = self.active.take()
            && !checks.rebound
        {
            self.diags.extend(checks.spans.into_iter().map(|span| {
                self.ctx.diagnostic("avoid-null-checks-in-equality-operators", MESSAGE, span)
            }));
        }
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        let Some(checks) = self.active.as_mut().filter(|checks| checks.contains(*node.span())) else {
            return StructuralTraversal::Continue;
        };
        match node {
            Stmt::LocalVar(declaration) => {
                for declarator in &declaration.declarators {
                    checks.check(&declarator.name);
                }
            }
            Stmt::LocalFunc(function) => {
                checks.check(&function.name);
                checks.check_params(&function.params);
            }
            Stmt::For(ForStmt { header: ForHeader { init: Some(init), .. }, .. }) => match init {
                ForInit::VarDecl(declaration) => {
                    for declarator in &declaration.declarators {
                        checks.check(&declarator.name);
                    }
                }
                ForInit::ForIn { name, .. } => checks.check(name),
                ForInit::PatternForIn { .. } | ForInit::Exprs(_) => {}
            },
            Stmt::TryCatch(statement) => {
                for clause in &statement.catches {
                    for variable in [&clause.exception_var, &clause.stack_trace_var].into_iter().flatten() {
                        checks.check(variable);
                    }
                }
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        let Some(checks) = self.active.as_mut().filter(|checks| checks.contains(*node.span())) else {
            return StructuralTraversal::Continue;
        };
        if let Expr::FuncExpr { params, .. } = node {
            checks.check_params(params);
        }
        if let Expr::Call { callee, span, .. } = node
            && let Expr::Field { object, is_null_safe: true, .. } = callee.as_ref()
            && is_param(object, checks.name)
        {
            checks.spans.push(*span);
            checks.method_properties.push(*callee.span());
            return StructuralTraversal::Continue;
        }
        if checks.method_properties.last() == Some(node.span()) {
            checks.method_properties.pop();
            return StructuralTraversal::Continue;
        }
        let flagged = match node {
            Expr::Binary { op: BinaryOp::EqEq | BinaryOp::NotEq, left, right, .. } => {
                (matches!(left.unparenthesized(), Expr::NullLit { .. }) && is_param(right, checks.name))
                    || (matches!(right.unparenthesized(), Expr::NullLit { .. }) && is_param(left, checks.name))
            }
            Expr::Binary { op: BinaryOp::NullCoalesce, left, .. } => is_param(left, checks.name),
            Expr::Field { object, is_null_safe: true, .. } => is_param(object, checks.name),
            _ => false,
        };
        if flagged {
            checks.spans.push(*node.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_pattern(&mut self, node: &Pattern) -> StructuralTraversal {
        if let Some(checks) = self.active.as_mut().filter(|checks| checks.contains(*node.span()))
            && let Pattern::Variable { name, .. } = node
        {
            checks.check(name);
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn is_param(expr: &Expr, name: &str) -> bool {
    matches!(expr.unparenthesized(), Expr::Ident(identifier) if identifier.name == name)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;

    use super::*;
    use falcon_analyze::RuleAnalysis;

    #[test]
    fn includes_method_arguments_without_extending_property_accesses() {
        let source = "class Box { bool operator ==(Object? other) => other?.toString() == other?.hashCode.toString(); }";
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
            ..
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config, &syntax);
        let diagnostics = AvoidNullChecksInEqualityOperators.analyze(&program, &ctx);
        let slices: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(slices, ["other?.toString()", "other?.hashCode"]);
    }
}
