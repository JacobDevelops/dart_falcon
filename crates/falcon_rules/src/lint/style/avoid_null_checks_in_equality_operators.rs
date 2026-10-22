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
use falcon_syntax::cst::*;

const MESSAGE: &str = "Remove the unnecessary null comparison from this `==` implementation.";

#[derive(Default)]
pub struct AvoidNullChecksInEqualityOperators;

impl Rule for AvoidNullChecksInEqualityOperators {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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

    fn check(&mut self, name: Identifier<'_>) {
        self.rebound |= name.name() == self.name;
    }

    fn check_params(&mut self, params: FormalParamList<'_>) {
        for param in params.params() {
            self.check(param.name());
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_operator_decl(&mut self, node: OperatorDecl<'a>) -> StructuralTraversal {
        let Some(body) = node.body() else {
            return StructuralTraversal::SkipChildren;
        };
        let mut params = node.params().params();
        let Some(param) = params.next() else {
            return StructuralTraversal::SkipChildren;
        };
        if params.next().is_some() || node.op() != "==" || !param.param_type().is_some_and(DartType::is_nullable) {
            return StructuralTraversal::SkipChildren;
        }
        self.active = Some(OperatorChecks {
            name: param.name().name(),
            body: body.span(),
            rebound: false,
            spans: Vec::new(),
            method_properties: Vec::new(),
        });
        StructuralTraversal::Continue
    }

    fn exit_operator_decl(&mut self, _node: OperatorDecl<'a>) {
        if let Some(checks) = self.active.take()
            && !checks.rebound
        {
            self.diags.extend(checks.spans.into_iter().map(|span| {
                self.ctx.diagnostic(MESSAGE, span)
            }));
        }
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        let Some(checks) = self.active.as_mut().filter(|checks| checks.contains(node.span())) else {
            return StructuralTraversal::Continue;
        };
        match node {
            Stmt::LocalVar(declaration) => {
                for declarator in declaration.declarators() {
                    checks.check(declarator.name());
                }
            }
            Stmt::LocalFunc(function) => {
                checks.check(function.name());
                checks.check_params(function.params());
            }
            Stmt::For(statement) => match statement.header().init() {
                Some(ForInit::VarDecl(declaration)) => {
                    for declarator in declaration.declarators() {
                        checks.check(declarator.name());
                    }
                }
                Some(ForInit::ForIn(initializer)) => checks.check(initializer.name()),
                Some(ForInit::PatternForIn(_) | ForInit::Exprs(_)) | None => {}
            },
            Stmt::TryCatch(statement) => {
                for clause in statement.catches() {
                    for variable in [clause.exception_var(), clause.stack_trace_var()].into_iter().flatten() {
                        checks.check(variable);
                    }
                }
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        let Some(checks) = self.active.as_mut().filter(|checks| checks.contains(node.span())) else {
            return StructuralTraversal::Continue;
        };
        if let Expr::FuncExpr(function) = node {
            checks.check_params(function.params());
        }
        if let Expr::Call(call) = node
            && let Expr::Field(field) = call.callee()
            && field.is_null_safe()
            && is_param(field.object(), checks.name)
        {
            checks.spans.push(call.span());
            checks.method_properties.push(call.callee().span());
            return StructuralTraversal::Continue;
        }
        if checks.method_properties.last() == Some(&node.span()) {
            checks.method_properties.pop();
            return StructuralTraversal::Continue;
        }
        let flagged = match node {
            Expr::Binary(binary) if matches!(binary.op(), BinaryOp::EqEq | BinaryOp::NotEq) => {
                (matches!(binary.left().unparenthesized(), Expr::NullLit(_)) && is_param(binary.right(), checks.name))
                    || (matches!(binary.right().unparenthesized(), Expr::NullLit(_)) && is_param(binary.left(), checks.name))
            }
            Expr::Binary(binary) if binary.op() == BinaryOp::NullCoalesce => is_param(binary.left(), checks.name),
            Expr::Field(field) if field.is_null_safe() => is_param(field.object(), checks.name),
            _ => false,
        };
        if flagged {
            checks.spans.push(node.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_pattern(&mut self, node: Pattern<'a>) -> StructuralTraversal {
        if let Some(checks) = self.active.as_mut().filter(|checks| checks.contains(node.span()))
            && let Pattern::Variable(variable) = node
        {
            checks.check(variable.name());
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn is_param(expr: Expr<'_>, name: &str) -> bool {
    matches!(expr.unparenthesized(), Expr::Ident(identifier) if identifier.name() == name)
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
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let program = parsed.tree.program(source);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
        let diagnostics = AvoidNullChecksInEqualityOperators.analyze(AvoidNullChecksInEqualityOperators::NAME, program, &ctx);
        let slices: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(slices, ["other?.toString()", "other?.hashCode"]);
    }
}
