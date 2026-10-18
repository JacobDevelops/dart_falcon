//! Require GlobalKey fields to be passed to a widget.
//!
//! Checks owned GlobalKey-subtype instance fields in concrete Flutter State
//! classes. Abstract State bases and injected fields are skipped. Passing a
//! field anywhere in a call argument counts conservatively, since helpers may
//! attach it to a widget. Shadowed locals and currentState/currentContext receiver
//! reads do not count as passing the key.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AlwaysPassGlobalKey;

struct KeyField<'a> {
    owner: DeclarationIdentity,
    name: &'a str,
    span: Span,
    passed: bool,
}

impl Rule for AlwaysPassGlobalKey {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(index), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path, ctx.parsed, index, ctx.types);
        let mut fields = Vec::new();
        // Index declaration headers only; the shared semantic walk observes uses.
        for declaration in program.declarations() {
            let (name, members) = match declaration {
                TopLevelDecl::Class(class) if !class.modifiers().is_abstract => {
                    (class.name().name(), class.members())
                }
                TopLevelDecl::MixinClass(class) if !class.is_abstract() => {
                    (class.name().name(), class.members())
                }
                _ => continue,
            };
            let Some(owner) = model.resolve_name(name) else {
                continue;
            };
            if signatures.is_subtype_of(
                &ResolvedType::interface_identity(owner.clone()),
                &DeclarationIdentity::flutter("State"),
                &model,
            ) != Truth::Yes
            {
                continue;
            }
            for member in members {
                let ClassMember::Field(field) = member else {
                    continue;
                };
                if field.is_static() {
                    continue;
                }
                for declarator in field.declarators() {
                    let name = declarator.name();
                    if declarator.initializer().is_none() {
                        continue;
                    }
                    let ty = field
                        .field_type()
                        .map(|ty| model.resolve_type(ty))
                        .or_else(|| {
                            declarator.initializer().and_then(|value| {
                                crate::lint::flutter::constructor_invocation(value, &model).map(
                                    |(identity, _, _)| {
                                        ResolvedType::interface_identity(model.unaliased(identity))
                                    },
                                )
                            })
                        });
                    if ty.as_ref().is_some_and(|ty| {
                        signatures.is_subtype_of(
                            ty,
                            &DeclarationIdentity::flutter("GlobalKey"),
                            &model,
                        ) == Truth::Yes
                    }) {
                        fields.push(KeyField {
                            owner: owner.clone(),
                            name: name.name(),
                            span: name.span(),
                            passed: false,
                        });
                    }
                }
            }
        }
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            fields,
            arguments: Vec::new(),
            state_receivers: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    fields: Vec<KeyField<'a>>,
    arguments: Vec<(DeclarationIdentity, Span)>,
    state_receivers: Vec<Span>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.fields
            .iter()
            .filter(|field| !field.passed)
            .map(|field| {
                self.ctx.diagnostic(
                    "always-pass-global-key",
                    "Pass this GlobalKey field to a widget.",
                    field.span,
                )
            })
            .collect()
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if self.fields.is_empty() {
            return;
        }
        if let Expr::Field(field) = node
            && matches!(field.field().name(), "currentState" | "currentContext")
            && matches!(
                field.object().unparenthesized(),
                Expr::Ident(_) | Expr::Field(_)
            )
        {
            self.state_receivers.push(field.object().span());
        }
        if self
            .state_receivers
            .iter()
            .any(|span| span.start <= node.span().start && span.end >= node.span().end)
        {
            return;
        }
        let Some(ResolvedType::Interface {
            identity: owner, ..
        }) = &state.enclosing_type
        else {
            return;
        };
        let args = match node {
            Expr::Call(call) => Some(call.args()),
            Expr::New(construction) => Some(construction.args()),
            _ => None,
        };
        if let Some(args) = args {
            self.arguments.extend(
                args.positional()
                    .chain(args.named().map(|arg| arg.value()))
                    .map(|argument| (owner.clone(), argument.span())),
            );
        }
        let (name, receiver) = match node.unparenthesized() {
            Expr::Ident(name)
                if !state.environment.is_bound(name.name()) || state.is_own_field(name.name()) =>
            {
                (name.name(), state.enclosing_type.clone().unwrap())
            }
            Expr::Field(field) => (field.field().name(), state.infer_receiver(field.object())),
            _ => return,
        };
        let Some(field_owner) =
            state
                .signatures
                .resolved_member_owner(&receiver, name, &state.model)
        else {
            return;
        };
        if !self.arguments.iter().any(|(argument_owner, span)| {
            argument_owner == owner
                && span.start <= node.span().start
                && span.end >= node.span().end
        }) {
            return;
        }
        for field in &mut self.fields {
            if field.owner == field_owner && field.name == name {
                field.passed = true;
            }
        }
    }
}
