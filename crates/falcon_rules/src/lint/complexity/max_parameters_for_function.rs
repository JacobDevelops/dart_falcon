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
//! `max-parameters` (integer, default: 5) — flag when the parameter count
//! exceeds this.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Maximum number of parameters allowed in a function.
    #[serde(deserialize_with = "falcon_config::integer")]
    max_parameters: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { max_parameters: 5 }
    }
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct MaxParametersForFunction;

impl Rule for MaxParametersForFunction {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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
    crate::lint::options::get::<Options>(ctx, MaxParametersForFunction::NAME).max_parameters
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
        self.diags.push(self.ctx.diagnostic(format!("Function has too many parameters (max {}).", self.threshold), *span));
    }

    fn check_formals(&mut self, params: FormalParamList<'_>, span: &Span) {
        self.check(params.params().len(), span);
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        if let Some(params) = node.params() {
            self.check_formals(params, &node.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'a>) -> StructuralTraversal {
        self.check_formals(node.params, &node.span);
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) -> StructuralTraversal {
        self.check_formals(node.params(), &node.span());
        StructuralTraversal::Continue
    }

    fn visit_setter_decl(&mut self, node: SetterDecl<'a>) -> StructuralTraversal {
        self.check(1, &node.span());
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        if let ClassMember::Operator(operator) = node {
            self.check_formals(operator.params(), &operator.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_dart_type(&mut self, node: DartType<'a>) -> StructuralTraversal {
        if let DartType::Function(function) = node {
            self.check(function.params().len(), &function.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_formal_param(&mut self, node: FormalParam<'a>) -> StructuralTraversal {
        if let Some(params) = node.function_params() {
            self.check_formals(params, &node.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node {
            self.check_formals(function.params(), &function.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::FuncExpr(function) = node {
            self.check_formals(function.params(), &function.span());
        }
        StructuralTraversal::Continue
    }
}
}
