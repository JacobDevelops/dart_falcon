//! Require `super.initState()` to be the first statement of a resolved Flutter
//! `State` subclass's `initState` override.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralRuleVisitor, StructuralTraversal, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct ProperSuperInitState;

impl Rule for ProperSuperInitState {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
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
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let ctx = self.ctx;
        let model = &self.model;
        let signatures = self.signatures;
        let state = &self.state;
        let diags = &mut self.diags;
        let TopLevelDecl::Class(class) = declaration else {
            return StructuralTraversal::SkipChildren;
        };
        let Some(identity) = model.resolve_name(class.name().name()) else {
            return StructuralTraversal::SkipChildren;
        };
        if signatures.is_subtype_of(&ResolvedType::interface_identity(identity), state, model)
            != Truth::Yes
        {
            return StructuralTraversal::SkipChildren;
        }
        for member in class.members() {
            let ClassMember::Method(method) = member else {
                continue;
            };
            if method.name().name() != "initState" {
                continue;
            }
            let Some(FunctionBody::Block(block)) = method.body() else {
                continue;
            };
            let super_call = block
                .stmts()
                .enumerate()
                .find(|(_, statement)| is_super_init_state_statement(*statement));
            if matches!(super_call, Some((0, _))) {
                continue;
            }
            let report = super_call.map_or(method.name().span(), |(_, statement)| statement.span());
            diags.push(ctx.diagnostic(
                "super.initState() should be called first in initState().",
                report,
            ));
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn is_super_init_state_statement(statement: Stmt<'_>) -> bool {
    let Stmt::Expr(statement) = statement else {
        return false;
    };
    matches!(statement.expr().unparenthesized(),
        Expr::Call(call)
            if call.args().positional().is_empty()
                && call.args().named().is_empty()
                && matches!(call.callee(),
                    Expr::Field(field)
                        if matches!(field.object(), Expr::Super(_))
                            && field.field().name() == "initState"))
}
