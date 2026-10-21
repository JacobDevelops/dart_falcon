//! Flags a required named parameter declared after an optional named one.
//!
//! Callers scanning a signature should see the parameters they must pass
//! first. Within the named parameters of any parameter list — functions,
//! methods, constructors, function literals, function-typed parameters, and
//! `Function(...)` types — every `required` (or `@required`) parameter must
//! precede the optional ones:
//!
//! ```dart
//! void f({int? a, required int b}) {}   // `b` flagged
//! void g({required int b, int? a}) {}   // fine
//! ```
//!
//! Optional named super parameters (`{super.key, required this.child}`) count
//! as optional parameters like any other.
//!
//! Adopted from the official `always_put_required_named_parameters_first` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

const MESSAGE: &str = "Required named parameters should be before optional named parameters.";

#[derive(Default)]
pub struct AlwaysPutRequiredNamedParametersFirst;

impl Rule for AlwaysPutRequiredNamedParametersFirst {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
}

/// `@required` from `package:meta`, possibly import-prefixed.
fn has_required_annotation<'a>(mut annotations: AstChildren<'a, Annotation<'a>>) -> bool {
    annotations.any(|annotation| {
        annotation.args().is_none()
            && annotation.constructor_name().is_none()
            && annotation
                .name()
                .last()
                .is_some_and(|name| name.name() == "required")
    })
}

impl Collector<'_, '_> {
    fn check(&mut self, params: FormalParamList<'_>) {
        let mut optional_seen = false;
        for param in params.named() {
            let required = matches!(param.kind(), ParamKind::Named { required: true })
                || has_required_annotation(param.annotations());
            if required {
                if optional_seen {
                    self.diags
                        .push(self.ctx.diagnostic(MESSAGE, param.name().span()));
                }
            } else {
                optional_seen = true;
            }
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

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
        // The shared alias callback visits legacy formals once, separately
        // from the return type and modern function-type syntax.
        if let Some(params) = node.legacy_params() {
            self.check(params);
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        if let ClassMember::Operator(operator) = node {
            self.check(operator.params());
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(func) = node {
            self.check(func.params());
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
            let mut optional_seen = false;
            for param in function.params().filter(|param| param.is_named()) {
                if param.is_required() {
                    if optional_seen && let Some(name) = param.name() {
                        self.diags
                            .push(self.ctx.diagnostic(MESSAGE, name.span()));
                    }
                } else {
                    optional_seen = true;
                }
            }
        }
        StructuralTraversal::Continue
    }
}
}
