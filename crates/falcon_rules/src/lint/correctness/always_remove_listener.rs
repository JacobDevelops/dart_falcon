//! Requires Flutter Listenable registrations in State to be removed.
//!
//! Receiver and callback must match: removing a different callback or listening
//! to a different object does not clean up the registration. Disposing the same
//! notifier also removes its listeners. Anonymous callbacks cannot be paired
//! with a different closure. Unresolved APIs and non-State classes are ignored;
//! local declaration spans distinguish shadowed receivers and callbacks.
//! Unresolved expression receivers (including temporary derived animation views)
//! are ignored rather than attributing their listeners to an unrelated field.
//! Local aliases stored in fields and elements of the same iterated collection
//! share receiver identity. Untraceable or ambiguous local ownership is ignored.
//! Unknown assignments, compound writes, and reused loop variables invalidate
//! local ownership. Null assertions do not change a receiver's identity.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AlwaysRemoveListener;
impl Rule for AlwaysRemoveListener {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            added: Vec::new(),
            removed: Vec::new(),
            disposed: Vec::new(),
            aliases: Vec::new(),
        }))
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Reference<'a> {
    text: &'a str,
    declaration: Option<Span>,
    elements: bool,
}
struct Registration<'a> {
    owner: ResolvedType,
    receiver: Reference<'a>,
    callback: Option<Reference<'a>>,
    span: Span,
}
struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    added: Vec<Registration<'a>>,
    removed: Vec<Registration<'a>>,
    disposed: Vec<(ResolvedType, Reference<'a>)>,
    aliases: Vec<(Reference<'a>, Option<Reference<'a>>)>,
}
impl<'a> Collector<'a> {
    fn reference(&self, expression: Expr<'a>, state: &SemanticState<'a>) -> Option<Reference<'a>> {
        let mut expression = expression.unparenthesized();
        while let Expr::NullAssert(assertion) = expression {
            expression = assertion.operand().unparenthesized();
        }
        match expression {
            Expr::Ident(identifier) => Some(Reference {
                text: identifier.name(),
                declaration: state.environment.binding_declaration(identifier.name()),
                elements: false,
            }),
            Expr::Field(field) => {
                if matches!(field.object(), Expr::This(_)) {
                    Some(Reference {
                        text: field.field().name(),
                        declaration: None,
                        elements: false,
                    })
                } else if matches!(field.object(), Expr::Ident(_)) {
                    Some(Reference {
                        text: &self.ctx.source()[expression.span().start..expression.span().end],
                        declaration: match field.object() {
                            Expr::Ident(identifier) => {
                                state.environment.binding_declaration(identifier.name())
                            }
                            _ => None,
                        },
                        elements: false,
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn resolve_receiver(&self, mut receiver: Reference<'a>) -> Option<Reference<'a>> {
        // A declaration span identifies a local, not a State-owned object.
        // Follow only unambiguous aliases; a cycle or unknown local is not proof.
        for _ in 0..=self.aliases.len() {
            if receiver.declaration.is_none() {
                return Some(receiver);
            }
            let mut aliases = self.aliases.iter().filter(|(local, _)| {
                local.text == receiver.text && local.declaration == receiver.declaration
            });
            let (_, target) = aliases.next()?;
            if aliases.any(|(_, other)| other != target) {
                return None;
            }
            let target = (*target)?;
            if receiver.elements && target.elements {
                return None;
            }
            receiver = Reference {
                elements: receiver.elements || target.elements,
                ..target
            };
        }
        None
    }

    fn registration(
        &mut self,
        object: Expr<'a>,
        method: &str,
        args: ArgList<'a>,
        state: &SemanticState<'a>,
    ) {
        let Some(owner) = &state.enclosing_type else {
            return;
        };
        if state.signatures.is_subtype_of(
            owner,
            &DeclarationIdentity::flutter("State"),
            &state.model,
        ) != Truth::Yes
        {
            return;
        }
        if !matches!(method, "addListener" | "removeListener" | "dispose") {
            return;
        }
        let mut ty = state.infer_receiver(object);
        // SDK-less analyses still know State<T>'s widget type through the
        // inheritance graph, even when the framework's widget getter is absent.
        if let Expr::Field(field) = object.unparenthesized()
            && matches!(field.object().unparenthesized(), Expr::Ident(identifier) if identifier.name() == "widget" && state.environment.binding_declaration("widget").is_none())
            && let Some(instance) = state.signatures.as_instance_of(
                owner,
                &DeclarationIdentity::flutter("State"),
                &state.model,
            )
            && let Some(widget) = instance.type_argument(0)
            && let Some(property) =
                state
                    .signatures
                    .instantiated_data_read(&widget, field.field().name(), &state.model)
        {
            ty = property;
        }
        let known_listenable = matches!(&ty, ResolvedType::Interface { identity: DeclarationIdentity::Package { package, name, .. }, .. }
            if package.as_str() == "flutter" && matches!(name.as_str(), "Listenable" | "ValueListenable" | "Animation" | "ChangeNotifier" | "ValueNotifier" | "FocusNode" | "TextEditingController" | "ScrollController" | "PageController" | "TabController" | "AnimationController"));
        if !known_listenable
            && state.signatures.is_subtype_of(
                &ty,
                &DeclarationIdentity::flutter("Listenable"),
                &state.model,
            ) != Truth::Yes
        {
            return;
        }
        let Some(receiver) = self.reference(object, state) else {
            return;
        };
        if method == "dispose" {
            self.disposed.push((owner.clone(), receiver));
            return;
        }
        let Some(callback) = args.positional().next() else {
            return;
        };
        let registration = Registration {
            owner: owner.clone(),
            receiver,
            callback: self.reference(callback, state),
            span: callback.span(),
        };
        if method == "addListener" {
            self.added.push(registration);
        } else {
            self.removed.push(registration);
        }
    }
}
impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        let mut disposed = std::mem::take(&mut self.disposed);
        disposed.retain_mut(|(_, receiver)| {
            let Some(resolved) = self.resolve_receiver(*receiver) else {
                return false;
            };
            *receiver = resolved;
            true
        });
        let mut removed = std::mem::take(&mut self.removed);
        removed.retain_mut(|registration| {
            let Some(resolved) = self.resolve_receiver(registration.receiver) else {
                return false;
            };
            registration.receiver = resolved;
            true
        });
        self.added
            .iter()
            .filter(|added| {
                let Some(receiver) = self.resolve_receiver(added.receiver) else {
                    return false;
                };
                !disposed
                    .iter()
                    .any(|(owner, disposed)| owner == &added.owner && disposed == &receiver)
                    && !removed.iter().any(|removed| {
                        added.callback.is_some()
                            && added.owner == removed.owner
                            && receiver == removed.receiver
                            && added.callback == removed.callback
                    })
            })
            .map(|added| {
                self.ctx.diagnostic("Remove this listener with the same callback when the `State` is disposed.", added.span)
            })
            .collect()
    }
    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        match node {
            Stmt::LocalVar(declaration) => {
                for variable in declaration.declarators() {
                    if let Some(initializer) = variable.initializer()
                        && let Some(target) = self.reference(initializer, state)
                    {
                        self.aliases.push((
                            Reference {
                                text: variable.name().name(),
                                declaration: Some(variable.name().span()),
                                elements: false,
                            },
                            Some(target),
                        ));
                    }
                }
            }
            Stmt::For(statement) => {
                if let Some(ForInit::ForIn(initializer)) = statement.header().init() {
                    let declaration = if initializer.declares_variable() {
                        Some(initializer.name().span())
                    } else {
                        state
                            .environment
                            .binding_declaration(initializer.name().name())
                    };
                    let collection = if initializer.declares_variable() {
                        self.reference(initializer.iterable(), state)
                            .map(|collection| Reference {
                                elements: true,
                                ..collection
                            })
                    } else {
                        None
                    };
                    self.aliases.push((
                        Reference {
                            text: initializer.name().name(),
                            declaration,
                            elements: false,
                        },
                        collection,
                    ));
                }
            }
            _ => {}
        }
    }
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::Assign(assignment) => {
                if let Some(target) = self.reference(assignment.target(), state) {
                    let value = if assignment.op() == AssignOp::Eq {
                        self.reference(assignment.value(), state)
                    } else {
                        None
                    };
                    if target.declaration.is_some() {
                        self.aliases.push((target, value));
                    } else if let Some(value) = value
                        && value.declaration.is_some()
                    {
                        self.aliases.push((value, Some(target)));
                    }
                }
            }
            Expr::Call(call) => {
                if let Expr::Field(field) = call.callee().unparenthesized() {
                    self.registration(field.object(), field.field().name(), call.args(), state);
                }
            }
            Expr::Cascade(cascade) => {
                for section in cascade.sections() {
                    if let Some(CascadeOp::Call(call)) = section.ops().next() {
                        self.registration(cascade.object(), call.name().name(), call.args(), state);
                    }
                }
            }
            _ => {}
        }
    }
}
