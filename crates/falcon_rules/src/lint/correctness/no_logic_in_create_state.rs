//! Require `StatefulWidget.createState` to return only a fresh zero-argument
//! Flutter `State` construction.
//!
//! Widget and State identities are resolved canonically, including prefixes,
//! aliases, and intermediate project subclasses. Unresolved ancestry or
//! constructor identity is not guessed from a name suffix.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, SemanticModel, SignatureIndex,
    TypeTruth,
};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct NoLogicInCreateState;

impl Rule for NoLogicInCreateState {
    fn name(&self) -> &'static str {
        "no-logic-in-create-state"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let stateful_widget = flutter_identity("StatefulWidget");
        let state = flutter_identity("State");
        let mut diags = Vec::new();
        for declaration in &program.declarations {
            let TopLevelDecl::Class(class) = declaration else {
                continue;
            };
            let Some(identity) = model.resolve_name(std::slice::from_ref(&class.name.name)) else {
                continue;
            };
            if signatures.is_subtype_of(&interface(identity), &stateful_widget, &model)
                != TypeTruth::Yes
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
                    != TypeTruth::Yes
                {
                    continue;
                }
                let Some(body) = &method.body else {
                    continue;
                };
                if body_result(body, &model, signatures, &state) != BodyResult::Violation {
                    continue;
                }
                diags.push(Diagnostic::new(
                    "no-logic-in-create-state",
                    Severity::Warning,
                    "Avoid logic in createState; it should only return a new State instance.",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: method.name.span.start,
                        end: method.name.span.end,
                    },
                ));
            }
        }
        diags
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
    let Some((constructed, args)) = construction(expression, model) else {
        return if looks_like_unresolved_construction(expression) {
            BodyResult::Unknown
        } else {
            BodyResult::Violation
        };
    };
    match signatures.is_subtype_of(&constructed, state, model) {
        TypeTruth::Yes if args.positional.is_empty() && args.named.is_empty() => {
            BodyResult::Trivial
        }
        TypeTruth::Yes => BodyResult::Violation,
        TypeTruth::No => BodyResult::Violation,
        TypeTruth::Unknown => BodyResult::Unknown,
    }
}

fn construction<'a>(
    expression: &'a Expr,
    model: &SemanticModel<'_>,
) -> Option<(ResolvedType, &'a ArgList)> {
    match expression {
        Expr::New {
            dart_type, args, ..
        } => Some((model.resolve_type(dart_type).with_nullable(false), args)),
        Expr::Call { callee, args, .. } => {
            let mut segments = expression_segments(callee)?;
            let identity = model.resolve_name(&segments).or_else(|| {
                segments.pop();
                model.resolve_name(&segments)
            })?;
            Some((interface(identity), args))
        }
        _ => None,
    }
}

fn looks_like_unresolved_construction(expression: &Expr) -> bool {
    matches!(expression, Expr::New { .. })
        || matches!(expression, Expr::Call { callee, .. }
            if expression_segments(callee).is_some())
}

fn expression_segments(expression: &Expr) -> Option<Vec<String>> {
    let mut current = expression;
    let mut segments = Vec::new();
    loop {
        match current {
            Expr::Ident(identifier) => {
                segments.push(identifier.name.clone());
                segments.reverse();
                return Some(segments);
            }
            Expr::Field { object, field, .. } => {
                segments.push(field.name.clone());
                current = object;
            }
            _ => return None,
        }
    }
}

fn interface(identity: DeclarationIdentity) -> ResolvedType {
    ResolvedType::Interface {
        identity,
        arguments: Vec::new(),
        nullable: false,
        extension_type: false,
    }
}

fn flutter_identity(name: &str) -> DeclarationIdentity {
    DeclarationIdentity::Package {
        package: "flutter".to_string(),
        name: name.to_string(),
    }
}
