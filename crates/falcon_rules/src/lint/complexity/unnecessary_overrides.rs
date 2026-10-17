//! Flags a member that does nothing but forward to `super` with
//! the same arguments and resolved contract.
//!
//! Such an override adds visual noise and a maintenance burden without changing
//! behavior — delete it and the inherited implementation stays in force.
//! Resolved inherited metadata and signatures preserve meaningful overrides,
//! including documentation, body comments, narrower contracts, and visibility changes.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberKind, ParameterContract, ResolvedType, Rule,
    RulePreparation, SemanticModel, SignatureIndex, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UnnecessaryOverrides;

impl Rule for UnnecessaryOverrides {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let diags = Vec::new();
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(diags);
        };
        let model = SemanticModel::new(ctx.file_path, ctx.parsed, identities, ctx.types);
        let docs = ctx.tree().doc_blocks(ctx.source).collect::<Vec<_>>();
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model,
            signatures,
            docs,
            diags,
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    docs: Vec<DocBlock<'a>>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        let model = &self.model;
        let Some(members) = crate::lint::declarations::members_of(decl) else {
            return StructuralTraversal::SkipChildren;
        };
        let name = match decl {
            TopLevelDecl::Class(node) => node.name().name(),
            TopLevelDecl::MixinClass(node) => node.name().name(),
            TopLevelDecl::Mixin(node) => node.name().name(),
            TopLevelDecl::Enum(node) => node.name().name(),
            _ => return StructuralTraversal::SkipChildren,
        };
        let Some(identity) = model.resolve_name(name) else {
            return StructuralTraversal::SkipChildren;
        };
        for member in members {
            check_member(
                member, &identity, model, self.signatures, &self.docs, &mut self.diags, self.ctx,
            );
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn check_member(
    member: ClassMember<'_>,
    identity: &DeclarationIdentity,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    docs: &[DocBlock<'_>],
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    let (annotations, name, name_span, span, write, body) = match member {
        ClassMember::Method(m) if !m.is_static() => (
            m.annotations(),
            m.name().name(),
            m.name().span(),
            m.span(),
            false,
            m.body(),
        ),
        ClassMember::Getter(g) if !g.is_static() => (
            g.annotations(),
            g.name().name(),
            g.name().span(),
            g.span(),
            false,
            g.body(),
        ),
        ClassMember::Setter(s) if !s.is_static() => (
            s.annotations(),
            s.name().name(),
            s.name().span(),
            s.span(),
            true,
            s.body(),
        ),
        ClassMember::Operator(o) => (
            o.annotations(),
            o.lookup_name(),
            crate::lint::declarations::operator_token_span(o),
            o.span(),
            false,
            o.body(),
        ),
        _ => return,
    };
    if !single_expr(body, ctx).is_some_and(|expr| forwards(member, expr.unparenthesized(), ctx)) {
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
                    .iter()
                    .any(|annotation| annotation.is_package("meta", "protected"))
        });
    let own_protected =
        crate::lint::declarations::has_meta_annotation(annotations.clone(), "protected", model);
    if (inherited_protected && !own_protected)
        || !annotations.clone().all(|annotation| {
            let Some(name) = falcon_analyze::QualifiedName::from_identifiers(annotation.name())
            else {
                return false;
            };
            let identity = model
                .resolve_value(name)
                .or_else(|| model.resolve_name(name))
                .or_else(|| model.resolve_imported_member(name));
            (annotation.args().is_none()
                && (identity
                    .as_ref()
                    .is_some_and(|identity| identity.is_sdk("dart:core", "override"))
                    || (identity.is_none()
                        && model
                            .resolve_sdk_member(name)
                            .is_some_and(|identity| identity.is_sdk("dart:core", "override")))))
                || (inherited_protected
                    && identity.is_some_and(|identity| identity.is_package("meta", "protected")))
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

fn args_match_params(args: ArgList<'_>, params: FormalParamList<'_>) -> bool {
    args.positional().len() == params.positional().count()
        && args.named().len() == params.named().count()
        && args.positional().zip(params.positional()).all(|(arg, parameter)| {
            matches!(arg.unparenthesized(), Expr::Ident(id) if id.name() == parameter.name().name())
        })
        && params.named().all(|parameter| {
            args.named()
                .find(|arg| arg.name().name() == parameter.name().name())
                .is_some_and(|arg| {
                    matches!(arg.value().unparenthesized(), Expr::Ident(id)
                        if id.name() == parameter.name().name())
                })
        })
}

fn has_preceding_comment(offset: usize, ctx: &AnalyzeContext) -> bool {
    crate::lint::tokens::preceding_trivia(ctx.tokens(), offset)
        .any(|token| token.kind().is_comment())
}

fn is_super(expr: Expr<'_>, ctx: &AnalyzeContext) -> bool {
    let expr = expr.unparenthesized();
    matches!(expr, Expr::Super(_)) && !has_preceding_comment(expr.span().start, ctx)
}

/// The single expression a body forwards, if the body is exactly one
/// forwarding statement (arrow, `return e;`, or a bare `e;`).
fn single_expr<'a>(body: Option<FunctionBody<'a>>, ctx: &AnalyzeContext) -> Option<Expr<'a>> {
    match body? {
        FunctionBody::Arrow(body) => Some(body.expr()),
        FunctionBody::Block(block) => {
            let mut stmts = block.stmts();
            let stmt = stmts.next()?;
            if stmts.next().is_some() {
                return None;
            }
            match stmt {
                Stmt::Return(ret) if !has_preceding_comment(ret.span().start, ctx) => ret.value(),
                Stmt::Expr(expr) => Some(expr.expr()),
                _ => None,
            }
        }
        FunctionBody::Native(_) => None,
    }
}

fn forwards(member: ClassMember<'_>, expr: Expr<'_>, ctx: &AnalyzeContext) -> bool {
    match (member, expr) {
        (ClassMember::Method(method), Expr::Call(call)) => {
            matches!(call.callee().unparenthesized(), Expr::Field(field)
                if is_super(field.object(), ctx) && field.field().name() == method.name().name())
                && args_match_params(call.args(), method.params())
        }
        (ClassMember::Getter(getter), Expr::Field(field)) => {
            is_super(field.object(), ctx) && field.field().name() == getter.name().name()
        }
        (ClassMember::Setter(setter), Expr::Assign(assign)) if assign.op() == AssignOp::Eq => {
            matches!(assign.target().unparenthesized(), Expr::Field(field)
                if is_super(field.object(), ctx) && field.field().name() == setter.name().name())
                && matches!(assign.value().unparenthesized(), Expr::Ident(id) if id.name() == setter.param().name())
        }
        (ClassMember::Operator(operator), Expr::Binary(binary)) => {
            let mut params = operator.params().params();
            let parameter = params.next();
            binary.op() != BinaryOp::NotEq
                && binary.op().method_name() == Some(operator.op())
                && parameter.is_some_and(|parameter| {
                    params.next().is_none()
                        && matches!(binary.right().unparenthesized(), Expr::Ident(id)
                            if id.name() == parameter.name().name())
                })
                && is_super(binary.left(), ctx)
        }
        (ClassMember::Operator(operator), Expr::Unary(unary)) => {
            operator.params().params().is_empty()
                && matches!(
                    (unary.op(), operator.op()),
                    (UnaryOp::Minus, "-") | (UnaryOp::Tilde, "~")
                )
                && is_super(unary.operand(), ctx)
        }
        _ => false,
    }
}
