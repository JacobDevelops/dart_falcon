//! Require `super.initState()` to be the first statement of a resolved Flutter
//! `State` subclass's `initState` override.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralRuleVisitor, StructuralTraversal, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct ProperSuperInitState;

impl Rule for ProperSuperInitState {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let state = DeclarationIdentity::flutter("State");
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model,
            signatures,
            state,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    state: DeclarationIdentity,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn visit_top_level_decl(&mut self, declaration: &TopLevelDecl) -> StructuralTraversal {
        let ctx = self.ctx;
        let model = &self.model;
        let signatures = self.signatures;
        let state = &self.state;
        let diags = &mut self.diags;
        let TopLevelDecl::Class(class) = declaration else {
            return StructuralTraversal::SkipChildren;
        };
        let Some(identity) = model.resolve_name(&class.name.name) else {
            return StructuralTraversal::SkipChildren;
        };
        if signatures.is_subtype_of(&ResolvedType::interface_identity(identity), state, model)
            != Truth::Yes
        {
            return StructuralTraversal::SkipChildren;
        }
        for member in &class.members {
            let ClassMember::Method(method) = member else {
                continue;
            };
            if method.name.name != "initState" {
                continue;
            }
            let Some(FunctionBody::Block(block)) = &method.body else {
                continue;
            };
            if block
                .stmts
                .first()
                .is_some_and(is_super_init_state_statement)
            {
                continue;
            }
            let report = block
                .stmts
                .iter()
                .find(|statement| is_super_init_state_statement(statement))
                .map(Stmt::span)
                .unwrap_or(&method.name.span);
            diags.push(ctx.diagnostic(
                "proper-super-init-state",
                "super.initState() should be called first in initState().",
                *report,
            ));
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn is_super_init_state_statement(statement: &Stmt) -> bool {
    let Stmt::Expr(ExprStmt { expr, .. }) = statement else {
        return false;
    };
    matches!(expr.unparenthesized(),
        Expr::Call { callee, args, .. }
            if args.positional.is_empty()
                && args.named.is_empty()
                && matches!(callee.as_ref(),
                    Expr::Field { object, field, .. }
                        if matches!(object.as_ref(), Expr::Super { .. })
                            && field.name == "initState"))
}
