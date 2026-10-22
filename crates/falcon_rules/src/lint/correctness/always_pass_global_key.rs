//! Keep GlobalKeys stable and pass owned State fields to widgets.
//!
//! Reports nonconstant GlobalKey-subtype constructors directly in widget `key`
//! arguments, including outside State classes. Value-stable GlobalObjectKeys,
//! constant keys and cached field/top-level initializers are exempt. Closures
//! inside those initializers are still checked. Stored keys are checked only for
//! concrete Flutter State classes: abstract bases and injected fields are skipped.
//! Passing a field anywhere in a call argument counts conservatively, since
//! helpers may attach it to a widget. Shadowed locals and currentState/currentContext
//! receiver reads do not count as passing the key.

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
        let (Some(index), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), index, ctx.types());
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
            diagnostics: Vec::new(),
            const_contexts: crate::lint::const_eval::ConstContexts::of(program),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    fields: Vec<KeyField<'a>>,
    arguments: Vec<(DeclarationIdentity, Span)>,
    state_receivers: Vec<Span>,
    diagnostics: Vec<Diagnostic>,
    const_contexts: crate::lint::const_eval::ConstContexts,
}

fn cached_initializer(expression: Expr<'_>) -> bool {
    for ancestor in expression.syntax().ancestors().skip(1) {
        if FuncExpr::cast(ancestor).is_some() || LocalVarDecl::cast(ancestor).is_some() {
            return false;
        }
        if FieldDecl::cast(ancestor).is_some()
            || matches!(
                TopLevelDecl::cast(ancestor),
                Some(TopLevelDecl::Variable(_))
            )
        {
            return true;
        }
    }
    false
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics.extend(
            self.fields
                .iter()
                .filter(|field| !field.passed)
                .map(|field| {
                    self.ctx.diagnostic("Pass this `GlobalKey` field to a widget.", field.span)
                }),
        );
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if matches!(node, Expr::Call(_) | Expr::New(_))
            && let Some((widget, _, args)) =
                crate::lint::flutter::scoped_constructor_invocation(node, state)
            && state.signatures.is_subtype_of(
                &ResolvedType::interface_identity(widget),
                &DeclarationIdentity::flutter("Widget"),
                &state.model,
            ) == Truth::Yes
            && let Some(key) = args
                .named()
                .find(|argument| argument.name().name() == "key")
            && !cached_initializer(node)
            && !self.const_contexts.contains(&key.value().span())
            && !matches!(key.value().unparenthesized(), Expr::New(node) if node.is_const())
            && let Some((identity, _, _)) =
                crate::lint::flutter::scoped_constructor_invocation(key.value(), state)
            && {
                let ty = ResolvedType::interface_identity(identity);
                state.signatures.is_subtype_of(
                    &ty,
                    &DeclarationIdentity::flutter("GlobalKey"),
                    &state.model,
                ) == Truth::Yes
                    && state.signatures.is_subtype_of(
                        &ty,
                        &DeclarationIdentity::flutter("GlobalObjectKey"),
                        &state.model,
                    ) != Truth::Yes
            }
        {
            self.diagnostics.push(self.ctx.diagnostic("Store this `GlobalKey` outside the widget's key argument.", key.value().span()));
        }
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
            .any(|span| span.contains(node.span()))
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
                && span.contains(node.span())
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
