//! Ban resolved API references, not identifiers that merely share their spelling.
//! Match top-level declaration names or `DeclaringType.member`; prefixes are ignored.
//! Calls, tear-offs, property reads/writes, constructors and cascades are included.
//! Unnamed constructors consistently use `Class.new`, including aliases and prefixes.
//! Dynamic and unresolved targets are skipped.

use falcon_analyze::semantic_scope::{CascadeAssignment, SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, QualifiedName, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::avoid_banned_names::{Entry, entries, report};

#[derive(Default)]
pub struct BannedUsage;

impl Rule for BannedUsage {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let entries = entries(ctx, "banned-usage");
        if entries.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            entries,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    entries: Vec<Entry<'a>>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    Read,
    Write,
    ReadWrite,
}

impl Access {
    fn assignment(operator: AssignOp) -> Self {
        if operator == AssignOp::Eq {
            Self::Write
        } else {
            Self::ReadWrite
        }
    }

    fn expression(node: Expr<'_>) -> Self {
        let parent = node
            .syntax()
            .ancestors()
            .skip(1)
            .filter_map(Expr::cast)
            .find(|parent| !matches!(parent, Expr::Paren(_)));
        match parent {
            Some(Expr::Assign(assignment))
                if assignment.target().unparenthesized().span() == node.span() =>
            {
                Self::assignment(assignment.op())
            }
            Some(Expr::PostfixIncDec(_)) => Self::ReadWrite,
            Some(Expr::Unary(unary))
                if matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus) =>
            {
                Self::ReadWrite
            }
            _ => Self::Read,
        }
    }
}

impl Collector<'_> {
    fn emit(&mut self, name: &str, span: Span) {
        report(
            self.ctx,
            &self.entries,
            "banned-usage",
            name,
            span,
            &mut self.diagnostics,
        );
    }

    fn is_unnamed_extension_member(identifier: Identifier<'_>) -> bool {
        let Some(extension) = identifier
            .syntax()
            .ancestors()
            .skip(1)
            .find_map(ExtensionDecl::cast)
        else {
            return false;
        };
        extension.name().is_none()
            && extension.members().any(|member| {
                let syntax = member.syntax();
                syntax
                    .children()
                    .filter_map(Identifier::cast)
                    .any(|name| name.name() == identifier.name())
                    || syntax
                        .children()
                        .filter_map(VarDeclarator::cast)
                        .any(|variable| variable.name().name() == identifier.name())
            })
    }

    fn member(
        &mut self,
        receiver: &ResolvedType,
        name: &str,
        span: Span,
        access: Access,
        state: &SemanticState<'_>,
    ) {
        let writable = (access != Access::Read)
            .then(|| {
                state
                    .signatures
                    .resolved_writable_member_owner(receiver, name, &state.model)
                    .or_else(|| {
                        state.signatures.resolved_extension_access_owner(
                            receiver,
                            name,
                            true,
                            &state.model,
                        )
                    })
            })
            .flatten();
        let diagnostic_count = self.diagnostics.len();
        if let Some(owner) = writable.as_ref() {
            self.emit(&format!("{}.{name}", owner.name()), span);
            if self.diagnostics.len() > diagnostic_count {
                return;
            }
        }
        let readable = (access != Access::Write)
            .then(|| {
                state
                    .signatures
                    .resolved_readable_member_owner(receiver, name, &state.model)
                    .or_else(|| {
                        state.signatures.resolved_extension_access_owner(
                            receiver,
                            name,
                            false,
                            &state.model,
                        )
                    })
            })
            .flatten();
        if let Some(owner) = readable.filter(|owner| Some(owner) != writable.as_ref()) {
            self.emit(&format!("{}.{name}", owner.name()), span);
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_constructor(&mut self, node: ConstructorDecl<'a>, state: &SemanticState<'a>) {
        if let Some(redirect) = node.redirect()
            && let Some((owner, name)) = falcon_analyze::resolve_new_constructor(
                redirect.type_(),
                redirect.constructor_name(),
                &state.model,
            )
        {
            let owner = state.model.unaliased(owner);
            self.emit(&format!("{}.{name}", owner.name()), redirect.span());
        }
        for initializer in node.initializers() {
            let (receiver, name, span) = match initializer {
                ConstructorInitializer::SuperCall(call) => {
                    (state.superclass.as_ref(), call.call_name(), call.span())
                }
                ConstructorInitializer::ThisCall(call) => {
                    (state.enclosing_type.as_ref(), call.call_name(), call.span())
                }
                _ => continue,
            };
            if let Some(ResolvedType::Interface { identity, .. }) = receiver {
                let owner = state.model.unaliased(identity.clone());
                self.emit(
                    &format!(
                        "{}.{}",
                        owner.name(),
                        name.map_or("new", |name| name.name())
                    ),
                    span,
                );
            }
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        // The call owns a constructor diagnostic; its selector children do not.
        let mut parent = node.syntax().parent();
        while let Some(ancestor) = parent {
            if let Some(Expr::Call(call)) = Expr::cast(ancestor) {
                let callee = call.callee().span();
                if callee.start <= node.span().start
                    && node.span().end <= callee.end
                    && state.is_constructor_invocation(call.callee())
                {
                    return;
                }
                break;
            }
            if let Some(Expr::Field(field)) = Expr::cast(ancestor)
                && state.is_constructor_invocation(Expr::Field(field))
                && falcon_analyze::resolve_call_constructor(Expr::Field(field), &state.model, true)
                    .is_some_and(|(_, name)| name == field.field().name())
            {
                return;
            }
            if !matches!(
                Expr::cast(ancestor),
                Some(Expr::Field(_) | Expr::Paren(_) | Expr::GenericInstantiation(_))
            ) {
                break;
            }
            parent = ancestor.parent();
        }
        match node {
            Expr::Call(call) if state.is_constructor_invocation(call.callee()) => {
                if let Some((owner, name)) =
                    falcon_analyze::resolve_call_constructor(call.callee(), &state.model, true)
                {
                    let owner = state.model.unaliased(owner);
                    self.emit(&format!("{}.{name}", owner.name()), call.callee().span());
                }
            }
            Expr::Ident(identifier)
                if (!state.environment.is_bound(identifier.name())
                    || (state.environment.is_bound_in_root_scope(identifier.name())
                        && state.constant_owner.as_ref().is_some_and(|owner| {
                            !state
                                .signatures
                                .declared_member_facts(owner, identifier.name())
                                .is_empty()
                        })))
                    && state.type_parameters.lookup(identifier.name()).is_none() =>
            {
                if Self::is_unnamed_extension_member(identifier) {
                    return;
                }
                if let Some(owner) = &state.constant_owner
                    && (!state
                        .signatures
                        .declared_member_facts(owner, identifier.name())
                        .is_empty()
                        || state.signatures.is_enum_value(owner, identifier.name()))
                {
                    if let Some(receiver) = &state.enclosing_type
                        && matches!(receiver, ResolvedType::Interface { identity, .. } if identity == owner)
                        && !state.signatures.is_enum_value(owner, identifier.name())
                        && !state
                            .signatures
                            .declared_member_facts(owner, identifier.name())
                            .iter()
                            .any(|fact| fact.is_static)
                    {
                        self.member(
                            receiver,
                            identifier.name(),
                            identifier.span(),
                            Access::expression(node),
                            state,
                        );
                    } else {
                        self.emit(
                            &format!("{}.{}", owner.name(), identifier.name()),
                            identifier.span(),
                        );
                    }
                } else if let Some(identity) = state
                    .model
                    .resolve_value(identifier.name())
                    .or_else(|| state.model.resolve_name(identifier.name()))
                {
                    self.emit(identity.name(), identifier.span());
                } else if let Some(receiver) = &state.enclosing_type {
                    self.member(
                        receiver,
                        identifier.name(),
                        identifier.span(),
                        Access::expression(node),
                        state,
                    );
                }
            }
            Expr::Field(field) => {
                if let Some((owner, name)) =
                    falcon_analyze::resolve_call_constructor(node, &state.model, true)
                    && name == field.field().name()
                    && state.is_constructor_invocation(node)
                {
                    let owner = state.model.unaliased(owner);
                    self.emit(&format!("{}.{name}", owner.name()), field.field().span());
                    return;
                }
                if let Expr::Ident(prefix) = field.object().unparenthesized()
                    && !state.environment.is_bound(prefix.name())
                    && state.type_parameters.lookup(prefix.name()).is_none()
                {
                    let name = QualifiedName::prefixed(prefix.name(), field.field().name());
                    if let Some(identity) = state
                        .model
                        .resolve_value(name)
                        .or_else(|| state.model.resolve_name(name))
                    {
                        self.emit(identity.name(), field.field().span());
                        return;
                    }
                }
                let owner = match field.object().unparenthesized() {
                    Expr::Ident(name)
                        if !state.environment.is_bound(name.name())
                            && state.type_parameters.lookup(name.name()).is_none() =>
                    {
                        state.model.resolve_name(name.name())
                    }
                    Expr::Field(ty) => {
                        if let Expr::Ident(prefix) = ty.object().unparenthesized()
                            && !state.environment.is_bound(prefix.name())
                            && state.type_parameters.lookup(prefix.name()).is_none()
                        {
                            state.model.resolve_name(QualifiedName::prefixed(
                                prefix.name(),
                                ty.field().name(),
                            ))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(owner) = owner {
                    let owner = state.model.unaliased(owner);
                    if state
                        .signatures
                        .declared_member_facts(&owner, field.field().name())
                        .iter()
                        .any(|facts| facts.is_static)
                        || state.signatures.is_enum_value(&owner, field.field().name())
                    {
                        self.emit(
                            &format!("{}.{}", owner.name(), field.field().name()),
                            field.field().span(),
                        );
                        return;
                    }
                }
                let receiver = state.infer_access_receiver(
                    field.object(),
                    field.field().name(),
                    field.is_null_safe(),
                );
                self.member(
                    &receiver,
                    field.field().name(),
                    field.field().span(),
                    Access::expression(node),
                    state,
                );
            }
            Expr::DotShorthand(shorthand) => {
                if let Some(ResolvedType::Interface { identity, .. }) = state.expected() {
                    let owner = state.model.unaliased(identity.clone());
                    self.emit(
                        &format!("{}.{}", owner.name(), shorthand.name().name()),
                        shorthand.name().span(),
                    );
                }
            }
            Expr::New(construction) => {
                if let Some((owner, name)) = falcon_analyze::resolve_new_constructor(
                    construction.dart_type(),
                    construction.constructor_name(),
                    &state.model,
                ) {
                    let owner = state.model.unaliased(owner);
                    self.emit(&format!("{}.{name}", owner.name()), construction.span());
                }
            }
            _ => {}
        }
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'a>,
    ) {
        match operation {
            CascadeOp::Call(call) => self.member(
                receiver,
                call.name().name(),
                call.name().span(),
                Access::Read,
                state,
            ),
            CascadeOp::Field(field) => self.member(
                receiver,
                field.name().name(),
                field.name().span(),
                Access::Read,
                state,
            ),
            _ => {}
        }
    }

    fn visit_cascade_assignment(
        &mut self,
        assignment: CascadeAssignment<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'a>,
    ) {
        if !assignment.indexed
            && let Expr::Ident(name) = assignment.target.unparenthesized()
        {
            self.member(
                receiver,
                name.name(),
                name.span(),
                Access::assignment(assignment.operator),
                state,
            );
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
