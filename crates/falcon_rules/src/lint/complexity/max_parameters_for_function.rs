//! Flags callable declarations and function types with more than the configured
//! number of parameters.
//!
//! A long parameter list is hard to call correctly and usually signals that
//! some arguments belong together in an object. The rule counts required,
//! optional-positional, and named parameters on functions, local functions,
//! closures, methods, constructors, operators, setters, and function types.
//!
//! ## Options
//!
//! `max_parameters` (integer, default: 5) — flag when the parameter count
//! exceeds this.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct MaxParametersForFunction;

impl Rule for MaxParametersForFunction {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            threshold: max_parameters_option(ctx),
            ctx,
            diags: Vec::new(),
        }))
    }
}

fn max_parameters_option(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::usize(
        crate::lint::options::for_rule(ctx, "max-parameters-for-function"),
        "max_parameters",
    )
    .unwrap_or(5)
}

struct Collector<'a> {
    threshold: usize,
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn check(&mut self, count: usize, span: &Span) {
        if count <= self.threshold {
            return;
        }
        self.diags.push(self.ctx.diagnostic(
            "max-parameters-for-function",
            format!("Function has too many parameters (max {}).", self.threshold),
            *span,
        ));
    }

    fn check_formals(&mut self, params: &FormalParamList, span: &Span) {
        self.check(params.params.len(), span);
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) -> StructuralTraversal {
        if let Some(params) = node.params() {
            self.check_formals(params, &node.span);
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'_>) -> StructuralTraversal {
        self.check_formals(node.params, &node.span);
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) -> StructuralTraversal {
        self.check_formals(&node.params, &node.span);
        StructuralTraversal::Continue
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) -> StructuralTraversal {
        self.check(1, &node.span);
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        if let ClassMember::Operator(operator) = node {
            self.check_formals(&operator.params, &operator.span);
        }
        StructuralTraversal::Continue
    }

    fn visit_dart_type(&mut self, node: &DartType) -> StructuralTraversal {
        if let DartType::Function(function) = node {
            self.check(function.params.len(), &function.span);
        }
        StructuralTraversal::Continue
    }

    fn visit_formal_param(&mut self, node: &FormalParam) -> StructuralTraversal {
        if let Some(params) = &node.function_params {
            self.check_formals(params, &node.span);
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node {
            self.check_formals(&function.params, &function.span);
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if let Expr::FuncExpr { params, span, .. } = node {
            self.check_formals(params, span);
        }
        StructuralTraversal::Continue
    }
}
}
