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

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_dart_type, walk_expr, walk_formal_param, walk_function_decl,
    walk_method_decl, walk_stmt, walk_type_alias_decl,
};

const RULE: &str = "always-put-required-named-parameters-first";
const MESSAGE: &str = "Required named parameters should be before optional named parameters.";

#[derive(Default)]
pub struct AlwaysPutRequiredNamedParametersFirst;

impl Rule for AlwaysPutRequiredNamedParametersFirst {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diags)
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
}

/// `@required` from `package:meta`, possibly import-prefixed.
fn has_required_annotation(annotations: &[Annotation]) -> bool {
    annotations.iter().any(|annotation| {
        annotation.args.is_none()
            && annotation.constructor_name.is_none()
            && annotation
                .name
                .last()
                .is_some_and(|name| name.name == "required")
    })
}

impl Collector<'_, '_> {
    fn check(&mut self, params: &FormalParamList) {
        let mut optional_seen = false;
        for param in params.named() {
            let required = matches!(param.kind, ParamKind::Named { required: true })
                || has_required_annotation(&param.annotations);
            if required {
                if optional_seen {
                    self.diags
                        .push(self.ctx.diagnostic(RULE, MESSAGE, param.name.span));
                }
            } else {
                optional_seen = true;
            }
        }
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        if let Some(params) = node.params() {
            self.check(params);
        }
        walk_function_decl(self, node);
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.check(&node.params);
        walk_method_decl(self, node);
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'_>) {
        self.check(node.params);
    }

    fn visit_type_alias_decl(&mut self, node: &TypeAliasDecl) {
        // The shared alias callback visits legacy formals once, separately
        // from the return type and modern function-type syntax.
        if let Some(params) = &node.legacy_params {
            self.check(params);
        }
        walk_type_alias_decl(self, node);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Operator(operator) = node {
            self.check(&operator.params);
        }
        walk_class_member(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalFunc(func) = node {
            self.check(&func.params);
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr { params, .. } = node {
            self.check(params);
        }
        walk_expr(self, node);
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        if let Some(params) = &node.function_params {
            self.check(params);
        }
        walk_formal_param(self, node);
    }

    fn visit_dart_type(&mut self, node: &DartType) {
        if let DartType::Function(function) = node {
            let mut optional_seen = false;
            for param in function.params.iter().filter(|param| param.is_named) {
                if param.is_required {
                    if optional_seen && let Some(name) = &param.name {
                        self.diags
                            .push(self.ctx.diagnostic(RULE, MESSAGE, name.span));
                    }
                } else {
                    optional_seen = true;
                }
            }
        }
        walk_dart_type(self, node);
    }
}
