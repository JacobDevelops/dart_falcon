//! Requires named parameters to follow the explicitly configured alphabetical order.
//! Positional parameters are excluded. Set `order` to `ascending` or `descending`;
//! without a valid order this rule does nothing.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::ordering::Order;

const RULE: &str = "parameters-ordering";

#[derive(Default)]
pub struct ParametersOrdering;

impl Rule for ParametersOrdering {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(order) = Order::configured(ctx, RULE) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            order,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    order: Order,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn check_names<'a>(&mut self, names: impl Iterator<Item = Identifier<'a>>) {
        self.order
            .check_names(self.ctx, RULE, names.map(Some), &mut self.diagnostics);
    }

    fn check(&mut self, params: FormalParamList<'_>) {
        self.check_names(params.named().map(|param| param.name()));
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        if let Some(params) = node.params() {
            self.check(params);
        }
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) -> StructuralTraversal {
        self.check(node.params());
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'a>) -> StructuralTraversal {
        self.check(node.params);
        StructuralTraversal::Continue
    }

    fn visit_type_alias_decl(&mut self, node: TypeAliasDecl<'a>) -> StructuralTraversal {
        if let Some(params) = node.legacy_params() {
            self.check(params);
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node {
            self.check(function.params());
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::FuncExpr(function) = node {
            self.check(function.params());
        }
        StructuralTraversal::Continue
    }

    fn visit_formal_param(&mut self, node: FormalParam<'a>) -> StructuralTraversal {
        if let Some(params) = node.function_params() {
            self.check(params);
        }
        StructuralTraversal::Continue
    }

    fn visit_dart_type(&mut self, node: DartType<'a>) -> StructuralTraversal {
        if let DartType::Function(function) = node {
            self.check_names(function.params().filter(|param| param.is_named()).filter_map(|param| param.name()));
        }
        StructuralTraversal::Continue
    }
}
}
