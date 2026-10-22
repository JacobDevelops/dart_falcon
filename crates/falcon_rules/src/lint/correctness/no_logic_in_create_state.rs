//! Require `StatefulWidget.createState` to return only a fresh zero-argument
//! Flutter `State` construction.
//!
//! Widget and State identities are resolved canonically, including prefixes,
//! aliases, and intermediate project subclasses. Unresolved ancestry or
//! constructor identity is not guessed from a name suffix.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralRuleVisitor, StructuralTraversal, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct NoLogicInCreateState;

impl Rule for NoLogicInCreateState {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
        let stateful_widget = DeclarationIdentity::flutter("StatefulWidget");
        let state = DeclarationIdentity::flutter("State");
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model,
            signatures,
            stateful_widget,
            state,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    stateful_widget: DeclarationIdentity,
    state: DeclarationIdentity,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let ctx = self.ctx;
        let model = &self.model;
        let signatures = self.signatures;
        let stateful_widget = &self.stateful_widget;
        let state = &self.state;
        let diags = &mut self.diags;
        let TopLevelDecl::Class(class) = declaration else {
            return StructuralTraversal::SkipChildren;
        };
        let Some(identity) = model.resolve_name(class.name().name()) else {
            return StructuralTraversal::SkipChildren;
        };
        if signatures.is_subtype_of(
            &ResolvedType::interface_identity(identity),
            stateful_widget,
            model,
        ) != Truth::Yes
        {
            return StructuralTraversal::SkipChildren;
        }
        for member in class.members() {
            let ClassMember::Method(method) = member else {
                continue;
            };
            if method.name().name() != "createState" {
                continue;
            }
            let Some(return_type) = method.return_type() else {
                continue;
            };
            if signatures.is_subtype_of(&model.resolve_type(return_type), state, model)
                != Truth::Yes
            {
                continue;
            }
            let Some(body) = method.body() else {
                continue;
            };
            if body_result(body, model, signatures, state) != BodyResult::Violation {
                continue;
            }
            diags.push(ctx.diagnostic(
                "Avoid logic in `createState`; it should only return a new `State` instance.",
                violation_span(body),
            ));
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn violation_span(body: FunctionBody<'_>) -> Span {
    match body {
        FunctionBody::Arrow(arrow) => arrow.expr().span(),
        FunctionBody::Block(block) => {
            let mut statements = block.stmts();
            match (statements.next(), statements.next()) {
                (Some(Stmt::Return(statement)), None) => {
                    statement.value().map_or(body.span(), Expr::span)
                }
                _ => body.span(),
            }
        }
        FunctionBody::Native(_) => body.span(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BodyResult {
    Trivial,
    Violation,
    Unknown,
}

fn body_result(
    body: FunctionBody<'_>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    state: &DeclarationIdentity,
) -> BodyResult {
    let expression = match body {
        FunctionBody::Arrow(arrow) => arrow.expr(),
        FunctionBody::Block(block) => {
            let mut statements = block.stmts();
            match (statements.next(), statements.next()) {
                (Some(Stmt::Return(statement)), None) => match statement.value() {
                    Some(expression) => expression,
                    None => return BodyResult::Violation,
                },
                _ => return BodyResult::Violation,
            }
        }
        FunctionBody::Native(_) => return BodyResult::Unknown,
    };
    let Some((constructed, args)) = crate::lint::flutter::construction(expression, model) else {
        return if looks_like_unresolved_construction(expression) {
            BodyResult::Unknown
        } else {
            BodyResult::Violation
        };
    };
    match signatures.is_subtype_of(&constructed, state, model) {
        Truth::Yes if args.positional().is_empty() && args.named().is_empty() => {
            BodyResult::Trivial
        }
        Truth::Yes => BodyResult::Violation,
        Truth::No => BodyResult::Violation,
        Truth::Unknown => BodyResult::Unknown,
    }
}

fn looks_like_unresolved_construction(expression: Expr<'_>) -> bool {
    matches!(expression.unparenthesized(), Expr::Call(call)
        if falcon_analyze::expression_segments(call.callee(), false).is_some())
}
