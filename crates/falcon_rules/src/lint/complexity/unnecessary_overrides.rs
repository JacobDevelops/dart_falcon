//! Flags a member that does nothing but forward to `super` with
//! the same arguments and resolved contract.
//!
//! Such an override adds visual noise and a maintenance burden without changing
//! behavior — delete it and the inherited implementation stays in force.
//! Resolved inherited metadata and signatures preserve meaningful overrides,
//! including documentation, body comments, narrower contracts, and visibility changes.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberKind, ParameterContract, ResolvedType, Rule,
    SemanticModel, SignatureIndex,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct UnnecessaryOverrides;

impl Rule for UnnecessaryOverrides {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return diags;
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let docs = ctx.syntax.doc_blocks(ctx.source).collect::<Vec<_>>();
        for decl in &program.declarations {
            let Some(members) = crate::lint::declarations::members_of(decl) else {
                continue;
            };
            let name = match decl {
                TopLevelDecl::Class(node) => &node.name.name,
                TopLevelDecl::MixinClass(node) => &node.name.name,
                TopLevelDecl::Mixin(node) => &node.name.name,
                TopLevelDecl::Enum(node) => &node.name.name,
                _ => continue,
            };
            let Some(identity) = model.resolve_name(name) else {
                continue;
            };
            for member in members {
                check_member(
                    member, &identity, &model, signatures, &docs, &mut diags, ctx,
                );
            }
        }
        diags
    }
}

fn check_member(
    member: &ClassMember,
    identity: &DeclarationIdentity,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    docs: &[falcon_syntax::token::DocBlock<'_>],
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    let (annotations, name, name_span, span, write, unnecessary) = match member {
        ClassMember::Method(m) if !m.is_static => (
            &m.annotations,
            m.name.name.as_str(),
            m.name.span,
            m.span,
            false,
            method_forwards(m, ctx),
        ),
        ClassMember::Getter(g) if !g.is_static => (
            &g.annotations,
            g.name.name.as_str(),
            g.name.span,
            g.span,
            false,
            getter_forwards(g, ctx),
        ),
        ClassMember::Setter(s) if !s.is_static => (
            &s.annotations,
            s.name.name.as_str(),
            s.name.span,
            s.span,
            true,
            setter_forwards(s, ctx),
        ),
        ClassMember::Operator(o) => (
            &o.annotations,
            o.lookup_name(),
            crate::lint::declarations::operator_token_span(o, ctx.syntax),
            o.span,
            false,
            operator_forwards(o, ctx),
        ),
        _ => return,
    };
    if !unnecessary {
        return;
    }
    let first_doc = docs.partition_point(|doc| doc.next_token < span.start);
    if name == "noSuchMethod"
        || docs
            .get(first_doc)
            .is_some_and(|doc| doc.next_token <= name_span.start)
    {
        return;
    }
    let receiver = ResolvedType::interface_identity(identity.clone());
    let Some(owner) = signatures.resolved_super_member_owner(&receiver, name, write, model) else {
        return;
    };
    let inherited_protected = signatures
        .declared_member_facts(&owner, name)
        .iter()
        .any(|fact| {
            let matches_accessor = match member {
                ClassMember::Method(_) => fact.kind == MemberKind::Method,
                ClassMember::Operator(_) => fact.kind == MemberKind::Operator,
                ClassMember::Getter(_) => fact.kind.has_getter(),
                ClassMember::Setter(_) => fact.kind.has_setter(),
                _ => false,
            };
            matches_accessor
                && fact
                    .annotations
                    .contains(&DeclarationIdentity::package("meta", "protected"))
        });
    let own_protected =
        crate::lint::declarations::has_meta_annotation(annotations, "protected", model);
    if (inherited_protected && !own_protected)
        || !annotations.iter().all(|annotation| {
            (annotation.args.is_none()
                && falcon_analyze::QualifiedName::from_identifiers(&annotation.name)
                    .and_then(|name| {
                        model
                            .resolve_value(name)
                            .or_else(|| model.resolve_name(name))
                            .or_else(|| model.resolve_imported_member(name))
                            .or_else(|| model.resolve_sdk_member(name))
                    })
                    .is_some_and(|identity| identity.is_sdk("dart:core", "override")))
                || (inherited_protected
                    && crate::lint::declarations::has_meta_annotation(
                        std::slice::from_ref(annotation),
                        "protected",
                        model,
                    ))
        })
    {
        return;
    }
    let Some(supertype) = (if write {
        signatures.super_receiver_for_write(&receiver, name, model)
    } else {
        signatures.super_receiver(&receiver, name, model)
    }) else {
        return;
    };
    if !matches!(member, ClassMember::Getter(_)) {
        let own_contracts = signatures.effective_parameter_contracts(&receiver, name, write, model);
        let inherited_contracts =
            signatures.super_parameter_contracts(&receiver, name, write, model);
        if !same_parameters(own_contracts.as_deref(), inherited_contracts.as_deref()) {
            return;
        }
    }
    if matches!(member, ClassMember::Method(_) | ClassMember::Operator(_)) {
        let (Some((declared, declared_substitutions)), Some((inherited, inherited_substitutions))) = (
            signatures.resolved_member(&receiver, name, model),
            signatures.resolved_member(&supertype, name, model),
        ) else {
            return;
        };
        if model.substitute(&declared.return_type, &declared_substitutions)
            != model.substitute(&inherited.return_type, &inherited_substitutions)
            || declared.positional.len() != inherited.positional.len()
            || declared.named.len() != inherited.named.len()
            || declared
                .positional
                .iter()
                .zip(&inherited.positional)
                .any(|(own, parent)| {
                    model.substitute(own, &declared_substitutions)
                        != model.substitute(parent, &inherited_substitutions)
                })
            || declared.named.iter().any(|(name, own)| {
                inherited.named.get(name).is_none_or(|parent| {
                    model.substitute(own, &declared_substitutions)
                        != model.substitute(parent, &inherited_substitutions)
                })
            })
        {
            return;
        }
    } else {
        let (Some((declared, own)), Some((inherited, parent))) = (if write {
            (
                signatures.resolved_writable_field(&receiver, name, model),
                signatures.resolved_writable_field(&supertype, name, model),
            )
        } else {
            (
                signatures.resolved_field(&receiver, name, model),
                signatures.resolved_field(&supertype, name, model),
            )
        }) else {
            return;
        };
        if model.substitute(&declared, &own) != model.substitute(&inherited, &parent) {
            return;
        }
    }
    diags.push(ctx.diagnostic(
        "unnecessary-overrides",
        "Unnecessary override — this member only forwards to `super` unchanged",
        name_span,
    ));
}

fn same_parameters(
    own: Option<&[ParameterContract]>,
    inherited: Option<&[ParameterContract]>,
) -> bool {
    let (Some(own), Some(inherited)) = (own, inherited) else {
        return false;
    };
    own.len() == inherited.len()
        && own.iter().zip(inherited).all(|(own, inherited)| {
            let required = |kind| {
                matches!(
                    kind,
                    ParamKind::Positional | ParamKind::Named { required: true }
                )
            };
            own.name == inherited.name
                && own.is_covariant == inherited.is_covariant
                && (own.kind == inherited.kind || (required(own.kind) && required(inherited.kind)))
                && own.default_value_code == inherited.default_value_code
        })
}

fn args_match_params(args: &ArgList, params: &FormalParamList) -> bool {
    if args.positional.len() != params.positional().count() {
        return false;
    }
    for (arg, expected) in args.positional.iter().zip(params.positional()) {
        match arg.unparenthesized() {
            Expr::Ident(id) if id.name == expected.name.name => {}
            _ => return false,
        }
    }
    if args.named.len() != params.named().count() {
        return false;
    }
    for np in params.named() {
        let Some(na) = args.named.iter().find(|na| na.name.name == np.name.name) else {
            return false;
        };
        match na.value.unparenthesized() {
            Expr::Ident(id) if id.name == np.name.name => {}
            _ => return false,
        }
    }
    true
}

fn has_preceding_comment(offset: usize, ctx: &AnalyzeContext) -> bool {
    let tokens = ctx.tokens();
    let index = tokens.partition_point(|token| token.offset() < offset);
    tokens[..index]
        .iter()
        .rev()
        .take_while(|token| token.is_trivia())
        .any(|token| token.kind.is_comment())
}

fn is_super(expr: &Expr, ctx: &AnalyzeContext) -> bool {
    let expr = expr.unparenthesized();
    matches!(expr, Expr::Super { .. }) && !has_preceding_comment(expr.span().start, ctx)
}

/// The single expression a body forwards, if the body is exactly one
/// forwarding statement (arrow, `return e;`, or a bare `e;`).
fn single_expr<'a>(body: &'a Option<FunctionBody>, ctx: &AnalyzeContext) -> Option<&'a Expr> {
    match body.as_ref()? {
        FunctionBody::Arrow(e, _) => Some(e),
        FunctionBody::Block(b) => match b.stmts.as_slice() {
            [Stmt::Return(r)] if !has_preceding_comment(r.span.start, ctx) => r.value.as_ref(),
            [Stmt::Expr(e)] => Some(&e.expr),
            _ => None,
        },
        FunctionBody::Native(_, _) => None,
    }
}

fn method_forwards(m: &MethodDecl, ctx: &AnalyzeContext) -> bool {
    let Some(expr) = single_expr(&m.body, ctx) else {
        return false;
    };
    match expr.unparenthesized() {
        Expr::Call { callee, args, .. } => match callee.unparenthesized() {
            Expr::Field { object, field, .. } => {
                is_super(object, ctx)
                    && field.name == m.name.name
                    && args_match_params(args, &m.params)
            }
            _ => false,
        },
        _ => false,
    }
}

fn getter_forwards(g: &GetterDecl, ctx: &AnalyzeContext) -> bool {
    let Some(expr) = single_expr(&g.body, ctx) else {
        return false;
    };
    match expr.unparenthesized() {
        Expr::Field { object, field, .. } => is_super(object, ctx) && field.name == g.name.name,
        _ => false,
    }
}

fn setter_forwards(s: &SetterDecl, ctx: &AnalyzeContext) -> bool {
    let Some(expr) = single_expr(&s.body, ctx) else {
        return false;
    };
    match expr.unparenthesized() {
        Expr::Assign {
            target,
            op: AssignOp::Eq,
            value,
            ..
        } => {
            let target_ok = matches!(
                target.unparenthesized(),
                Expr::Field { object, field, .. } if is_super(object, ctx) && field.name == s.name.name
            );
            let value_ok =
                matches!(value.unparenthesized(), Expr::Ident(id) if id.name == s.param.name);
            target_ok && value_ok
        }
        _ => false,
    }
}

fn operator_forwards(operator: &OperatorDecl, ctx: &AnalyzeContext) -> bool {
    let Some(expr) = single_expr(&operator.body, ctx) else {
        return false;
    };
    match expr.unparenthesized() {
        Expr::Binary {
            left, op, right, ..
        } => {
            *op != BinaryOp::NotEq
                && op.method_name() == Some(operator.op.as_str())
                && matches!(operator.params.params.as_slice(), [parameter]
                    if matches!(right.unparenthesized(), Expr::Ident(id)
                        if id.name == parameter.name.name))
                && is_super(left, ctx)
        }
        Expr::Unary { op, operand, .. } => {
            operator.params.params.is_empty()
                && matches!(
                    (op, operator.op.as_str()),
                    (UnaryOp::Minus, "-") | (UnaryOp::Tilde, "~")
                )
                && is_super(operand, ctx)
        }
        _ => false,
    }
}
