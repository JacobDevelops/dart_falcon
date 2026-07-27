//! Require `super.initState()` to be the first statement of a resolved Flutter
//! `State` subclass's `initState` override.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, SemanticModel, TypeTruth,
};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct ProperSuperInitState;

impl Rule for ProperSuperInitState {
    fn name(&self) -> &'static str {
        "proper-super-init-state"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let state = flutter_state();
        let mut diags = Vec::new();
        for declaration in &program.declarations {
            let TopLevelDecl::Class(class) = declaration else {
                continue;
            };
            let Some(identity) = model.resolve_name(std::slice::from_ref(&class.name.name)) else {
                continue;
            };
            if signatures.is_subtype_of(&interface(identity), &state, &model) != TypeTruth::Yes {
                continue;
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
                diags.push(Diagnostic::new(
                    "proper-super-init-state",
                    Severity::Warning,
                    "super.initState() should be called first in initState().",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: report.start,
                        end: report.end,
                    },
                ));
            }
        }
        diags
    }
}

fn is_super_init_state_statement(statement: &Stmt) -> bool {
    let Stmt::Expr(ExprStmt { expr, .. }) = statement else {
        return false;
    };
    matches!(expr,
        Expr::Call { callee, args, .. }
            if args.positional.is_empty()
                && args.named.is_empty()
                && matches!(callee.as_ref(),
                    Expr::Field { object, field, .. }
                        if matches!(object.as_ref(), Expr::Super { .. })
                            && field.name == "initState"))
}

fn interface(identity: DeclarationIdentity) -> ResolvedType {
    ResolvedType::Interface {
        identity,
        arguments: Vec::new(),
        nullable: false,
        extension_type: false,
    }
}

fn flutter_state() -> DeclarationIdentity {
    DeclarationIdentity::Package {
        package: "flutter".to_string(),
        name: "State".to_string(),
    }
}
