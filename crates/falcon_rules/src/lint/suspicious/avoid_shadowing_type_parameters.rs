//! Flags a type parameter that shadows a type parameter of an enclosing
//! declaration.
//!
//! When a nested declaration — a method, local function, function expression, or
//! generic function type or typedef — reuses the name of a type parameter from
//! its surrounding class or function, the inner name hides the outer one. Code in
//! the nested scope can no longer refer to the enclosing type, and a reader can
//! easily mistake the two unrelated types for the same one, which invites subtle
//! type errors. Rename the inner parameter to something distinct.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_dart_type, walk_expr, walk_formal_param, walk_stmt,
    walk_top_level_decl,
};

#[derive(Default)]
pub struct AvoidShadowingTypeParameters;

impl Rule for AvoidShadowingTypeParameters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut checker = Checker {
            diags: Vec::new(),
            ctx,
            scope: Vec::new(),
        };
        checker.visit_program(program);
        checker.diags
    }
}

struct Checker<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
    scope: Vec<(String, &'static str)>,
}

impl Checker<'_, '_> {
    fn enter(&mut self, tps: &[TypeParam], kind: &'static str) -> usize {
        for tp in tps {
            for (_, ancestor_kind) in self.scope.iter().filter(|(name, _)| name == &tp.name.name) {
                self.diags.push(self.ctx.diagnostic(
                    "avoid-shadowing-type-parameters",
                    format!(
                        "The type parameter '{}' shadows a type parameter from the enclosing {}.",
                        tp.name.name, ancestor_kind
                    ),
                    tp.name.span,
                ));
            }
        }
        self.scope
            .extend(tps.iter().map(|tp| (tp.name.name.clone(), kind)));
        tps.len()
    }

    fn scoped(&mut self, tps: &[TypeParam], kind: &'static str, walk: impl FnOnce(&mut Self)) {
        let entered = self.enter(tps, kind);
        walk(self);
        self.scope.truncate(self.scope.len() - entered);
    }
}

impl Visitor for Checker<'_, '_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        let (type_params, kind): (&[TypeParam], _) = match node {
            TopLevelDecl::Class(x) => (&x.type_params, "class"),
            TopLevelDecl::ClassTypeAlias(x) => (&x.type_params, "class"),
            TopLevelDecl::Mixin(x) => (&x.type_params, "mixin"),
            TopLevelDecl::MixinClass(x) => (&x.type_params, "mixin"),
            TopLevelDecl::Enum(x) => (&x.type_params, "enum"),
            TopLevelDecl::Extension(x) => (&x.type_params, "extension"),
            TopLevelDecl::ExtensionType(x) => (&x.type_params, "extension type"),
            TopLevelDecl::Function(x) => (&x.type_params, "function"),
            TopLevelDecl::TypeAlias(x) => (&x.type_params, "typedef"),
            TopLevelDecl::Variable(_) | TopLevelDecl::Error(_) => (&[], "declaration"),
        };
        self.scoped(type_params, kind, |this| walk_top_level_decl(this, node));
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        let type_params: &[TypeParam] = match node {
            ClassMember::Method(x) => &x.type_params,
            ClassMember::Field(_)
            | ClassMember::Constructor(_)
            | ClassMember::PrimaryConstructorBody(_)
            | ClassMember::Getter(_)
            | ClassMember::Setter(_)
            | ClassMember::Operator(_)
            | ClassMember::Error(_) => &[],
        };
        // Upstream exempts the static method's own parameters, but nested
        // declarations still check all syntactic ancestors, including the class.
        if matches!(node, ClassMember::Method(method) if method.is_static) {
            self.scope.extend(
                type_params
                    .iter()
                    .map(|parameter| (parameter.name.name.clone(), "method")),
            );
            walk_class_member(self, node);
            self.scope.truncate(self.scope.len() - type_params.len());
        } else {
            self.scoped(type_params, "method", |this| walk_class_member(this, node));
        }
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalFunc(local) = node {
            self.scoped(&local.type_params, "function", |this| walk_stmt(this, node));
        } else {
            walk_stmt(self, node);
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr { type_params, .. } = node {
            self.scoped(type_params, "function", |this| walk_expr(this, node));
        } else {
            walk_expr(self, node);
        }
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        self.scoped(&node.type_params, "parameter", |this| {
            walk_formal_param(this, node)
        });
    }

    fn visit_dart_type(&mut self, node: &DartType) {
        if let DartType::Function(function) = node {
            self.scoped(&function.type_params, "function", |this| {
                walk_dart_type(this, node)
            });
        } else {
            walk_dart_type(self, node);
        }
    }
}
