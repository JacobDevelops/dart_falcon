//! Requires handling invocations whose declarations carry `@Throws` metadata.
//!
//! A call is handled by an enclosing `try` body with matching catches, or by
//! `@Throws` on its own enclosing executable. Typed contracts require coverage
//! of every declared type; an empty contract requires a catch-all or an empty
//! enclosing contract. Catch and finally bodies do not use their sibling catches,
//! and deferred nested functions do not inherit their declaration site's handlers.
//!
//! `Throws` from `dart_code_metrics_annotations` applies to imported declarations,
//! methods, constructors, callback parameters and annotated local tear-offs.
//! Constant exception sets and generic type literals retain their resolved types.
//! Unknown invocation targets are not guessed, and no implicit SDK contracts
//! or configuration options are introduced.

use std::cell::RefCell;
use std::collections::HashMap;

use falcon_analyze::{
    AnalyzeContext, ConstantContext, ConstantValue, DeclarationIdentity, MemberSelector,
    MemberSelectorKind, QualifiedName, ResolvedType, ResolvedTypeView, Rule, RulePreparation,
    SemanticModel, SemanticRuleVisitor, SemanticState, Truth, TypeRelation, evaluate_constant,
    resolve_call_constructor, resolve_new_constructor, resolved_constant_type,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{
    Annotation, AstChildren, CascadeOp, ClassMember, ConstructorInitializer, ConstructorRef, Expr,
    NodeId, NodeKind, Program, Span, SyntaxNode, TopLevelDecl, TryCatchStmt,
};

use crate::lint::declarations;

/// Requires handling calls whose declarations are annotated with `Throws`.
///
/// Matching catches in enclosing try bodies or a covering `Throws` annotation
/// on the nearest executable handle a call. Typed contracts require coverage
/// of every thrown type; unrestricted contracts require catch-all handling or
/// unrestricted enclosing metadata. Only the DCM `Throws` annotation is accepted,
/// including on callback parameters and annotated local tear-offs.
///
/// Deferred nested executables do not inherit their declaration site's handlers,
/// while closures passed directly as call arguments use the enclosing handlers.
/// Catch and finally bodies do not use their sibling catches. Unresolved targets
/// are ignored and no SDK contracts are inferred.
/// Explicit constructor initializers, redirects, and implicit super calls in
/// source-declared constructors are checked. Fully synthesized constructors have
/// no source executable for the semantic visitor and are not inspected.
#[derive(Default)]
pub struct HandleThrowingInvocations;

impl Rule for HandleThrowingInvocations {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            locals: ctx.local_declarations(program),
            contracts: RefCell::new(HashMap::new()),
            local_contracts: RefCell::new(HashMap::new()),
            diagnostics: Vec::new(),
        }))
    }
}

/// Unknown constant sets must not be mistaken for unrestricted enclosing metadata.
#[derive(Clone)]
enum Throws {
    Any,
    Known(Vec<ResolvedType>),
    Unknown,
}

/// Resolve metadata in the declaration's own library, not the caller's imports.
fn contract(node: SyntaxNode<'_>, context: ConstantContext<'_, '_>) -> Option<Throws> {
    annotation_contract(AstChildren::new(node), context)
}

/// Interpret constructor views and ordinary declaration metadata identically.
fn annotation_contract(
    annotations: AstChildren<'_, Annotation<'_>>,
    context: ConstantContext<'_, '_>,
) -> Option<Throws> {
    let model = context.model;
    for annotation in annotations {
        let dcm_throws = QualifiedName::from_identifiers(annotation.name())
            .and_then(|name| model.resolve_name(name))
            .is_some_and(|identity| {
                model.identities().is_package_declaration(
                    &model.unaliased(identity),
                    "dart_code_metrics_annotations",
                    "Throws",
                )
            });
        if !dcm_throws {
            continue;
        }
        let Some(args) = annotation.args() else {
            return Some(Throws::Any);
        };
        let Some(argument) = args.positional().next() else {
            return Some(Throws::Any);
        };
        let elements = match evaluate_constant(argument, context) {
            Some(ConstantValue::Set(_, elements)) => elements,
            Some(ConstantValue::Map(_, elements)) if elements.is_empty() => Vec::new(),
            _ => return Some(Throws::Unknown),
        };
        let types: Option<Vec<_>> = elements
            .into_iter()
            .map(|element| match element {
                ConstantValue::Type(ty) => Some(resolved_constant_type(&ty)),
                _ => None,
            })
            .collect();
        return Some(match types {
            Some(types) if types.is_empty() => Throws::Any,
            Some(types) => Throws::Known(types),
            None => Throws::Unknown,
        });
    }
    None
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    locals: &'a std::collections::HashMap<usize, NodeId>,
    diagnostics: Vec<Diagnostic>,
    contracts: RefCell<HashMap<DeclarationIdentity, Vec<CachedContract>>>,
    local_contracts: RefCell<HashMap<NodeId, Option<Throws>>>,
}

struct CachedContract {
    kind: MemberSelectorKind,
    is_static: bool,
    name: String,
    throws: Option<Throws>,
}

impl CachedContract {
    fn matches(&self, selector: MemberSelector<'_>) -> bool {
        self.kind == selector.kind
            && self.is_static == selector.is_static
            && self.name == selector.name
    }
}

impl<'a> Collector<'a> {
    fn local_contract(&self, node: SyntaxNode<'a>, state: &SemanticState<'a>) -> Option<Throws> {
        if let Some(cached) = self.local_contracts.borrow().get(&node.id()) {
            return cached.clone();
        }
        let mut context = ConstantContext::new(&state.model, &state.signatures);
        context.owner = state.constant_owner.as_ref();
        context.cache = Some(self.ctx.constant_cache());
        let throws = contract(node, context);
        self.local_contracts
            .borrow_mut()
            .insert(node.id(), throws.clone());
        throws
    }

    /// Inspect only the canonical declaration or selected member, never namesakes.
    fn declaration_contract(
        &self,
        owner: &DeclarationIdentity,
        selector: MemberSelector<'_>,
        state: &SemanticState<'_>,
    ) -> Option<Throws> {
        if let Some(cached) = self
            .contracts
            .borrow()
            .get(owner)
            .and_then(|contracts| contracts.iter().find(|cached| cached.matches(selector)))
        {
            return cached.throws.clone();
        }
        let throws = self.read_declaration_contract(owner, selector, state);
        self.contracts
            .borrow_mut()
            .entry(owner.clone())
            .or_default()
            .push(CachedContract {
                kind: selector.kind,
                is_static: selector.is_static,
                name: selector.name.to_owned(),
                throws: throws.clone(),
            });
        throws
    }

    fn read_declaration_contract(
        &self,
        owner: &DeclarationIdentity,
        selector: MemberSelector<'_>,
        state: &SemanticState<'_>,
    ) -> Option<Throws> {
        let identities = self.ctx.identities()?;
        let (path, parsed, declaration) = identities.source_declaration(owner)?;
        let model = SemanticModel::new(path, parsed, identities, self.ctx.types());
        let mut context = ConstantContext::new(&model, &state.signatures);
        context.owner = Some(owner);
        context.cache = Some(self.ctx.constant_cache());
        if selector.kind == MemberSelectorKind::Constructor {
            let members = match declaration {
                TopLevelDecl::Class(node) => node.element_members(),
                TopLevelDecl::MixinClass(node) => node.element_members(),
                TopLevelDecl::Enum(node) => node.element_members(),
                TopLevelDecl::ExtensionType(node) => node.element_members(),
                _ => return None,
            };
            let constructor = members.constructors().find(|constructor| {
                constructor
                    .constructor_name
                    .map_or("new", |name| name.name())
                    == selector.name
            })?;
            return annotation_contract(constructor.annotations, context);
        }
        if matches!(
            selector.kind,
            MemberSelectorKind::Declaration | MemberSelectorKind::Variable
        ) {
            return contract(declaration.syntax(), context);
        }
        let member = declarations::members_of(declaration)?.find(|member| match member {
            ClassMember::Method(node) => node.name().name() == selector.name,
            ClassMember::Getter(node) => node.name().name() == selector.name,
            ClassMember::Field(node) => node
                .declarators()
                .any(|declarator| declarator.name().name() == selector.name),
            _ => false,
        })?;
        contract(member.syntax(), context)
    }

    /// Bind local and parameter metadata through the shared lexical identity index.
    fn invocation_contract(&self, callee: Expr<'a>, state: &SemanticState<'a>) -> Option<Throws> {
        if state.is_constructor_invocation(callee) {
            let (owner, name) = resolve_call_constructor(callee, &state.model, true)?;
            return self.declaration_contract(
                &state.model.unaliased(owner),
                MemberSelector::new(MemberSelectorKind::Constructor, false, name),
                state,
            );
        }
        let mut callee = callee.unparenthesized();
        let mut fallback = None;
        loop {
            callee = match callee {
                Expr::GenericInstantiation(generic) => generic.target().unparenthesized(),
                Expr::NullAssert(assertion) => assertion.operand().unparenthesized(),
                Expr::Field(field)
                    if field.field().name() == "call"
                        && !state.is_constructor_invocation(field.object()) =>
                {
                    fallback = self.target_contract(callee, state).or(fallback);
                    field.object().unparenthesized()
                }
                _ => break,
            };
        }
        self.target_contract(callee, state).or(fallback)
    }

    fn target_contract(&self, callee: Expr<'a>, state: &SemanticState<'a>) -> Option<Throws> {
        if let Expr::DotShorthand(shorthand) = callee {
            let ResolvedTypeView::Interface { identity, .. } = state.expected()?.view() else {
                return None;
            };
            let name = shorthand.name().name();
            let kind = if name == "new" || state.signatures.has_constructor(identity, name) {
                MemberSelectorKind::Constructor
            } else if state
                .signatures
                .declared_member_facts(identity, name)
                .iter()
                .any(|facts| facts.is_static)
            {
                MemberSelectorKind::Read
            } else {
                return None;
            };
            return self.declaration_contract(
                identity,
                MemberSelector::new(kind, true, name),
                state,
            );
        }
        if let Expr::Ident(identifier) = callee
            && let Some(id) = self.locals.get(&identifier.span().start)
        {
            let declaration = self.ctx.tree().node(self.ctx.source(), *id)?;
            let mut parent = declaration.parent()?;
            if parent.kind() == NodeKind::VarDeclarator {
                parent = parent.parent()?;
            }
            return if matches!(
                parent.kind(),
                NodeKind::FormalParam(_)
                    | NodeKind::LocalVarDecl
                    | NodeKind::LocalFuncDecl
                    | NodeKind::ForInInit
            ) {
                self.local_contract(parent, state)
            } else {
                None
            };
        }
        if let Some(reference) = state.referenced_declaration(callee, false) {
            return self.declaration_contract(&reference.owner, reference.selector, state);
        }
        None
    }

    /// Resolve constructor initializers through the semantic walker's receiver.
    fn constructor_contract(
        &self,
        receiver: Option<&ResolvedType>,
        name: &str,
        state: &SemanticState<'_>,
    ) -> Option<Throws> {
        let ResolvedTypeView::Interface { identity, .. } = receiver?.view() else {
            return None;
        };
        self.declaration_contract(
            identity,
            MemberSelector::new(MemberSelectorKind::Constructor, false, name),
            state,
        )
    }

    /// Accumulate coverage across nested try bodies, stopping at the executable boundary.
    fn handled(&self, node: SyntaxNode<'a>, throws: Throws, state: &SemanticState<'a>) -> bool {
        let restricted = matches!(throws, Throws::Known(_));
        let mut remaining = match throws {
            Throws::Known(types) => types,
            Throws::Any | Throws::Unknown => Vec::new(),
        };
        let mut child = node;
        for ancestor in node.ancestors().skip(1) {
            if let Some(statement) = TryCatchStmt::cast(ancestor)
                && child.id() == statement.body().syntax().id()
            {
                for catch in statement.catches() {
                    let Some(ty) = catch.exception_type() else {
                        return true;
                    };
                    let ty = state.model.resolve_type_in(ty, &state.type_parameters);
                    if ty == state.model.core_type("Object", false)
                        || ty == state.model.core_type("Object", true)
                    {
                        return true;
                    }
                    remaining.retain(|thrown| {
                        state.signatures.relation(
                            thrown,
                            TypeRelation::SubtypeOf(&ty),
                            &state.model,
                        ) != Truth::Yes
                    });
                    if restricted && remaining.is_empty() {
                        return true;
                    }
                }
            }
            if ancestor.kind() == NodeKind::FuncExpr
                && ancestor.parent().is_some_and(|parent| {
                    matches!(parent.kind(), NodeKind::ArgList | NodeKind::NamedArg)
                })
            {
                child = ancestor;
                continue;
            }
            if matches!(
                ancestor.kind(),
                NodeKind::FunctionDecl
                    | NodeKind::MethodDecl
                    | NodeKind::GetterDecl
                    | NodeKind::SetterDecl
                    | NodeKind::ConstructorDecl
                    | NodeKind::PrimaryConstructor
                    | NodeKind::PrimaryConstructorBody
                    | NodeKind::OperatorDecl
                    | NodeKind::LocalFuncDecl
                    | NodeKind::FuncExpr
            ) {
                let boundary = if ancestor.kind() == NodeKind::PrimaryConstructor {
                    ancestor
                        .parent()
                        .and_then(|parent| {
                            AstChildren::<ClassMember>::new(parent).find_map(
                                |member| match member {
                                    ClassMember::PrimaryConstructorBody(body) => {
                                        Some(body.syntax())
                                    }
                                    _ => None,
                                },
                            )
                        })
                        .unwrap_or(ancestor)
                } else {
                    ancestor
                };
                match self.local_contract(boundary, state) {
                    Some(Throws::Any) => return true,
                    Some(Throws::Known(covered)) => {
                        remaining.retain(|thrown| {
                            !covered.iter().any(|ty| {
                                state.signatures.relation(
                                    thrown,
                                    TypeRelation::SubtypeOf(ty),
                                    &state.model,
                                ) == Truth::Yes
                            })
                        });
                    }
                    Some(Throws::Unknown) | None => {}
                }
                return restricted && remaining.is_empty();
            }
            child = ancestor;
        }
        false
    }

    fn check(
        &mut self,
        node: SyntaxNode<'a>,
        span: Span,
        throws: Option<Throws>,
        state: &SemanticState<'a>,
    ) {
        if let Some(throws) = throws
            && !self.handled(node, throws, state)
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "Handle this throwing invocation or declare its thrown types with `@Throws`.",
                span,
            ));
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_constructor_ref(
        &mut self,
        constructor: ConstructorRef<'a>,
        state: &SemanticState<'a>,
    ) {
        let mut explicit_target = false;
        for initializer in constructor.initializers {
            let (receiver, name, keyword) = match initializer {
                ConstructorInitializer::SuperCall(call) => {
                    (state.superclass.as_ref(), call.call_name(), "super")
                }
                ConstructorInitializer::ThisCall(call) => {
                    (state.enclosing_type.as_ref(), call.call_name(), "this")
                }
                _ => continue,
            };
            explicit_target = true;
            let syntax = initializer.syntax();
            let span = name.map_or_else(
                || Span::new(syntax.span().start, syntax.span().start + keyword.len()),
                falcon_syntax::cst::Identifier::span,
            );
            self.check(
                syntax,
                span,
                self.constructor_contract(receiver, name.map_or("new", |name| name.name()), state),
                state,
            );
        }
        if let Some(redirect) = constructor.redirect {
            let throws = resolve_new_constructor(
                redirect.type_(),
                redirect.constructor_name(),
                &state.model,
            )
            .and_then(|(owner, name)| {
                self.declaration_contract(
                    &state.model.unaliased(owner),
                    MemberSelector::new(MemberSelectorKind::Constructor, false, name),
                    state,
                )
            });
            let span = redirect.constructor_name().map_or(
                redirect.type_().span(),
                falcon_syntax::cst::Identifier::span,
            );
            self.check(redirect.syntax(), span, throws, state);
        } else if !constructor.is_factory && !constructor.is_external && !explicit_target {
            self.check(
                constructor.params.syntax(),
                constructor.name_span,
                self.constructor_contract(state.superclass.as_ref(), "new", state),
                state,
            );
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::Call(call) => {
                let callee = call.callee();
                let mut target = callee.unparenthesized();
                while let Expr::GenericInstantiation(generic) = target {
                    target = generic.target().unparenthesized();
                }
                let span = match target {
                    Expr::Field(field) => field.field().span(),
                    _ => target.span(),
                };
                self.check(
                    node.syntax(),
                    span,
                    self.invocation_contract(callee, state),
                    state,
                );
            }
            Expr::New(construction) => {
                let throws = resolve_new_constructor(
                    construction.dart_type(),
                    construction.constructor_name(),
                    &state.model,
                )
                .and_then(|(owner, name)| {
                    self.declaration_contract(
                        &state.model.unaliased(owner),
                        MemberSelector::new(MemberSelectorKind::Constructor, false, name),
                        state,
                    )
                });
                let span = construction.constructor_name().map_or(
                    construction.dart_type().span(),
                    falcon_syntax::cst::Identifier::span,
                );
                self.check(node.syntax(), span, throws, state);
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
        if let CascadeOp::Call(call) = operation {
            let name = call.name();
            let throws = state
                .referenced_member_owner(receiver, name.name(), false)
                .and_then(|owner| {
                    self.declaration_contract(
                        &owner,
                        MemberSelector::new(MemberSelectorKind::Read, false, name.name()),
                        state,
                    )
                });
            self.check(call.syntax(), name.span(), throws, state);
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
