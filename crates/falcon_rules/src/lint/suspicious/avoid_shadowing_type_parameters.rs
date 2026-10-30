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
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidShadowingTypeParameters;

impl Rule for AvoidShadowingTypeParameters {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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
    scope: Vec<(&'ctx str, &'static str)>,
    scope_lengths: Vec<usize>,
}

impl<'a> Checker<'_, 'a> {
    fn enter(&mut self, tps: &AstChildren<'a, TypeParam<'a>>, kind: &'static str) -> usize {
        for tp in tps.clone() {
            for (_, ancestor_kind) in self
                .scope
                .iter()
                .filter(|(name, _)| *name == tp.name().name())
            {
                self.diags.push(self.ctx.diagnostic(format!(
                    "The type parameter `{}` shadows a type parameter from the enclosing {}.",
                    tp.name().name(),
                    ancestor_kind
                ), tp.name().span()));
            }
        }
        self.scope
            .extend(tps.clone().map(|tp| (tp.name().name(), kind)));
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
impl<'a> StructuralRuleVisitor<'a> for Checker<'_, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.save_scope();
        let kind = match node {
            TopLevelDecl::Class(_) | TopLevelDecl::ClassTypeAlias(_) => "class",
            TopLevelDecl::Mixin(_) | TopLevelDecl::MixinClass(_) => "mixin",
            TopLevelDecl::Enum(_) => "enum",
            TopLevelDecl::Extension(_) => "extension",
            TopLevelDecl::ExtensionType(_) => "extension type",
            TopLevelDecl::Function(_) => "function",
            TopLevelDecl::TypeAlias(_) => "typedef",
            TopLevelDecl::Variable(_) | TopLevelDecl::Error(_) => return StructuralTraversal::Continue,
        };
        self.enter(&node.type_params(), kind);
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        self.save_scope();
        let ClassMember::Method(method) = node else {
            return StructuralTraversal::Continue;
        };
        let type_params = method.type_params();
        // Upstream exempts the static method's own parameters, but nested
        // declarations still check all syntactic ancestors, including the class.
        if method.is_static() {
            self.scope.extend(
                type_params
                    .map(|parameter| (parameter.name().name(), "method")),
            );
        } else {
            self.enter(&type_params, "method");
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        self.save_scope();
        if let Stmt::LocalFunc(local) = node {
            self.enter(&local.type_params(), "function");
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        self.save_scope();
        if let Expr::FuncExpr(function) = node {
            self.enter(&function.type_params(), "function");
        }
        StructuralTraversal::Continue
    }

    fn visit_formal_param(&mut self, node: FormalParam<'a>) -> StructuralTraversal {
        self.save_scope();
        self.enter(&node.type_params(), "parameter");
        StructuralTraversal::Continue
    }

    fn visit_dart_type(&mut self, node: DartType<'a>) -> StructuralTraversal {
        self.save_scope();
        if let DartType::Function(function) = node {
            self.enter(&function.type_params(), "function");
        }
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, _node: TopLevelDecl<'a>) {
        self.restore_scope();
    }

    fn exit_class_member(&mut self, _node: ClassMember<'a>) {
        self.restore_scope();
    }

    fn exit_stmt(&mut self, _node: Stmt<'a>) {
        self.restore_scope();
    }

    fn exit_expr(&mut self, _node: Expr<'a>) {
        self.restore_scope();
    }

    fn exit_formal_param(&mut self, _node: FormalParam<'a>) {
        self.restore_scope();
    }

    fn exit_dart_type(&mut self, _node: DartType<'a>) {
        self.restore_scope();
    }
}
}
