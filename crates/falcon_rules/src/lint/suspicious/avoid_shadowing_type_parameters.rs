//! Flags a type parameter that shadows a type parameter of an enclosing
//! declaration.
//!
//! When a nested declaration — a method, local function, function expression, or
//! generic function type or typedef — reuses the name of a type parameter from
//! its surrounding class or function, the inner name hides the outer one. Code in
//! the nested scope can no longer refer to the enclosing type, and a reader can
//! easily mistake the two unrelated types for the same one, which invites subtle
//! type errors. Rename the inner parameter to something distinct.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidShadowingTypeParameters;

impl Rule for AvoidShadowingTypeParameters {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let checker = Checker {
            diags: Vec::new(),
            ctx,
            scope: Vec::new(),
            scope_lengths: Vec::new(),
        };
        RulePreparation::Structural(Box::new(checker))
    }
}

struct Checker<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
    scope: Vec<(String, &'static str)>,
    scope_lengths: Vec<usize>,
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

    fn save_scope(&mut self) {
        self.scope_lengths.push(self.scope.len());
    }

    fn restore_scope(&mut self) {
        self.scope.truncate(self.scope_lengths.pop().unwrap());
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Checker<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
        self.save_scope();
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
        self.enter(type_params, kind);
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        self.save_scope();
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
        } else {
            self.enter(type_params, "method");
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        self.save_scope();
        if let Stmt::LocalFunc(local) = node {
            self.enter(&local.type_params, "function");
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        self.save_scope();
        if let Expr::FuncExpr { type_params, .. } = node {
            self.enter(type_params, "function");
        }
        StructuralTraversal::Continue
    }

    fn visit_formal_param(&mut self, node: &FormalParam) -> StructuralTraversal {
        self.save_scope();
        self.enter(&node.type_params, "parameter");
        StructuralTraversal::Continue
    }

    fn visit_dart_type(&mut self, node: &DartType) -> StructuralTraversal {
        self.save_scope();
        if let DartType::Function(function) = node {
            self.enter(&function.type_params, "function");
        }
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, _node: &TopLevelDecl) {
        self.restore_scope();
    }

    fn exit_class_member(&mut self, _node: &ClassMember) {
        self.restore_scope();
    }

    fn exit_stmt(&mut self, _node: &Stmt) {
        self.restore_scope();
    }

    fn exit_expr(&mut self, _node: &Expr) {
        self.restore_scope();
    }

    fn exit_formal_param(&mut self, _node: &FormalParam) {
        self.restore_scope();
    }

    fn exit_dart_type(&mut self, _node: &DartType) {
        self.restore_scope();
    }
}
}
