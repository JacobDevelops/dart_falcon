//! Order existing superclass calls in resolved Flutter `State` lifecycle methods,
//! including mixins whose `on` constraints are `State` or its subtypes.
//!
//! Calls to `super.initState()`, `super.didUpdateWidget()`, and `super.activate()`
//! belong in the first direct statement. `didChangeDependencies` is not checked.
//! Calls to `super.deactivate()` and `super.dispose()` belong in the last direct
//! statement, also allowing the final statement of a terminal `finally` block.
//! Nested conditional calls do not satisfy this ordering; callbacks and local
//! functions are excluded.
//!
//! Missing calls are not reported for any lifecycle method. The existing
//! `proper-super-init-state` and `require-super-dispose` rules require presence
//! only for `initState` and `dispose`, respectively.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralRuleVisitor, StructuralTraversal, Truth, TypeRelation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

/// Enforces boundary ordering of existing Flutter `State` lifecycle super calls.
#[derive(Default)]
pub struct ProperSuperCalls;

impl Rule for ProperSuperCalls {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            signatures,
            state: DeclarationIdentity::flutter("State"),
            lifecycle: None,
            diagnostics: Vec::new(),
        }))
    }
}

struct Lifecycle {
    name: &'static str,
    first: bool,
    boundary: Option<Span>,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    state: DeclarationIdentity,
    lifecycle: Option<Lifecycle>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let name = match declaration {
            TopLevelDecl::Class(class) => class.name(),
            TopLevelDecl::Mixin(mixin) => {
                return if mixin.on_clause().any(|constraint| {
                    self.signatures.relation(
                        &self.model.resolve_type(constraint),
                        TypeRelation::SubtypeOfDeclaration(&self.state),
                        &self.model,
                    ) == Truth::Yes
                }) {
                    StructuralTraversal::Continue
                } else {
                    StructuralTraversal::SkipChildren
                };
            }
            _ => return StructuralTraversal::SkipChildren,
        };
        let Some(identity) = self.model.resolve_name(name.name()) else {
            return StructuralTraversal::SkipChildren;
        };
        if self.signatures.relation(
            &ResolvedType::interface_identity(identity),
            TypeRelation::SubtypeOfDeclaration(&self.state),
            &self.model,
        ) != Truth::Yes {
            return StructuralTraversal::SkipChildren;
        }
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, method: MethodDecl<'a>) -> StructuralTraversal {
        let (name, first) = match method.name().name() {
            "initState" => ("initState", true),
            "didUpdateWidget" => ("didUpdateWidget", true),
            "activate" => ("activate", true),
            "dispose" => ("dispose", false),
            "deactivate" => ("deactivate", false),
            _ => return StructuralTraversal::SkipChildren,
        };
        let Some(FunctionBody::Block(block)) = method.body() else {
            return StructuralTraversal::SkipChildren;
        };
        let mut boundary = if first {
            block.stmts().next()
        } else {
            block.stmts().last()
        };
        if !first {
            while let Some(Stmt::TryCatch(statement)) = boundary {
                boundary = statement.finally().and_then(|block| block.stmts().last());
            }
        }
        self.lifecycle = Some(Lifecycle {
            name,
            first,
            boundary: boundary.and_then(|statement| match statement {
                Stmt::Expr(statement) => Some(statement.expr().unparenthesized().span()),
                _ => None,
            }),
        });
        StructuralTraversal::Continue
    }

    fn exit_method_decl(&mut self, _method: MethodDecl<'a>) {
        self.lifecycle = None;
    }

    fn visit_stmt(&mut self, statement: Stmt<'a>) -> StructuralTraversal {
        if matches!(statement, Stmt::LocalFunc(_)) {
            return StructuralTraversal::SkipChildren;
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, expression: Expr<'a>) -> StructuralTraversal {
        if matches!(expression, Expr::FuncExpr(_)) {
            return StructuralTraversal::SkipChildren;
        }
        if let Some(lifecycle) = &self.lifecycle
            && let Expr::Call(call) = expression
            && let Expr::Field(field) = call.callee().unparenthesized()
            && matches!(field.object().unparenthesized(), Expr::Super(_))
            && field.field().name() == lifecycle.name
            && lifecycle.boundary != Some(call.span())
        {
            let position = if lifecycle.first { "first" } else { "last" };
            self.diagnostics.push(self.ctx.diagnostic(
                format!(
                    "Place `super.{}()` as the {position} statement in `{}()`.",
                    lifecycle.name, lifecycle.name,
                ),
                call.span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}
