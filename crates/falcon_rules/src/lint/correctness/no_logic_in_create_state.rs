//! Require `StatefulWidget.createState` to return only a fresh zero-argument
//! Flutter `State` construction.
//!
//! Widget and State identities are resolved canonically, including prefixes,
//! aliases, and intermediate project subclasses. Unresolved ancestry or
//! constructor identity is not guessed from a name suffix.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    SignatureIndex, Truth,
};
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct NoLogicInCreateState;

impl Rule for NoLogicInCreateState {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let stateful_widget = DeclarationIdentity::flutter("StatefulWidget");
        let state = DeclarationIdentity::flutter("State");
        let mut diags = Vec::new();
        for declaration in &program.declarations {
            let TopLevelDecl::Class(class) = declaration else {
                continue;
            };
            let Some(identity) = model.resolve_name(&class.name.name) else {
                continue;
            };
            if signatures.is_subtype_of(
                &ResolvedType::interface_identity(identity),
                &stateful_widget,
                &model,
            ) != Truth::Yes
            {
                continue;
            }
            for member in &class.members {
                let ClassMember::Method(method) = member else {
                    continue;
                };
                if method.name.name != "createState" {
                    continue;
                }
                let Some(return_type) = &method.return_type else {
                    continue;
                };
                if signatures.is_subtype_of(&model.resolve_type(return_type), &state, &model)
                    != Truth::Yes
                {
                    continue;
                }
                let Some(body) = &method.body else {
                    continue;
                };
                if body_result(body, &model, signatures, &state) != BodyResult::Violation {
                    continue;
                }
                diags.push(ctx.diagnostic(
                    "no-logic-in-create-state",
                    "Avoid logic in createState; it should only return a new State instance.",
                    violation_span(body),
                ));
            }
        }
        RulePreparation::Diagnostics(diags)
    }
}

fn violation_span(body: &FunctionBody) -> Span {
    match body {
        FunctionBody::Arrow(expression, _) => *expression.span(),
        FunctionBody::Block(block) => match block.stmts.as_slice() {
            [
                Stmt::Return(ReturnStmt {
                    value: Some(expression),
                    ..
                }),
            ] => *expression.span(),
            _ => *body.span(),
        },
        FunctionBody::Native(..) => *body.span(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BodyResult {
    Trivial,
    Violation,
    Unknown,
}

fn body_result(
    body: &FunctionBody,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    state: &DeclarationIdentity,
) -> BodyResult {
    let expression = match body {
        FunctionBody::Arrow(expression, _) => expression.as_ref(),
        FunctionBody::Block(block) => match block.stmts.as_slice() {
            [
                Stmt::Return(ReturnStmt {
                    value: Some(expression),
                    ..
                }),
            ] => expression,
            _ => return BodyResult::Violation,
        },
        FunctionBody::Native(..) => return BodyResult::Unknown,
    };
    let Some((constructed, args)) = crate::lint::flutter::construction(expression, model) else {
        return if looks_like_unresolved_construction(expression) {
            BodyResult::Unknown
        } else {
            BodyResult::Violation
        };
    };
    match signatures.is_subtype_of(&constructed, state, model) {
        Truth::Yes if args.positional.is_empty() && args.named.is_empty() => BodyResult::Trivial,
        Truth::Yes => BodyResult::Violation,
        Truth::No => BodyResult::Violation,
        Truth::Unknown => BodyResult::Unknown,
    }
}

fn looks_like_unresolved_construction(expression: &Expr) -> bool {
    matches!(expression.unparenthesized(), Expr::Call { callee, .. }
        if falcon_analyze::expression_segments(callee, false).is_some())
}
