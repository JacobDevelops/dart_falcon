use crate::resolve::ResolvedTypeKind;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::constant_value::LocalConstants;
use crate::resolve::flow::{FlowContext, loop_jumps};
use crate::resolve::flow_writes::FlowWrites;
use crate::{
    AnalyzeContext, ConstantValue, DeclarationIdentity, Feature, InferenceContext, MemberKind,
    ParameterOwner, QualifiedName, ResolvedSignature, ResolvedType, SemanticModel, SignatureIndex,
    TypeEnvironment, TypeParameterId, TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;

use falcon_syntax::cst::visit::{
    Visitor, walk_annotation, walk_expr, walk_expr_chain, walk_formal_param_annotations,
    walk_function_body, walk_pattern, walk_top_level_decl, walk_type_params,
};
use falcon_syntax::cst::*;

/// Local constants retain both source syntax and their declaration-time value.
struct LocalConstant<'a> {
    depth: usize,
    declaration: Span,
    name: String,
    initializer: Expr<'a>,
    value: Option<ConstantValue>,
}

/// Assignment syntax with the shared walker's index-selector classification.
#[derive(Clone, Copy)]
pub struct CascadeAssignment<'a> {
    pub target: Expr<'a>,
    pub value: Expr<'a>,
    pub operator: AssignOp,
    pub indexed: bool,
}

/// A resolved non-local reference, retaining member access and static dispatch.
pub struct ReferencedDeclaration<'a> {
    pub owner: DeclarationIdentity,
    pub selector: crate::MemberSelector<'a>,
}

/// Observes semantic events without recursing into syntax.
///
/// The shared traversal owns scope and inference state for every observer.
pub trait SemanticRuleVisitor<'a> {
    /// Consume this file's collector after the shared traversal.
    #[must_use]
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        Vec::new()
    }

    /// Called once after traversal, with the completed top-level semantic state.
    fn visit_program_end(&mut self, _state: &SemanticState<'a>) {}

    fn visit_expr(&mut self, _node: Expr<'a>, _state: &SemanticState<'a>) {}

    fn visit_call_arguments(
        &mut self,
        _args: ArgList<'a>,
        _positional: &[ResolvedType],
        _named: &HashMap<String, ResolvedType>,
        _state: &SemanticState<'a>,
    ) {
    }

    fn visit_stmt(&mut self, _node: Stmt<'a>, _state: &SemanticState<'a>) {}

    /// Called after a block's statements, while its local bindings remain visible.
    fn visit_block_end(&mut self, _node: Block<'a>, _state: &SemanticState<'a>) {}

    fn visit_constructor(&mut self, _node: ConstructorDecl<'a>, _state: &SemanticState<'a>) {}

    fn visit_constructor_ref(&mut self, _node: ConstructorRef<'a>, _state: &SemanticState<'a>) {}

    /// Called before shared cascade inference advances to the next receiver.
    fn visit_cascade_operation(
        &mut self,
        _operation: CascadeOp<'a>,
        _receiver: &ResolvedType,
        _section: Span,
        _state: &SemanticState<'a>,
    ) {
    }

    /// Assignment targets retain the shared walker's index-selector classification.
    fn visit_cascade_assignment(
        &mut self,
        _assignment: CascadeAssignment<'a>,
        _receiver: &ResolvedType,
        _section: Span,
        _state: &SemanticState<'a>,
    ) {
    }

    fn visit_pattern(
        &mut self,
        _node: Pattern<'a>,
        _matched: Option<&ResolvedType>,
        _state: &SemanticState<'a>,
    ) {
    }
}

struct InferredExpression {
    ty: ResolvedType,
    /// Present when a null-shorted selector has a distinct reached branch.
    reached: Option<ResolvedType>,
}

pub struct SemanticState<'a> {
    pub model: SemanticModel<'a>,
    // Only the single-file fallback owns signatures.
    pub signatures: Cow<'a, SignatureIndex>,
    pub environment: TypeEnvironment,
    pub type_parameters: TypeParameterScope,
    /// Context types of the enclosing expressions; `None` marks a position
    /// without a context type (an untyped declaration's initializer), as
    /// opposed to `Some(Unknown)` for a context falcon cannot resolve.
    expected: Vec<Option<ResolvedType>>,
    return_type: Vec<ResolvedType>,
    matched: Vec<ResolvedType>,
    pub enclosing_type: Option<ResolvedType>,
    /// Lexical constant namespace, distinct from an extension's `this` on-type.
    pub constant_owner: Option<DeclarationIdentity>,
    unnamed_extension_members: Option<AstChildren<'a, ClassMember<'a>>>,
    pub superclass: Option<ResolvedType>,
    globals: &'a HashMap<&'a str, ResolvedType>,
    /// `const` local variables in open scopes, with the environment scope
    /// index that binds them.
    local_constants: Vec<LocalConstant<'a>>,
    /// Variables each loop, switch, `try` and `catch` region writes, keyed by
    /// statement start, and each nested callable's executable writes.
    writes: &'a FlowWrites,
    private_field_promotion: bool,
    inference_depth: std::cell::Cell<usize>,
    inference_cache: std::cell::RefCell<HashMap<NodeId, InferredExpression>>,
    inference_environment: std::cell::RefCell<Option<TypeEnvironment>>,
    pub inside_cascade: bool,
}

impl<'a> SemanticState<'a> {
    pub fn new(program: Program<'a>, ctx: &'a AnalyzeContext<'a>) -> Option<Self> {
        let identities = ctx.identities()?;
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
        let signatures = ctx.signatures().map_or_else(
            || Cow::Owned(SignatureIndex::from_program(program, &model)),
            Cow::Borrowed,
        );
        let private_field_promotion = ctx
            .language_version()
            .supports(Feature::PrivateFieldPromotion);
        Some(Self {
            model,
            signatures,
            environment: TypeEnvironment::with_field_promotions(private_field_promotion),
            type_parameters: TypeParameterScope::default(),
            expected: Vec::new(),
            return_type: Vec::new(),
            matched: Vec::new(),
            enclosing_type: None,
            constant_owner: None,
            unnamed_extension_members: None,
            globals: ctx.globals(program),
            writes: ctx.flow_writes(program),
            superclass: None,
            local_constants: Vec::new(),
            private_field_promotion,
            inference_depth: std::cell::Cell::new(0),
            inference_cache: std::cell::RefCell::new(HashMap::new()),
            inference_environment: std::cell::RefCell::new(None),
            inside_cascade: false,
        })
    }

    /// Resolve a declaration reference in the current lexical and receiver scope.
    /// Local bindings and ambiguous or unknown receivers have no declaration target.
    /// `write` selects setters rather than getters; constructors retain their name.
    pub fn referenced_declaration(
        &self,
        expression: Expr<'a>,
        write: bool,
    ) -> Option<ReferencedDeclaration<'a>> {
        let mut node = expression.unparenthesized();
        while let Expr::GenericInstantiation(instantiation) = node {
            node = instantiation.target().unparenthesized();
        }
        match node {
            Expr::Ident(name) => self.referenced_identifier(name, write),
            Expr::Field(access) => self.referenced_field(access, write),
            _ => None,
        }
    }

    fn static_reference_selector(
        &self,
        owner: &DeclarationIdentity,
        name: &'a str,
        write: bool,
    ) -> Option<crate::MemberSelector<'a>> {
        use crate::{MemberSelector, MemberSelectorKind as Kind};
        self.signatures
            .declared_member_facts(owner, name)
            .iter()
            .find_map(|fact| {
                if !fact.is_static {
                    return None;
                }
                let selector_kind = match fact.kind {
                    crate::MemberKind::Field { writable, .. } if !write || writable => Kind::Field,
                    crate::MemberKind::Setter if write => Kind::Write,
                    crate::MemberKind::Getter | crate::MemberKind::Method if !write => Kind::Read,
                    _ => return None,
                };
                Some(MemberSelector::new(selector_kind, true, name))
            })
            .or_else(|| {
                self.model
                    .identities()
                    .static_member_selector(owner, name, write)
            })
    }

    fn referenced_identifier(
        &self,
        name: Identifier<'a>,
        write: bool,
    ) -> Option<ReferencedDeclaration<'a>> {
        use crate::{MemberSelector, MemberSelectorKind as Kind};
        let kind = if write { Kind::Write } else { Kind::Read };
        let target = |owner, selector| Some(ReferencedDeclaration { owner, selector });
        if self.environment.is_bound(name.name())
            && !self.environment.is_bound_in_root_scope(name.name())
        {
            return None;
        }
        if self
            .unnamed_extension_members
            .as_ref()
            .is_some_and(|members| {
                members.clone().any(|member| match member {
                    ClassMember::Field(field) => field
                        .declarators()
                        .any(|declarator| declarator.name().name() == name.name()),
                    ClassMember::Method(method) => method.name().name() == name.name(),
                    ClassMember::Getter(getter) => getter.name().name() == name.name(),
                    ClassMember::Setter(setter) => setter.name().name() == name.name(),
                    _ => false,
                })
            })
        {
            // Unnamed extensions have no addressable declaration identity,
            // but their own members still shadow the on-type and library.
            return None;
        }
        if let Some(owner) = &self.constant_owner {
            let facts = self.signatures.declared_member_facts(owner, name.name());
            if write
                && !facts.is_empty()
                && facts
                    .iter()
                    .all(|fact| matches!(fact.kind, MemberKind::Getter))
            {
                return None;
            }
            if let Some(fact) = facts.first() {
                if fact.is_static {
                    let selector = self.static_reference_selector(owner, name.name(), write)?;
                    return target(owner.clone(), selector);
                }
                // Extension bodies bind `this` to their on-type, not
                // to the extension declaration supplying the member.
                if !matches!(&self.enclosing_type,
                    Some(ResolvedType { kind: ResolvedTypeKind::Interface { identity, .. } }) if identity == owner)
                {
                    return target(owner.clone(), MemberSelector::new(kind, false, name.name()));
                }
            }
        }
        if matches!((&self.constant_owner, &self.enclosing_type),
            (Some(owner), Some(ResolvedType { kind: ResolvedTypeKind::Interface { identity, .. } })) if owner == identity)
            && self.enclosing_type_declares(name.name())
            && let Some(receiver) = &self.enclosing_type
        {
            let owner = self.referenced_member_owner(receiver, name.name(), write)?;
            return target(owner, MemberSelector::new(kind, false, name.name()));
        }
        let value = self.model.resolve_value(name.name());
        if let Some(owner) = value
            .clone()
            .or_else(|| self.model.resolve_name(name.name()))
            .or_else(|| self.model.resolve_extension(name.name()))
        {
            return target(
                owner,
                MemberSelector::new(
                    if write && value.is_some() {
                        Kind::Write
                    } else {
                        Kind::Declaration
                    },
                    false,
                    "",
                ),
            );
        }
        let receiver = self.enclosing_type.as_ref()?;
        let owner = self.referenced_member_owner(receiver, name.name(), write)?;
        target(owner, MemberSelector::new(kind, false, name.name()))
    }

    fn referenced_field(
        &self,
        access: FieldExpr<'a>,
        write: bool,
    ) -> Option<ReferencedDeclaration<'a>> {
        use crate::{MemberSelector, MemberSelectorKind as Kind};
        let kind = if write { Kind::Write } else { Kind::Read };
        let target = |owner, selector| Some(ReferencedDeclaration { owner, selector });
        let name = access.field().name();
        let mut object = access.object().unparenthesized();
        while let Expr::GenericInstantiation(instantiation) = object {
            object = instantiation.target().unparenthesized();
        }
        // Type-qualified constructor tear-offs precede ordinary static lookup.
        let qualified = match object {
            Expr::Ident(identifier) if !self.environment.is_bound(identifier.name()) => {
                Some(QualifiedName::from(identifier.name()))
            }
            Expr::Field(field) => match field.object().unparenthesized() {
                Expr::Ident(prefix) if !self.environment.is_bound(prefix.name()) => {
                    Some(QualifiedName::prefixed(prefix.name(), field.field().name()))
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(owner) = qualified.and_then(|name| self.model.resolve_name(name)) {
            let owner = self.model.unaliased(owner);
            if self.signatures.constructor(&owner, name).is_some() {
                return target(owner, MemberSelector::new(Kind::Constructor, false, name));
            }
        }
        if matches!(object, Expr::Super(_)) {
            let owner = self.signatures.resolved_super_member_owner(
                self.enclosing_type.as_ref()?,
                name,
                write,
                &self.model,
            )?;
            return target(owner, MemberSelector::new(kind, false, name));
        }
        if let Some(owner) = self.named_extension(object)
            && let Some(selector) = self.static_reference_selector(&owner, name, write)
        {
            return target(owner, selector);
        }
        if let Expr::Call(call) = object
            && let Some(owner) = self.named_extension(call.callee())
        {
            return target(owner, MemberSelector::new(kind, false, name));
        }
        if let Expr::Ident(prefix) = object
            && !self.environment.is_bound(prefix.name())
        {
            let qualified = QualifiedName::prefixed(prefix.name(), name);
            let value = self.model.resolve_value(qualified);
            if let Some(owner) = value
                .clone()
                .or_else(|| self.model.resolve_name(qualified))
                .or_else(|| self.model.resolve_extension(qualified))
            {
                return target(
                    owner,
                    MemberSelector::new(
                        if write && value.is_some() {
                            Kind::Write
                        } else {
                            Kind::Declaration
                        },
                        false,
                        "",
                    ),
                );
            }
            if let Some(owner) = self.model.resolve_name(prefix.name()) {
                let owner = self.model.unaliased(owner);
                let selector = self.static_reference_selector(&owner, name, write)?;
                return target(owner, selector);
            }
        }
        let receiver = self.infer_access_receiver(access.object(), name, access.is_null_safe());
        let owner = self.referenced_member_owner(&receiver, name, write)?;
        target(owner, MemberSelector::new(kind, false, name))
    }

    /// Resolve an instance or implicit-extension member, including cascade members.
    pub fn referenced_member_owner(
        &self,
        receiver: &ResolvedType,
        name: &str,
        write: bool,
    ) -> Option<DeclarationIdentity> {
        if name == "call"
            && (matches!(
                receiver,
                ResolvedType {
                    kind: ResolvedTypeKind::Function { .. }
                }
            ) || receiver.interface("dart:core", "Function"))
        {
            return None;
        }
        if write {
            self.signatures
                .resolved_writable_member_owner(receiver, name, &self.model)
        } else {
            self.signatures
                .resolved_readable_member_owner(receiver, name, &self.model)
        }
        .or_else(|| {
            self.signatures
                .resolved_extension_access_owner(receiver, name, write, &self.model)
        })
    }

    pub fn is_constructor_invocation(&self, callee: Expr<'a>) -> bool {
        crate::resolve_call_constructor(callee, &self.model, true)
            .filter(|_| {
                crate::expression_segments(callee, true).is_some_and(|segments| {
                    segments.first().is_some_and(|head| {
                        !self.environment.is_bound(head)
                            && self.implicit_member_type(head).is_none()
                            && !matches!(&self.enclosing_type,
                                Some(ResolvedType { kind: ResolvedTypeKind::Interface { identity, .. } })
                                    if !self.signatures.declared_member_facts(identity, head).is_empty())
                    })
                })
            })
            .is_some_and(|(identity, name)| {
                self.signatures.has_constructor(&self.model.unaliased(identity), name)
            })
    }

    pub fn is_callable_value_member(&self, receiver: &ResolvedType, name: &str) -> bool {
        if receiver.record_field(name).is_some() {
            return true;
        }
        let facts = match receiver {
            ResolvedType {
                kind: ResolvedTypeKind::Interface { identity, .. },
            } if !self
                .signatures
                .declared_member_facts(identity, name)
                .is_empty() =>
            {
                self.signatures.declared_member_facts(identity, name)
            }
            _ => self
                .signatures
                .resolved_member_facts(receiver, name, &self.model)
                .unwrap_or_default(),
        };
        facts
            .iter()
            .any(|fact| matches!(fact.kind, MemberKind::Field { .. } | MemberKind::Getter))
    }

    /// Analyzer rewrites calls through variables, getters and record fields
    /// into function-expression invocations rather than method invocations.
    pub fn is_function_expression_invocation(&self, callee: Expr<'a>) -> bool {
        match callee {
            Expr::Ident(identifier) => {
                let name = identifier.name();
                if self.environment.is_bound(name) {
                    return !self.environment.is_local_function(name);
                }
                if let Some(
                    receiver @ ResolvedType {
                        kind: ResolvedTypeKind::Interface { identity, .. },
                    },
                ) = &self.enclosing_type
                    && !self
                        .signatures
                        .declared_member_facts(identity, name)
                        .is_empty()
                {
                    return self.is_callable_value_member(receiver, name);
                }
                if let Some(identity) = self.model.resolve_value(name) {
                    return self.signatures.value_type(&identity).is_some()
                        || self.signatures.is_value_getter(&identity);
                }
                self.enclosing_type
                    .as_ref()
                    .is_some_and(|receiver| self.is_callable_value_member(receiver, name))
            }
            Expr::Field(node) => {
                let object = node.object();
                let field = node.field();
                let is_null_safe = node.is_null_safe();
                if let Some(segments) = crate::expression_segments(callee, false)
                    && segments.first().is_some_and(|head| {
                        !self.environment.is_bound(head)
                            && self.implicit_member_type(head).is_none()
                    })
                    && let Some(name) = QualifiedName::from_segments(&segments)
                    && let Some(identity) = self.model.resolve_value(name)
                {
                    return self.signatures.value_type(&identity).is_some()
                        || self.signatures.is_value_getter(&identity);
                }
                if let Some(segments) = crate::expression_segments(object, false)
                    && segments.first().is_some_and(|head| {
                        !self.environment.is_bound(head)
                            && self.implicit_member_type(head).is_none()
                    })
                    && let Some(identity) = self.model.resolve_dotted(&segments)
                    && self
                        .signatures
                        .declared_member_facts(&identity, field.name())
                        .iter()
                        .any(|fact| {
                            matches!(fact.kind, MemberKind::Field { .. } | MemberKind::Getter)
                        })
                {
                    return true;
                }
                self.is_callable_value_member(
                    &self.infer_access_receiver(object, field.name(), is_null_safe),
                    field.name(),
                )
            }
            _ => true,
        }
    }

    pub fn infer(&self, expression: Expr<'a>) -> ResolvedType {
        if matches!(expression, Expr::This(_)) {
            return self.enclosing_type.clone().unwrap_or(ResolvedType::UNKNOWN);
        }
        self.inference_query(|| {
            if !self
                .inference_environment
                .borrow()
                .as_ref()
                .is_some_and(|environment| self.environment.same_flow_state(environment))
            {
                self.inference_cache.borrow_mut().clear();
                *self.inference_environment.borrow_mut() = Some(self.environment.clone());
            }
            let key = expression.syntax().id();
            if let Some(cached) = self.inference_cache.borrow().get(&key) {
                return cached.ty.clone();
            }
            let lookup = |name: &str| self.implicit_member_type(name);
            let context = InferenceContext {
                model: &self.model,
                signatures: &self.signatures,
                type_parameters: &self.type_parameters,
                lookup: &lookup,
                argument_fallback: None,
            };
            let cached = |prefix: Expr<'a>| {
                self.inference_cache
                    .borrow()
                    .get(&prefix.syntax().id())
                    .map(|cached| cached.ty.clone())
            };
            let publish = |prefix: Expr<'a>, ty: &ResolvedType| {
                self.inference_cache.borrow_mut().insert(
                    prefix.syntax().id(),
                    InferredExpression {
                        ty: ty.clone(),
                        reached: None,
                    },
                );
            };
            let ty = match expression {
                Expr::Binary(binary) => self
                    .environment
                    .infer_binary_chain(binary, &context, cached, publish),
                Expr::Conditional(conditional) => Some(self.environment.infer_conditional_chain(
                    conditional,
                    &context,
                    cached,
                    publish,
                )),
                Expr::Field(_) | Expr::Index(_) | Expr::NullAssert(_) => Some(
                    self.environment
                        .infer_selector_chain(
                            expression,
                            &context,
                            None,
                            |prefix| {
                                self.inference_cache
                                    .borrow()
                                    .get(&prefix.syntax().id())
                                    .map(|cached| (cached.ty.clone(), cached.reached.clone()))
                            },
                            |prefix, ty, reached| {
                                self.inference_cache.borrow_mut().insert(
                                    prefix.syntax().id(),
                                    InferredExpression {
                                        ty: ty.clone(),
                                        reached: reached.cloned(),
                                    },
                                );
                            },
                        )
                        .0,
                ),
                _ => None,
            }
            .unwrap_or_else(|| self.infer_with_context(expression, None));
            self.inference_cache
                .borrow_mut()
                .entry(key)
                .or_insert_with(|| InferredExpression {
                    ty: ty.clone(),
                    reached: None,
                });
            ty
        })
    }

    fn inference_query(&self, query: impl FnOnce() -> ResolvedType) -> ResolvedType {
        let depth = self.inference_depth.get();
        self.inference_depth.set(depth + 1);
        let result = query();
        self.inference_depth.set(depth);
        if depth == 0 {
            self.inference_cache.borrow_mut().clear();
            self.inference_environment.borrow_mut().take();
        }
        result
    }

    pub fn infer_with_context(
        &self,
        expression: Expr<'a>,
        expected: Option<&ResolvedType>,
    ) -> ResolvedType {
        self.environment.infer_invocation(
            expression,
            &InferenceContext {
                model: &self.model,
                signatures: &self.signatures,
                type_parameters: &self.type_parameters,
                lookup: &|name| self.implicit_member_type(name),
                argument_fallback: None,
            },
            expected,
        )
    }

    pub fn infer_constant_name(&self, names: PatternNames<'a>) -> ResolvedType {
        self.environment.infer_constant_name(
            names,
            &InferenceContext {
                model: &self.model,
                signatures: &self.signatures,
                type_parameters: &self.type_parameters,
                lookup: &|name| self.implicit_member_type(name),
                argument_fallback: None,
            },
        )
    }

    /// Official omission policy distinguishes generic top/local functions from
    /// ordinary methods and function-valued variables.
    pub fn generic_function_returns_type_parameter(&self, expression: Expr<'a>) -> bool {
        let Expr::Call(node) = expression else {
            return false;
        };
        let callee = node.callee();
        let type_args = node.type_args();
        if !type_args.is_empty() || matches!(callee, Expr::GenericInstantiation(_)) {
            return false;
        }
        match callee {
            Expr::Ident(identifier) if self.environment.is_local_function(identifier.name()) => {
                matches!(self.environment.lookup(identifier.name()),
                    ResolvedType { kind: ResolvedTypeKind::Function { return_type, .. } } if matches!(return_type.as_ref(), ResolvedType { kind: ResolvedTypeKind::TypeParameter { .. } }))
            }
            target => crate::expression_segments(target, true)
                .filter(|segments| {
                    segments
                        .first()
                        .is_some_and(|name| !self.environment.is_bound(name))
                })
                .and_then(|segments| {
                    QualifiedName::from_segments(&segments)
                        .and_then(|name| self.model.resolve_value(name))
                })
                .and_then(|identity| self.signatures.function(&identity))
                .is_some_and(|signature| {
                    matches!(
                        signature.return_type,
                        ResolvedType {
                            kind: ResolvedTypeKind::TypeParameter { .. }
                        }
                    )
                }),
        }
    }

    /// The static type of a member access's receiver: `super` is the
    /// superclass, `x!` the non-nullable type of `x`, anything else
    /// [`Self::infer`].
    pub fn infer_receiver(&self, mut object: Expr<'a>) -> ResolvedType {
        let mut asserted = false;
        loop {
            object = match object {
                Expr::Paren(node) => node.expr(),
                Expr::NullAssert(node) => {
                    asserted = true;
                    node.operand()
                }
                _ => break,
            };
        }
        let ty = if matches!(object, Expr::Super(_)) {
            self.superclass.clone().unwrap_or(ResolvedType::UNKNOWN)
        } else {
            self.infer(object)
        };
        if asserted { ty.non_null() } else { ty }
    }

    pub fn infer_access_receiver(
        &self,
        object: Expr<'a>,
        name: &str,
        is_null_safe: bool,
    ) -> ResolvedType {
        if matches!(
            object,
            Expr::Field(_) | Expr::Index(_) | Expr::NullAssert(_)
        ) {
            return self.inference_query(|| {
                let ty = self.infer(object);
                let reached = self
                    .inference_cache
                    .borrow()
                    .get(&object.syntax().id())
                    .and_then(|cached| cached.reached.clone())
                    .unwrap_or(ty);
                if is_null_safe {
                    reached.non_null()
                } else {
                    reached
                }
            });
        }
        self.environment.infer_access_receiver(
            object,
            name,
            is_null_safe,
            &InferenceContext {
                model: &self.model,
                signatures: &self.signatures,
                type_parameters: &self.type_parameters,
                lookup: &|name| self.implicit_member_type(name),
                argument_fallback: None,
            },
            None,
        )
    }

    #[must_use]
    pub fn callable_return_type(callable: &ResolvedType) -> ResolvedType {
        if let Some(return_type) = callable.function_return_type() {
            return return_type;
        }
        match callable {
            ResolvedType {
                kind: ResolvedTypeKind::Dynamic,
            } => ResolvedType::DYNAMIC,
            ty if ty.interface("dart:core", "Function") => ResolvedType::DYNAMIC,
            _ => ResolvedType::UNKNOWN,
        }
    }

    pub fn cascade_invoke_return_type(
        &self,
        receiver: &ResolvedType,
        type_args: &AstChildren<'a, DartType<'a>>,
        args: ArgList<'a>,
    ) -> ResolvedType {
        self.signatures
            .resolved_member(receiver, "call", &self.model)
            .map_or_else(
                || Self::callable_return_type(receiver),
                |(signature, owner)| {
                    self.instantiate_return_type(signature, &owner, type_args, args, None)
                },
            )
    }

    /// The implicit `this` receiver of an unqualified instance method, unless shadowed.
    pub fn implicit_invocation_receiver(&self, name: &str) -> Option<ResolvedType> {
        if self.environment.is_bound(name) || self.implicit_member_type(name).is_some() {
            return None;
        }
        self.environment
            .implicit_this_method(name, &self.model, &self.signatures)?;
        let receiver = self.environment.receiver();
        if let ResolvedType {
            kind: ResolvedTypeKind::Interface { identity, .. },
        } = &receiver
            && self
                .signatures
                .declared_member_facts(identity, name)
                .iter()
                .any(|facts| facts.is_static)
        {
            return None;
        }
        Some(receiver)
    }

    /// Whether the enclosing type itself declares a member `name`.
    fn enclosing_type_declares(&self, name: &str) -> bool {
        matches!(
            &self.enclosing_type,
            Some(ResolvedType { kind: ResolvedTypeKind::Interface { identity, .. } })
                if !self.signatures.declared_member_facts(identity, name).is_empty()
        )
    }

    /// The type of an unqualified name that is not a local, in lexical lookup
    /// order: a member the enclosing type itself declares, library and imported
    /// declarations, then an inherited field or getter (through `this`) or a
    /// static field.
    pub fn implicit_member_type(&self, name: &str) -> Option<ResolvedType> {
        if self.environment.is_bound(name) {
            return None;
        }
        if let Some(promoted) = self.environment.promoted_this_field(name) {
            return Some(promoted);
        }
        if let Some(owner) = &self.constant_owner
            && !matches!(
                &self.enclosing_type,
                Some(ResolvedType { kind: ResolvedTypeKind::Interface { identity, .. } }) if identity == owner
            )
            && self
                .signatures
                .declared_member_facts(owner, name)
                .iter()
                .any(|fact| fact.is_static)
        {
            return self
                .signatures
                .member(owner, name)
                .map(ResolvedSignature::function_type)
                .or_else(|| self.signatures.field(owner, name).map(|(_, ty)| ty.clone()));
        }
        if let Some(ResolvedType {
            kind: ResolvedTypeKind::Interface { identity, .. },
        }) = &self.enclosing_type
            && self
                .signatures
                .declared_member_facts(identity, name)
                .iter()
                .any(|facts| matches!(facts.kind, crate::MemberKind::Method))
        {
            // Generic tear-offs need the individual expression's context,
            // not the enclosing declaration's expected type.
            return None;
        }
        let member = || {
            let receiver = self.enclosing_type.as_ref()?;
            if name == "call"
                && matches!(
                    receiver,
                    ResolvedType {
                        kind: ResolvedTypeKind::Function { .. }
                    }
                )
            {
                return Some(self.model.core_type("Function", false));
            }
            if let Some(ty) = self
                .signatures
                .instantiated_data_read(receiver, name, &self.model)
            {
                return Some(ty);
            }
            if let ResolvedType {
                kind: ResolvedTypeKind::Interface { identity, .. },
            } = receiver
                && let Some((_, ty)) = self.signatures.field(identity, name)
            {
                return Some(ty.clone());
            }
            None
        };
        if self.enclosing_type_declares(name) {
            return Some(member().unwrap_or(ResolvedType::UNKNOWN));
        }
        if self.model.resolve_value(name).is_some() || self.model.resolve_name(name).is_some() {
            return self
                .globals
                .get(name)
                .cloned()
                .filter(|ty| *ty != ResolvedType::UNKNOWN);
        }
        self.globals.get(name).cloned().or_else(member)
    }

    pub fn expected(&self) -> Option<&ResolvedType> {
        self.expected.last()?.as_ref()
    }

    /// The initializer of the `const` local variable `name` refers to, when
    /// its innermost binding is one.
    pub fn local_constant(&self, name: &str) -> Option<Expr<'a>> {
        let declaration = self.environment.binding_declaration(name)?;
        self.local_constants
            .iter()
            .rev()
            .find(|constant| constant.declaration == declaration)
            .map(|constant| constant.initializer)
    }

    /// Declaration-time values and current promoted types of visible constants.
    pub fn visible_local_constants(&self) -> LocalConstants<'_> {
        self.local_constants
            .iter()
            .filter(|constant| {
                self.environment.binding_declaration(&constant.name) == Some(constant.declaration)
            })
            .map(|constant| {
                (
                    constant.name.as_str(),
                    (&constant.value, self.environment.lookup(&constant.name)),
                )
            })
            .collect()
    }

    /// Whether the current expression's context type is unresolved, rather
    /// than resolved or absent.
    pub fn expected_unknown(&self) -> bool {
        matches!(
            self.expected.last(),
            Some(Some(ResolvedType {
                kind: ResolvedTypeKind::Unknown
            }))
        )
    }

    /// Whether the current expression `node` is implicitly torn off as
    /// `node.call`: an instance with a `call` method in a function-typed (or
    /// `Function`, possibly `FutureOr`-wrapped) context, or such an instance
    /// instantiated with type arguments (`c<int>`). The analyzer wraps such an
    /// expression in an `ImplicitCallReference`.
    pub fn implicit_call_tearoff(&self, node: Expr<'a>) -> bool {
        match node {
            Expr::Field(field) if field.field().name() == "call" => return false,
            Expr::GenericInstantiation(instantiation) => {
                return self.has_call_method(&self.infer(instantiation.target()));
            }
            _ => {}
        }
        let Some(expected) = self.expected() else {
            return false;
        };
        let function_context = |ty: &ResolvedType| {
            matches!(
                ty,
                ResolvedType {
                    kind: ResolvedTypeKind::Function { .. }
                }
            ) || ty.interface("dart:core", "Function")
        };
        let function_context = function_context(expected)
            || expected.interface("dart:async", "FutureOr")
                && expected.arguments().first().is_some_and(function_context);
        function_context && self.has_call_method(&self.infer(node))
    }

    /// Whether instances of `ty` have an instance `call` method.
    fn has_call_method(&self, ty: &ResolvedType) -> bool {
        let is_call =
            |fact: &crate::MemberFacts| fact.kind == crate::MemberKind::Method && !fact.is_static;
        match ty {
            ResolvedType {
                kind: ResolvedTypeKind::TypeParameter { bound, .. },
            } => self.has_call_method(bound),
            ResolvedType {
                kind:
                    ResolvedTypeKind::Function { .. }
                    | ResolvedTypeKind::Unknown
                    | ResolvedTypeKind::Dynamic,
            } => false,
            ResolvedType {
                kind: ResolvedTypeKind::Interface { identity, .. },
            } if self.signatures.type_kind(identity)
                == Some(crate::resolve::TypeKind::ExtensionType) =>
            {
                matches!(
                    self.signatures.interface_member(identity, identity, "call"),
                    Some(Some(fact)) if is_call(fact)
                )
            }
            _ => self
                .signatures
                .resolved_member_facts(ty, "call", &self.model)
                .is_some_and(|facts| facts.iter().any(is_call)),
        }
    }

    pub fn is_own_field(&self, name: &str) -> bool {
        self.environment.is_bound_in_root_scope(name)
            && self.enclosing_type.as_ref().is_some_and(|enclosing| {
                self.signatures
                    .resolved_member_owner(enclosing, name, &self.model)
                    .is_some()
            })
    }

    /// Whether an unqualified read denotes storage rather than an explicit
    /// getter. Follow the same local/member/library order as type inference.
    pub fn is_variable_read(&self, name: &str) -> bool {
        if self.environment.is_bound(name) {
            return !self.environment.is_local_function(name);
        }
        let member_is_field = || {
            self.enclosing_type.as_ref().is_some_and(|receiver| {
                self.signatures
                    .resolved_member_facts(receiver, name, &self.model)
                    .is_some_and(|facts| {
                        facts
                            .iter()
                            .any(|fact| matches!(fact.kind, crate::MemberKind::Field { .. }))
                            && !facts
                                .iter()
                                .any(|fact| fact.kind == crate::MemberKind::Getter)
                    })
            })
        };
        if self.enclosing_type_declares(name) {
            return member_is_field();
        }
        if let Some(identity) = self.model.resolve_value(name) {
            return self.signatures.value_type(&identity).is_some()
                && !self.signatures.is_value_getter(&identity);
        }
        if self.model.resolve_name(name).is_some() {
            return false;
        }
        member_is_field()
    }

    pub fn writable_type(&self, target: Expr<'a>) -> ResolvedType {
        let member = match target {
            Expr::Ident(identifier) if self.environment.is_bound(identifier.name()) => {
                return self.environment.declared_type(identifier.name());
            }
            Expr::Ident(identifier) if !self.environment.is_bound(identifier.name()) => {
                if let Some(ty) = self.globals.get(identifier.name()) {
                    return ty.clone();
                }
                self.enclosing_type
                    .as_ref()
                    .map(|receiver| (receiver.clone(), identifier.name()))
            }
            Expr::Field(node) => {
                let object = node.object();
                let field = node.field();
                let is_null_safe = node.is_null_safe();
                if let Expr::Ident(identifier) = object
                    && !self.environment.is_bound(identifier.name())
                    && let Some(identity) = self.model.resolve_name(identifier.name())
                    && let Some((_, ty)) = self.signatures.field(&identity, field.name())
                {
                    return ty.clone();
                }
                Some((
                    self.infer_access_receiver(object, field.name(), is_null_safe),
                    field.name(),
                ))
            }
            Expr::Index(node) => {
                let object = node.object();
                let is_null_safe = node.is_null_safe();
                return self
                    .signatures
                    .resolved_member(
                        &self.infer_access_receiver(object, "[]=", is_null_safe),
                        "[]=",
                        &self.model,
                    )
                    .and_then(|(signature, substitutions)| {
                        signature
                            .positional
                            .get(1)
                            .map(|ty| self.model.substitute(ty, &substitutions))
                    })
                    .unwrap_or(ResolvedType::UNKNOWN);
            }
            _ => None,
        };
        member
            .and_then(|(receiver, name)| {
                self.signatures
                    .resolved_writable_field(&receiver, name, &self.model)
                    .map(|(ty, substitutions)| self.model.substitute(&ty, &substitutions))
            })
            .unwrap_or_else(|| self.infer(target))
    }

    pub fn named_extension(&self, callee: Expr<'a>) -> Option<crate::DeclarationIdentity> {
        self.environment.named_extension(
            callee,
            &InferenceContext {
                model: &self.model,
                signatures: &self.signatures,
                type_parameters: &self.type_parameters,
                lookup: &|name| self.implicit_member_type(name),
                argument_fallback: None,
            },
        )
    }

    pub fn call_signature(
        &self,
        callee: Expr<'a>,
        type_args: AstChildren<'a, DartType<'a>>,
        args: ArgList<'a>,
        expected: Option<&ResolvedType>,
    ) -> Option<ResolvedSignature> {
        let target = self.call_target(callee, type_args)?;
        let substitutions = self.signature_substitutions_with_result(
            target.inference_signature(),
            &target.type_args,
            args,
            expected,
            None,
        );
        let return_type = self
            .model
            .substitute(target.inference_signature().2, &substitutions);
        let call_parameters = target
            .constructor_result
            .as_ref()
            .map(|_| target.signature.owner_parameters.clone());
        let signature = match target.signature {
            Cow::Borrowed(signature) => {
                let (positional, named) = self.substituted_parameters(signature, &substitutions);
                ResolvedSignature {
                    owner_parameters: signature.owner_parameters.clone(),
                    call_parameters: call_parameters
                        .unwrap_or_else(|| signature.call_parameters.clone()),
                    shape: signature.substituted_shape(&self.model, &substitutions),
                    optional_positional: signature.optional_positional,
                    positional,
                    named,
                    return_type,
                    defaults: signature.defaults.clone(),
                    positional_parameter_names: signature.positional_parameter_names.clone(),
                    named_parameter_names: Arc::clone(&signature.named_parameter_names),
                    non_optional_defaults: signature.non_optional_defaults.clone(),
                }
            }
            Cow::Owned(mut signature) => {
                signature.shape = signature.substituted_shape(&self.model, &substitutions);
                for ty in &mut signature.positional {
                    *ty = self.model.substitute(ty, &substitutions);
                }
                for ty in signature.named.values_mut() {
                    *ty = self.model.substitute(ty, &substitutions);
                }
                signature.return_type = return_type;
                if let Some(parameters) = call_parameters {
                    signature.call_parameters = parameters;
                }
                signature
            }
        };
        Some(signature)
    }

    pub fn call_return_type(
        &self,
        callee: Expr<'a>,
        type_args: AstChildren<'a, DartType<'a>>,
        args: ArgList<'a>,
        expected: Option<&ResolvedType>,
    ) -> Option<ResolvedType> {
        let target = self.call_target(callee, type_args)?;
        let substitutions = self.signature_substitutions_with_result(
            target.inference_signature(),
            &target.type_args,
            args,
            expected,
            None,
        );
        Some(
            self.model
                .substitute(target.inference_signature().2, &substitutions),
        )
    }

    pub fn call_parameter_types(
        &self,
        callee: Expr<'a>,
        type_args: AstChildren<'a, DartType<'a>>,
        args: ArgList<'a>,
        expected: Option<&ResolvedType>,
    ) -> Option<(Vec<ResolvedType>, HashMap<String, ResolvedType>)> {
        let target = self.call_target(callee, type_args)?;
        let mut argument_contexts = HashMap::new();
        let substitutions = self.signature_substitutions_with_result(
            target.inference_signature(),
            &target.type_args,
            args,
            expected,
            Some(&mut argument_contexts),
        );
        let (mut positional, mut named) =
            self.substituted_parameters(&target.signature, &substitutions);
        for (parameter, argument) in positional.iter_mut().zip(args.positional()) {
            if let Some(context) = argument_contexts.remove(&argument.span().start) {
                *parameter = context;
            }
        }
        for argument in args.named() {
            if let Some(context) = argument_contexts.remove(&argument.value().span().start) {
                named.insert(argument.name().name().to_owned(), context);
            }
        }
        Some((positional, named))
    }

    pub fn call_target<'s>(
        &'s self,
        callee: Expr<'a>,
        type_args: AstChildren<'a, DartType<'a>>,
    ) -> Option<crate::ResolvedInvocation<'s, 'a>> {
        self.environment.resolved_invocation(
            callee,
            type_args,
            &InferenceContext {
                model: &self.model,
                signatures: &self.signatures,
                type_parameters: &self.type_parameters,
                lookup: &|name| self.implicit_member_type(name),
                argument_fallback: None,
            },
        )
    }

    /// Resolve a constructor on an already instantiated receiver, including
    /// superclass type arguments supplied by the enclosing declaration.
    fn constructor_parameter_types(
        &self,
        receiver: &ResolvedType,
        name: &str,
    ) -> Option<(Vec<ResolvedType>, HashMap<String, ResolvedType>)> {
        let ResolvedType {
            kind: ResolvedTypeKind::Interface { identity, .. },
        } = receiver
        else {
            return None;
        };
        let signature = self.signatures.constructor(identity, name)?;
        let substitutions = signature
            .owner_parameters
            .iter()
            .cloned()
            .zip(receiver.type_arguments())
            .collect();
        Some(self.substituted_parameters(signature, &substitutions))
    }

    /// Infer call substitutions while preserving the resolved receiver's owner arguments.
    pub fn signature_substitutions(
        &self,
        signature: &ResolvedSignature,
        owner: &FxHashMap<TypeParameterId, ResolvedType>,
        type_args: &AstChildren<'a, DartType<'a>>,
        args: ArgList<'a>,
        expected: Option<&ResolvedType>,
    ) -> FxHashMap<TypeParameterId, ResolvedType> {
        self.signature_substitutions_with_result(
            (
                signature,
                &signature.call_parameters,
                &signature.return_type,
                owner,
            ),
            type_args,
            args,
            expected,
            None,
        )
    }

    fn signature_substitutions_with_result(
        &self,
        signature: (
            &ResolvedSignature,
            &[TypeParameterId],
            &ResolvedType,
            &FxHashMap<TypeParameterId, ResolvedType>,
        ),
        type_args: &AstChildren<'a, DartType<'a>>,
        args: ArgList<'a>,
        expected: Option<&ResolvedType>,
        argument_contexts: Option<&mut HashMap<usize, ResolvedType>>,
    ) -> FxHashMap<TypeParameterId, ResolvedType> {
        let context = InferenceContext {
            model: &self.model,
            signatures: &self.signatures,
            type_parameters: &self.type_parameters,
            lookup: &|name| self.implicit_member_type(name),
            argument_fallback: None,
        };
        self.environment.infer_signature_substitutions_with_result(
            signature,
            args,
            type_args,
            &context,
            expected,
            argument_contexts,
        )
    }

    fn instantiate_return_type(
        &self,
        signature: &ResolvedSignature,
        owner: &FxHashMap<TypeParameterId, ResolvedType>,
        type_args: &AstChildren<'a, DartType<'a>>,
        args: ArgList<'a>,
        expected: Option<&ResolvedType>,
    ) -> ResolvedType {
        let substitutions =
            self.signature_substitutions(signature, owner, type_args, args, expected);
        self.model
            .substitute(&signature.return_type, &substitutions)
    }

    pub fn substituted_parameters(
        &self,
        signature: &ResolvedSignature,
        substitutions: &FxHashMap<TypeParameterId, ResolvedType>,
    ) -> (Vec<ResolvedType>, HashMap<String, ResolvedType>) {
        let positional = signature
            .positional
            .iter()
            .map(|ty| self.model.substitute(ty, substitutions))
            .collect();
        let named = signature
            .named
            .iter()
            .map(|(name, ty)| (name.clone(), self.model.substitute(ty, substitutions)))
            .collect();
        (positional, named)
    }
}

/// The receiver of one explicit `.call` access, without traversing other syntax.
#[must_use]
pub fn explicit_call_receiver(callee: Expr<'_>) -> Option<Expr<'_>> {
    if let Expr::Field(field) = callee
        && field.field().name() == "call"
    {
        Some(field.object())
    } else {
        None
    }
}

pub fn visit_program<'a, V: SemanticRuleVisitor<'a> + ?Sized>(
    visitor: &mut V,
    program: Program<'a>,
    state: SemanticState<'a>,
) -> SemanticState<'a> {
    let mut walker = ScopedWalker {
        visitor,
        state,
        assigning_pattern: false,
        break_targets: Vec::new(),
        primary_environment: None,
        primary_body_environment: None,
        chain_result: None,
        chain_result_needed: false,
    };
    walker.visit_program(program);
    walker.visitor.visit_program_end(&walker.state);
    walker.state
}

/// The type a super parameter `param` of the constructor taking `params`
/// inherits from the super constructor `constructor_name` of `superclass`.
/// Named super parameters match by name; positional ones by their position
/// among the positional super parameters (a super call cannot also pass
/// positional arguments).
#[must_use]
pub fn resolved_super_param_type<'a>(
    param: FormalParam<'a>,
    params: FormalParamList<'a>,
    superclass: &ResolvedType,
    constructor_name: &str,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> Option<ResolvedType> {
    let ResolvedType {
        kind: ResolvedTypeKind::Interface { identity, .. },
    } = superclass
    else {
        return None;
    };
    let signature = signatures.constructor(identity, constructor_name)?;
    let ty = if matches!(param.kind(), ParamKind::Named { .. }) {
        signature.named.get(param.name().name())?
    } else {
        let index = params
            .positional()
            .filter(|positional| positional.initializing() == Some(InitializingFormal::Super))
            .position(|positional| positional.syntax().id() == param.syntax().id())?;
        signature.positional.get(index)?
    };
    let substitutions = signature
        .owner_parameters
        .iter()
        .cloned()
        .zip(superclass.type_arguments())
        .collect();
    Some(model.substitute(ty, &substitutions))
}

struct ScopedWalker<'visitor, 'a, V: ?Sized> {
    visitor: &'visitor mut V,
    state: SemanticState<'a>,
    assigning_pattern: bool,
    /// Open `break` targets of the current callable, innermost last.
    break_targets: Vec<BreakTarget>,
    /// All primary formals are visible in constructor initializers.
    primary_environment: Option<TypeEnvironment>,
    /// Body blocks bind only non-declaring, non-initializing primary formals.
    primary_body_environment: Option<TypeEnvironment>,
    chain_result: Option<ResolvedType>,
    chain_result_needed: bool,
}

/// A statement a `break` may leave, and the states reaching it by `break`.
struct BreakTarget {
    /// The statement's label; `None` for an unlabeled loop or switch.
    label: Option<String>,
    /// Whether an unlabeled `break` leaves it (a loop or switch).
    unlabeled: bool,
    states: Vec<TypeEnvironment>,
}

/// The syntax of a callable [`ScopedWalker::visit_function`] walks.
struct Callable<'a> {
    /// The start of a local function or function literal.
    nested: Option<usize>,
    type_parameters: AstChildren<'a, TypeParam<'a>>,
    params: Option<CallableParams<'a>>,
    return_type: Option<DartType<'a>>,
    body: Option<FunctionBody<'a>>,
    async_marker: AsyncMarker,
}

enum CallableParams<'a> {
    List(FormalParamList<'a>),
    Setter(SetterDecl<'a>),
}

enum BinaryChainPending<'a> {
    Left {
        binary: BinaryExpr<'a>,
        result_needed: bool,
    },
    Right {
        binary: BinaryExpr<'a>,
        left: ResolvedType,
        before: Option<TypeEnvironment>,
        left_effects: bool,
        result_needed: bool,
    },
}

impl<'a, V: SemanticRuleVisitor<'a> + ?Sized> ScopedWalker<'_, 'a, V> {
    fn visit_selector_chain(&mut self, root: Expr<'a>) {
        let mut spine = Vec::new();
        let mut operand = root;
        loop {
            let child = match operand {
                Expr::Field(field) => field.object(),
                Expr::Index(index) => index.object(),
                Expr::NullAssert(assertion) => assertion.operand(),
                _ => break,
            };
            spine.push((operand, false));
            operand = child;
        }
        let mut dot_shorthand = crate::expression_has_dot_shorthand_base(operand);
        for (expression, receiver_dot_shorthand) in spine.iter_mut().rev() {
            *receiver_dot_shorthand = dot_shorthand;
            dot_shorthand &= matches!(expression, Expr::Field(_));
        }
        for (expression, receiver_dot_shorthand) in &spine {
            self.visitor.visit_expr(*expression, &self.state);
            let expected = match expression {
                Expr::Field(_) => self
                    .state
                    .expected()
                    .filter(|expected| {
                        !matches!(expected.kind, ResolvedTypeKind::Unknown)
                            && *receiver_dot_shorthand
                    })
                    .cloned(),
                Expr::Index(_) => Some(ResolvedType::UNKNOWN),
                Expr::NullAssert(_) => Some(
                    self.state
                        .expected()
                        .map_or(ResolvedType::UNKNOWN, |ty| ty.with_nullable(true)),
                ),
                _ => unreachable!("selector spine"),
            };
            self.state.expected.push(expected);
        }
        self.visit_expr(operand);
        for (expression, _) in spine.into_iter().rev() {
            if let Expr::Index(index) = expression {
                self.visit_expr(index.index());
            }
            self.state.expected.pop();
            FlowContext {
                model: &self.state.model,
                signatures: &self.state.signatures,
                type_parameters: &self.state.type_parameters,
                field_promotions: self.state.private_field_promotion,
            }
            .finish_expression(&mut self.state.environment, expression, None);
        }
    }

    fn visit_binary_chain(&mut self, root: BinaryExpr<'a>) {
        let root_needed = std::mem::take(&mut self.chain_result_needed);
        let mut pending = Vec::new();
        let mut effects = false;
        let mut evaluated = None;
        walk_expr_chain(self, Expr::Binary(root), |walker, expression| {
            // Finish completed parents before preparing the next sibling's context.
            while matches!(pending.last(), Some(BinaryChainPending::Right { .. })) {
                (evaluated, effects) =
                    walker.finish_binary_chain(pending.pop().unwrap(), evaluated.take(), effects);
            }
            if let Some(BinaryChainPending::Left {
                binary,
                result_needed,
            }) = pending.last()
                && binary.right().syntax().id() == expression.syntax().id()
            {
                let binary = *binary;
                let result_needed = *result_needed;
                pending.pop();
                // Dart evaluates the receiver before the RHS can write a local.
                let left = evaluated.take().unwrap_or(ResolvedType::UNKNOWN);
                pending.push(walker.prepare_binary_chain_right(
                    binary,
                    left,
                    effects,
                    result_needed,
                ));
            }
            let (result_needed, context) = walker.binary_chain_context(pending.last(), root_needed);
            if let Expr::Binary(binary) = expression {
                walker.visitor.visit_expr(expression, &walker.state);
                pending.push(BinaryChainPending::Left {
                    binary,
                    result_needed,
                });
                walker.state.expected.push(
                    binary
                        .op()
                        .operator_name()
                        .is_none()
                        .then_some(ResolvedType::UNKNOWN),
                );
                true
            } else {
                (evaluated, effects) =
                    walker.visit_binary_chain_operand(expression, result_needed, context.as_ref());
                false
            }
        });
        while let Some(parent) = pending.pop() {
            (evaluated, effects) = self.finish_binary_chain(parent, evaluated.take(), effects);
        }
        self.chain_result = evaluated;
    }

    fn prepare_binary_chain_right(
        &mut self,
        binary: BinaryExpr<'a>,
        left: ResolvedType,
        effects: bool,
        result_needed: bool,
    ) -> BinaryChainPending<'a> {
        self.state.expected.pop();
        let before = binary
            .op()
            .operator_name()
            .is_none()
            .then(|| self.state.environment.clone());
        let expected = match binary.op() {
            BinaryOp::And | BinaryOp::Or => {
                // Only effectful prefixes need their promotions and invalidating writes replayed.
                if effects {
                    self.assume_condition(binary.left(), binary.op() == BinaryOp::And);
                }
                ResolvedType::UNKNOWN
            }
            BinaryOp::NullCoalesce => self
                .state
                .expected()
                .filter(|ty| {
                    !matches!(
                        ty,
                        ResolvedType {
                            kind: ResolvedTypeKind::Unknown | ResolvedTypeKind::Dynamic
                        }
                    )
                })
                .cloned()
                .unwrap_or_else(|| left.clone()),
            _ => self
                .state
                .signatures
                .resolved_member(
                    &left,
                    binary.op().operator_name().expect("operator chain"),
                    &self.state.model,
                )
                .and_then(|(signature, owner)| {
                    signature
                        .positional
                        .first()
                        .map(|ty| self.state.model.substitute(ty, &owner))
                })
                .unwrap_or(ResolvedType::UNKNOWN),
        };
        self.state.expected.push(Some(expected));
        BinaryChainPending::Right {
            binary,
            left,
            before,
            left_effects: effects,
            result_needed,
        }
    }

    fn binary_chain_context(
        &self,
        pending: Option<&BinaryChainPending<'a>>,
        root_needed: bool,
    ) -> (bool, Option<ResolvedType>) {
        match pending {
            Some(BinaryChainPending::Left { binary, .. }) => {
                (!matches!(binary.op(), BinaryOp::And | BinaryOp::Or), None)
            }
            Some(BinaryChainPending::Right {
                binary,
                result_needed,
                left,
                ..
            }) => (
                *result_needed
                    && !matches!(
                        binary.op(),
                        BinaryOp::And
                            | BinaryOp::Or
                            | BinaryOp::EqEq
                            | BinaryOp::NotEq
                            | BinaryOp::Lt
                            | BinaryOp::Gt
                            | BinaryOp::LtEq
                            | BinaryOp::GtEq
                    ),
                if binary.op() == BinaryOp::NullCoalesce {
                    Some(left.non_null()).filter(|ty| *ty != ResolvedType::UNKNOWN)
                } else {
                    self.state.expected().cloned()
                },
            ),
            None => (root_needed, None),
        }
    }

    fn visit_binary_chain_operand(
        &mut self,
        expression: Expr<'a>,
        result_needed: bool,
        context: Option<&ResolvedType>,
    ) -> (Option<ResolvedType>, bool) {
        let parenthesized_chain = matches!(expression, Expr::Paren(_))
            && matches!(expression.unparenthesized(), Expr::Binary(_));
        // Optional RHS typing depends on its outer context, unlike operator-chain results.
        let contextual_optional = context.is_some_and(|ty| {
            !matches!(
                ty.kind,
                ResolvedTypeKind::Unknown | ResolvedTypeKind::Dynamic
            )
        }) && matches!(expression.unparenthesized(), Expr::Binary(binary)
            if binary.op() == BinaryOp::NullCoalesce);
        let mut evaluated =
            (result_needed && (!parenthesized_chain || contextual_optional)).then(|| {
                if let Some(context) = context.filter(|ty| {
                    !matches!(
                        ty.kind,
                        ResolvedTypeKind::Unknown | ResolvedTypeKind::Dynamic
                    )
                }) {
                    self.state.infer_with_context(expression, Some(context))
                } else {
                    self.state.infer(expression)
                }
            });
        if parenthesized_chain {
            self.chain_result = None;
            self.chain_result_needed = result_needed && !contextual_optional;
        }
        self.visit_expr(expression);
        if parenthesized_chain && !contextual_optional {
            evaluated = self.chain_result.take();
        }
        let effects = !matches!(
            expression,
            Expr::Ident(_)
                | Expr::IntLit(_)
                | Expr::DoubleLit(_)
                | Expr::BoolLit(_)
                | Expr::NullLit(_)
        );
        (evaluated, effects)
    }

    fn finish_binary_chain(
        &mut self,
        pending: BinaryChainPending<'a>,
        right: Option<ResolvedType>,
        right_effects: bool,
    ) -> (Option<ResolvedType>, bool) {
        let BinaryChainPending::Right {
            binary,
            left,
            before,
            left_effects,
            result_needed,
        } = pending
        else {
            unreachable!("binary right operand was not visited");
        };
        let evaluated = result_needed.then(|| match binary.op() {
            BinaryOp::NullCoalesce => crate::resolve::null_coalesce_result(
                &left,
                &right.expect("evaluated right operand"),
                &self.state.signatures,
                &self.state.model,
            ),
            BinaryOp::EqEq
            | BinaryOp::NotEq
            | BinaryOp::Lt
            | BinaryOp::Gt
            | BinaryOp::LtEq
            | BinaryOp::GtEq
            | BinaryOp::And
            | BinaryOp::Or => self.state.model.core_type("bool", false),
            op => crate::resolve::binary_result(
                op,
                &left,
                &right.expect("evaluated right operand"),
                &self.state.signatures,
                &self.state.model,
            ),
        });
        self.state.expected.pop();
        if let Some(before) = before {
            self.state.environment = before.join(&self.state.environment);
        }
        FlowContext {
            model: &self.state.model,
            signatures: &self.state.signatures,
            type_parameters: &self.state.type_parameters,
            field_promotions: self.state.private_field_promotion,
        }
        .finish_expression(&mut self.state.environment, Expr::Binary(binary), None);
        (
            evaluated,
            left_effects
                || right_effects
                || matches!(binary.op(), BinaryOp::EqEq | BinaryOp::NotEq),
        )
    }

    fn visit_conditional_chain(&mut self, root: ConditionalExpr<'a>) {
        enum Pending<'a> {
            Enter(Expr<'a>),
            Condition(ConditionalExpr<'a>),
            Then(ConditionalExpr<'a>, TypeEnvironment),
            Else(ConditionalExpr<'a>, TypeEnvironment),
            Finish(Expr<'a>),
        }
        let mut pending = vec![Pending::Enter(Expr::Conditional(root))];
        while let Some(next) = pending.pop() {
            match next {
                Pending::Enter(Expr::Conditional(conditional)) => {
                    self.visitor
                        .visit_expr(Expr::Conditional(conditional), &self.state);
                    self.state.expected.push(None);
                    pending.push(Pending::Condition(conditional));
                    pending.push(Pending::Enter(conditional.condition()));
                }
                Pending::Enter(Expr::Paren(paren)) => {
                    self.visitor.visit_expr(Expr::Paren(paren), &self.state);
                    pending.push(Pending::Finish(Expr::Paren(paren)));
                    pending.push(Pending::Enter(paren.expr()));
                }
                Pending::Enter(expression) => self.visit_expr(expression),
                Pending::Condition(conditional) => {
                    self.state.expected.pop();
                    let before = self.state.environment.clone();
                    self.assume_condition(conditional.condition(), true);
                    pending.push(Pending::Then(conditional, before));
                    // Expression branches own no bindings, so nested ternaries need no extra scope.
                    pending.push(Pending::Enter(conditional.then_expr()));
                }
                Pending::Then(conditional, before) => {
                    let then_environment = std::mem::replace(&mut self.state.environment, before);
                    self.assume_condition(conditional.condition(), false);
                    pending.push(Pending::Else(conditional, then_environment));
                    pending.push(Pending::Enter(conditional.else_expr()));
                }
                Pending::Else(conditional, then_environment) => {
                    FlowContext::join_branches(
                        &mut self.state.environment,
                        then_environment,
                        matches!(conditional.then_expr(), Expr::Throw(_)),
                        matches!(conditional.else_expr(), Expr::Throw(_)),
                    );
                    pending.push(Pending::Finish(Expr::Conditional(conditional)));
                }
                Pending::Finish(expression) => {
                    FlowContext {
                        model: &self.state.model,
                        signatures: &self.state.signatures,
                        type_parameters: &self.state.type_parameters,
                        field_promotions: self.state.private_field_promotion,
                    }
                    .finish_expression(
                        &mut self.state.environment,
                        expression,
                        None,
                    );
                }
            }
        }
    }

    fn with_fresh_environment(&mut self, f: impl FnOnce(&mut Self)) {
        let saved = std::mem::replace(
            &mut self.state.environment,
            TypeEnvironment::with_field_promotions(self.state.private_field_promotion),
        );
        let saved_constants = std::mem::take(&mut self.state.local_constants);
        f(self);
        self.state.environment = saved;
        self.state.local_constants = saved_constants;
    }

    fn with_scope(&mut self, f: impl FnOnce(&mut Self)) {
        self.state.environment.push_scope();
        f(self);
        self.state.environment.pop_scope();
        let depth = self.state.environment.depth();
        self.state
            .local_constants
            .retain(|constant| constant.depth < depth);
    }

    fn with_primary_environment(&mut self, f: impl FnOnce(&mut Self)) {
        let Some(environment) = self.primary_environment.take() else {
            f(self);
            return;
        };
        let outer = std::mem::replace(&mut self.state.environment, environment);
        self.with_scope(f);
        self.primary_environment = Some(std::mem::replace(&mut self.state.environment, outer));
    }

    fn assume_condition(&mut self, expression: Expr<'a>, truth: bool) {
        FlowContext {
            model: &self.state.model,
            signatures: &self.state.signatures,
            type_parameters: &self.state.type_parameters,
            field_promotions: self.state.private_field_promotion,
        }
        .assume_condition(&mut self.state.environment, expression, truth);
    }

    fn assume_pattern(&mut self, value: Expr<'a>, pattern: Pattern<'a>, truth: bool) {
        FlowContext {
            model: &self.state.model,
            signatures: &self.state.signatures,
            type_parameters: &self.state.type_parameters,
            field_promotions: self.state.private_field_promotion,
        }
        .assume_pattern(&mut self.state.environment, value, pattern, truth);
    }

    /// Leaves the successful match active and returns the failed pattern
    /// or guard state, without the pattern's new bindings.
    fn visit_pattern_condition(
        &mut self,
        value: Expr<'a>,
        pattern: Pattern<'a>,
        guard: Option<Expr<'a>>,
        matched: ResolvedType,
    ) -> TypeEnvironment {
        let before = self.state.environment.clone();
        self.assume_pattern(value, pattern, false);
        let mut unmatched = std::mem::replace(&mut self.state.environment, before);
        self.assume_pattern(value, pattern, true);
        self.visit_matched_pattern(pattern, matched);
        if let Some(guard) = guard {
            self.visit_expr(guard);
            let tested = self.state.environment.clone();
            self.assume_condition(guard, false);
            unmatched = unmatched.join(&self.state.environment);
            self.state.environment = tested;
            self.assume_condition(guard, true);
        }
        unmatched
    }

    fn statement_exits(&self, statement: Stmt<'a>) -> bool {
        FlowContext {
            model: &self.state.model,
            signatures: &self.state.signatures,
            type_parameters: &self.state.type_parameters,
            field_promotions: self.state.private_field_promotion,
        }
        .statement_exits(&self.state.environment, statement)
    }

    fn visit_branches(
        &mut self,
        condition: Expr<'a>,
        then_branch: impl FnOnce(&mut Self),
        else_branch: impl FnOnce(&mut Self),
        then_exits: bool,
        else_exits: bool,
    ) {
        let before = self.state.environment.clone();
        self.assume_condition(condition, true);
        self.with_scope(then_branch);
        let then_environment = std::mem::replace(&mut self.state.environment, before);
        self.assume_condition(condition, false);
        self.with_scope(else_branch);
        FlowContext::join_branches(
            &mut self.state.environment,
            then_environment,
            then_exits,
            else_exits,
        );
    }

    /// Where control may re-enter `region` (a loop's back edge, a `catch`, a
    /// `finally`, a `continue`d switch case): promotions of the variables it
    /// writes are forgotten, and those its closures write stay unpromotable.
    fn enter_region(&mut self, region: usize) {
        let state = &mut self.state;
        if let Some(names) = state.writes.region_writes.get(&region) {
            for name in names {
                state.environment.demote(name);
            }
        }
        if let Some(names) = state.writes.captured_region_writes.get(&region) {
            for name in names {
                state.environment.mark_captured(name);
            }
        }
    }

    /// Visits a statement `break` may leave, then joins its `break` states
    /// into the state after it. When `statement` cannot complete normally,
    /// only the `break` states reach its end.
    fn with_break_target(
        &mut self,
        label: Option<&str>,
        unlabeled: bool,
        statement: Stmt<'a>,
        visit: impl FnOnce(&mut Self),
    ) {
        let depth = self.state.environment.depth();
        self.break_targets.push(BreakTarget {
            label: label.map(str::to_owned),
            unlabeled,
            states: Vec::new(),
        });
        visit(self);
        let states = self.break_targets.pop().expect("break target").states;
        // Collected breaks are the only exits of an unconditional loop.
        let condition_exit = match statement {
            Stmt::While(statement) => {
                !matches!(statement.condition(), Expr::BoolLit(value) if value.value())
            }
            Stmt::DoWhile(statement) => {
                !matches!(statement.condition(), Expr::BoolLit(value) if value.value())
            }
            Stmt::For(statement)
                if matches!(
                    statement.header().init(),
                    None | Some(ForInit::VarDecl(_) | ForInit::Exprs(_))
                ) =>
            {
                statement.header().condition().is_some_and(
                    |condition| !matches!(condition, Expr::BoolLit(value) if value.value()),
                )
            }
            _ => true,
        };
        let completes = condition_exit && !self.statement_exits(statement);
        FlowContext::join_paths(
            &mut self.state.environment,
            states.into_iter().map(|state| (Cow::Owned(state), true)),
            depth,
            completes,
        );
    }

    fn record_break(&mut self, label: Option<Identifier<'a>>) {
        let target = self
            .break_targets
            .iter_mut()
            .rev()
            .find(|target| match label {
                Some(label) => target.label.as_deref() == Some(label.name()),
                None => target.unlabeled,
            });
        if let Some(target) = target {
            target.states.push(self.state.environment.clone());
        }
    }

    /// Enters a loop at `region`, returning its entry and condition-false
    /// states. `with_break_target` excludes a nonexistent unconditional exit.
    fn enter_loop(
        &mut self,
        region: usize,
        condition: Option<Expr<'a>>,
    ) -> (TypeEnvironment, TypeEnvironment) {
        self.enter_region(region);
        let entry = self.state.environment.clone();
        let Some(condition) = condition else {
            return (entry.clone(), entry);
        };
        self.visit_expr(condition);
        let tested = self.state.environment.clone();
        self.assume_condition(condition, false);
        let exit = std::mem::replace(&mut self.state.environment, tested);
        self.assume_condition(condition, true);
        (entry, exit)
    }

    fn block_exits(&self, mut statements: AstChildren<'a, Stmt<'a>>) -> bool {
        statements.any(|statement| self.statement_exits(statement))
    }

    fn with_expected(&mut self, expected: ResolvedType, f: impl FnOnce(&mut Self)) {
        self.with_context(Some(expected), f);
    }

    fn with_context(&mut self, expected: Option<ResolvedType>, f: impl FnOnce(&mut Self)) {
        self.state.expected.push(expected);
        f(self);
        self.state.expected.pop();
    }

    fn with_cleared_expected(&mut self, f: impl FnOnce(&mut Self)) {
        let outer = std::mem::take(&mut self.state.expected);
        f(self);
        self.state.expected = outer;
    }

    /// The context type of a list or set literal's elements: its type
    /// argument, else the element type of an iterable context. `None` when the
    /// literal has no context, `Some(Unknown)` when it is unresolved.
    fn element_context(&self, type_arg: Option<DartType<'a>>) -> Option<ResolvedType> {
        if let Some(type_arg) = type_arg {
            return Some(
                self.state
                    .model
                    .resolve_type_in(type_arg, &self.state.type_parameters),
            );
        }
        let expected = self.state.expected()?;
        if matches!(
            expected,
            ResolvedType {
                kind: ResolvedTypeKind::Unknown
            }
        ) {
            return Some(ResolvedType::UNKNOWN);
        }
        self.state
            .signatures
            .instantiated_supertype(expected, "dart:core", "Iterable", &self.state.model)
            .and_then(|iterable| iterable.type_argument(0))
    }

    fn visit_body(&mut self, body: FunctionBody<'a>) {
        match body {
            FunctionBody::Arrow(body) => {
                let expression = body.expr();
                let expected = self
                    .state
                    .return_type
                    .last()
                    .cloned()
                    .unwrap_or(ResolvedType::UNKNOWN);
                self.with_expected(expected, |walker| walker.visit_expr(expression));
            }
            _ => walk_function_body(self, body),
        }
    }

    /// Declaration bodies use their lexical owner's completed signature.
    fn visit_member_callable(&mut self, name: Option<&str>, setter: bool, callable: Callable<'a>) {
        let contextual = name.and_then(|name| {
            let receiver = self.state.enclosing_type.as_ref()?;
            let ResolvedType {
                kind: ResolvedTypeKind::Interface { identity, .. },
            } = receiver
            else {
                return None;
            };
            if self.state.constant_owner.as_ref() != Some(identity) {
                return None;
            }
            if setter {
                let (ty, substitutions) = self.state.signatures.resolved_writable_field(
                    receiver,
                    name,
                    &self.state.model,
                )?;
                return Some(ResolvedType {
                    kind: ResolvedTypeKind::Function {
                        shape: Some(crate::resolve::FunctionTypeShape::empty()),
                        optional_positional: 0,
                        alias: crate::resolve::FunctionTypeAlias::default(),
                        return_type: ResolvedType::VOID.into(),
                        positional: Arc::from([self.state.model.substitute(&ty, &substitutions)]),
                        named: Arc::default(),
                        nullable: false,
                    },
                });
            }
            let signature = self.state.signatures.member(identity, name)?;
            Some(signature.function_type())
        });
        if let Some(expected) = contextual {
            self.with_expected(expected, |walker| {
                walker.visit_function_context(callable, true, true);
            });
        } else {
            self.visit_function(callable, false);
        }
    }

    /// Visits a callable; one with a `nested` start keeps the enclosing
    /// promotions of variables its executable never writes.
    fn visit_function(&mut self, callable: Callable<'a>, contextual: bool) {
        self.visit_function_context(callable, contextual, false);
    }

    fn callable_context_substitutions(
        &self,
        contextual_parameters: Option<&AstChildren<'a, TypeParam<'a>>>,
    ) -> Option<FxHashMap<TypeParameterId, ResolvedType>> {
        let Some(contextual_parameters) = contextual_parameters else {
            return Some(FxHashMap::default());
        };
        if contextual_parameters.is_empty() {
            return Some(FxHashMap::default());
        }
        let expected = self.state.expected()?;
        if !matches!(
            expected,
            ResolvedType {
                kind: ResolvedTypeKind::Function { .. }
            }
        ) {
            return None;
        }
        let mut by_owner = HashMap::new();
        collect_type_parameter_ids(expected, &mut by_owner);
        let parameters = by_owner.values().find(|parameters| {
            contextual_parameters
                .clone()
                .enumerate()
                .all(|(ordinal, declared)| {
                    parameters.iter().any(|parameter| {
                        parameter.ordinal == ordinal
                            && parameter.name.as_ref() == declared.name().name()
                    })
                })
        })?;
        Some(
            parameters
                .iter()
                .filter_map(|parameter| {
                    contextual_parameters
                        .clone()
                        .nth(parameter.ordinal)
                        .and_then(|declared| {
                            self.state.type_parameters.lookup(declared.name().name())
                        })
                        .map(|resolved| (parameter.clone(), resolved))
                })
                .collect(),
        )
    }

    fn callable_return_type(
        &self,
        return_type: Option<DartType<'a>>,
        contextual: bool,
        substitutions: Option<&FxHashMap<TypeParameterId, ResolvedType>>,
        async_marker: AsyncMarker,
    ) -> ResolvedType {
        let resolved_return = return_type
            .map(|ty| {
                self.state
                    .model
                    .resolve_type_in(ty, &self.state.type_parameters)
            })
            .or_else(|| {
                contextual.then(|| {
                    self.contextual_closure_type(substitutions, None)
                        .unwrap_or_else(|| self.uncontextual_type())
                })
            })
            .unwrap_or(ResolvedType::DYNAMIC);
        match resolved_return {
            ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        identity,
                        arguments,
                        occurrence,
                        ..
                    },
            } if async_marker == AsyncMarker::Async && identity.is_sdk("dart:async", "Future") => {
                ResolvedType {
                    kind: ResolvedTypeKind::Interface {
                        occurrence,
                        identity: crate::DeclarationIdentity::sdk("dart:async", "FutureOr"),
                        arguments,
                        nullable: false,
                        extension_type: false,
                    },
                }
            }
            resolved_return => resolved_return,
        }
    }

    fn bind_callable_params(
        &mut self,
        params: Option<&CallableParams<'a>>,
        contextual: bool,
        substitutions: Option<&FxHashMap<TypeParameterId, ResolvedType>>,
    ) {
        match params {
            Some(CallableParams::List(params)) => {
                self.bind_params(*params, None, contextual, substitutions);
                self.visit_param_constants(*params);
            }
            Some(CallableParams::Setter(setter)) => {
                let mut ty = setter.param_type().map_or(ResolvedType::DYNAMIC, |ty| {
                    self.state
                        .model
                        .resolve_type_in(ty, &self.state.type_parameters)
                });
                if setter.param_type().is_none() && contextual {
                    ty = self
                        .contextual_closure_type(
                            substitutions,
                            Some((setter.param().name(), ParamKind::Positional, 0)),
                        )
                        .unwrap_or_else(|| self.uncontextual_type());
                }
                self.state.environment.declare_at(
                    setter.param().name().to_owned(),
                    ty,
                    setter.param().span(),
                );
            }
            None => {}
        }
    }

    fn visit_function_context(
        &mut self,
        callable: Callable<'a>,
        contextual: bool,
        declaration_context: bool,
    ) {
        let Callable {
            nested,
            type_parameters,
            params,
            return_type,
            body,
            async_marker,
        } = callable;
        // Indexed declaration types already use this declaration's source IDs.
        let contextual_parameters = if declaration_context {
            None
        } else {
            Some(type_parameters.clone())
        };
        // A nested callable may run after any write of its executable.
        let mut saved_environment = self.state.environment.clone();
        if let Some(names) = nested.and_then(|start| self.state.writes.anywhere_writes.get(&start))
        {
            for name in names {
                self.state.environment.forget(name);
            }
        }
        // A write-captured binding cannot be promoted inside this closure,
        // even when the writing closure is declared later.
        if let Some(names) =
            nested.and_then(|start| self.state.writes.anywhere_captured.get(&start))
        {
            for name in names {
                self.state.environment.mark_captured(name);
            }
        }
        self.state.environment.enter_callable();
        let saved_breaks = std::mem::take(&mut self.break_targets);
        self.state
            .type_parameters
            .push(type_parameters.clone(), &self.state.model);
        self.with_cleared_expected(|walker| walk_type_params(walker, type_parameters));
        let saved_expected = (!contextual).then(|| std::mem::take(&mut self.state.expected));
        let saved_returns = (!contextual).then(|| std::mem::take(&mut self.state.return_type));
        // Match the context's generic owner once for both parameters and return.
        let contextual_substitutions = contextual
            .then(|| self.callable_context_substitutions(contextual_parameters.as_ref()))
            .flatten();
        let resolved_return = self.callable_return_type(
            return_type,
            contextual,
            contextual_substitutions.as_ref(),
            async_marker,
        );
        self.state.return_type.push(resolved_return);
        self.with_scope(|walker| {
            walker.bind_callable_params(
                params.as_ref(),
                contextual,
                contextual_substitutions.as_ref(),
            );
            if let Some(body) = body {
                // The closure's own context only shapes its parameters and
                // return type; its body statements have no context.
                walker.with_cleared_expected(|walker| walker.visit_body(body));
            }
        });
        // Only actual writes in this callable escape to its enclosing flow.
        // Entry-wide capture facts apply inside the callable, not outside it.
        if let Some(names) =
            nested.and_then(|start| self.state.writes.callable_captured_writes.get(&start))
        {
            for name in names {
                saved_environment.mark_captured(name);
            }
        }
        self.state.environment = saved_environment;
        self.break_targets = saved_breaks;
        self.state.return_type.pop();
        if let Some(saved) = saved_returns {
            self.state.return_type = saved;
        }
        if let Some(saved) = saved_expected {
            self.state.expected = saved;
        }
        self.state.type_parameters.pop();
    }

    /// Type of a closure component the context does not supply.
    fn uncontextual_type(&self) -> ResolvedType {
        self.state
            .signatures
            .uncontextual_closure_type(self.state.expected())
    }

    fn contextual_closure_type(
        &self,
        substitutions: Option<&FxHashMap<TypeParameterId, ResolvedType>>,
        parameter: Option<(&str, ParamKind, usize)>,
    ) -> Option<ResolvedType> {
        let substitutions = substitutions?;
        let ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    return_type,
                    positional,
                    named,
                    ..
                },
        } = self.state.expected()?
        else {
            return None;
        };
        let contextual_type = match parameter {
            None => return_type.as_ref(),
            Some((parameter, kind, ordinal)) => match kind {
                ParamKind::Named { .. } => named
                    .iter()
                    .find(|(name, _)| name == parameter)
                    .map(|(_, ty)| ty)?,
                _ => positional.get(ordinal)?,
            },
        };
        let contextual_type = self.state.model.substitute(contextual_type, substitutions);
        // Uninferred call parameters must not become closure-local types just
        // because an unrelated in-scope parameter has the same name.
        let mut by_owner = HashMap::new();
        collect_type_parameter_ids(&contextual_type, &mut by_owner);
        by_owner
            .values()
            .flatten()
            .all(|id| {
                matches!(self.state.type_parameters.lookup(&id.name),
                    Some(ResolvedType { kind: ResolvedTypeKind::TypeParameter { id: scoped, .. } }) if scoped == *id)
            })
            .then_some(contextual_type)
    }

    fn bind_params(
        &mut self,
        params: FormalParamList<'a>,
        super_constructor: Option<&str>,
        contextual: bool,
        contextual_substitutions: Option<&FxHashMap<TypeParameterId, ResolvedType>>,
    ) {
        let mut positional_ordinal = 0;
        for param in params.params() {
            let mut ty = self
                .state
                .model
                .resolve_formal_parameter_in(param, &self.state.type_parameters);
            if param.param_type().is_none() && param.function_params().is_none() {
                if param.initializing() == Some(InitializingFormal::Super) {
                    ty = super_constructor
                        .and_then(|constructor| {
                            self.state.superclass.as_ref().and_then(|superclass| {
                                resolved_super_param_type(
                                    param,
                                    params,
                                    superclass,
                                    constructor,
                                    &self.state.signatures,
                                    &self.state.model,
                                )
                            })
                        })
                        .unwrap_or(ResolvedType::UNKNOWN);
                } else if param.initializing() == Some(InitializingFormal::This) {
                    ty = self.state.environment.lookup(param.name().name());
                    if matches!(
                        ty,
                        ResolvedType {
                            kind: ResolvedTypeKind::Unknown
                        }
                    ) {
                        ty = self
                            .state
                            .enclosing_type
                            .as_ref()
                            .and_then(|enclosing| {
                                self.state.signatures.resolved_field(
                                    enclosing,
                                    param.name().name(),
                                    &self.state.model,
                                )
                            })
                            .map_or(ResolvedType::UNKNOWN, |(ty, substitutions)| {
                                self.state.model.substitute(&ty, &substitutions)
                            });
                    }
                }
                if contextual {
                    ty = self
                        .contextual_closure_type(
                            contextual_substitutions,
                            Some((param.name().name(), param.kind(), positional_ordinal)),
                        )
                        .unwrap_or_else(|| self.uncontextual_type());
                }
            }
            self.state.environment.declare_at(
                param.name().name().to_owned(),
                ty,
                param.name().span(),
            );
            if !matches!(param.kind(), ParamKind::Named { .. }) {
                positional_ordinal += 1;
            }
        }
    }

    fn visit_param_constants(&mut self, params: FormalParamList<'a>) {
        for param in params.params() {
            self.with_cleared_expected(|walker| walk_formal_param_annotations(walker, param));
            if let Some(value) = param.default_value() {
                let expected = self.state.environment.lookup(param.name().name());
                self.with_expected(expected, |walker| walker.visit_expr(value));
            }
        }
    }

    fn declare_fields(&mut self, members: EffectiveMembers<'a>) {
        for param in members.declaring_params() {
            let ty = self
                .state
                .model
                .resolve_formal_parameter_in(param, &self.state.type_parameters);
            self.declare_field(param.name().name(), ty);
        }
        if let Some(primary) = members.primary {
            let super_constructor = members
                .declared
                .clone()
                .find_map(|member| {
                    let ClassMember::PrimaryConstructorBody(body) = member else {
                        return None;
                    };
                    body.initializers()
                        .find_map(|initializer| match initializer {
                            ConstructorInitializer::SuperCall(call) => {
                                Some(call.call_name().map_or("new", |name| name.name()))
                            }
                            _ => None,
                        })
                })
                .unwrap_or("new");
            self.with_scope(|walker| {
                walker.bind_params(primary.params(), Some(super_constructor), false, None);
                walker.state.environment.declare_receiver(
                    walker
                        .state
                        .enclosing_type
                        .clone()
                        .unwrap_or(ResolvedType::UNKNOWN),
                );
                walker.primary_environment = Some(walker.state.environment.clone());
            });
        }
        for member in members.declared {
            let ClassMember::Field(field) = member else {
                continue;
            };
            for declarator in field.declarators() {
                let ty = field
                    .field_type()
                    .map(|ty| {
                        self.state
                            .model
                            .resolve_type_in(ty, &self.state.type_parameters)
                    })
                    .or_else(|| {
                        declarator.initializer().map(|initializer| {
                            let mut inferred = ResolvedType::UNKNOWN;
                            if !field.is_static() && !field.is_late() {
                                self.with_primary_environment(|walker| {
                                    inferred = walker.state.infer(initializer);
                                });
                            } else {
                                inferred = self.state.infer(initializer);
                            }
                            inferred
                        })
                    })
                    .unwrap_or(ResolvedType::UNKNOWN);
                self.declare_field(declarator.name().name(), ty);
            }
        }
    }

    fn declare_field(&mut self, name: &str, ty: ResolvedType) {
        if self.state.private_field_promotion
            && self.state.enclosing_type.as_ref().is_some_and(|receiver| {
                self.state
                    .signatures
                    .is_promotable_field(receiver, name, &self.state.model)
            })
        {
            self.state.environment.declare(name, ty);
        } else {
            self.state.environment.declare_unpromotable(name, ty);
        }
    }
    fn visit_local_declaration(&mut self, declaration: LocalVarDecl<'a>) {
        self.visit_declaration_annotations(declaration.annotations());
        for declarator in declaration.declarators() {
            let declared = declaration.var_type().map(|ty| {
                self.state
                    .model
                    .resolve_type_in(ty, &self.state.type_parameters)
            });
            if let Some(initializer) = declarator.initializer() {
                self.with_context(declared.clone(), |walker| walker.visit_expr(initializer));
            }
            let initialized = declarator
                .initializer()
                .map(|initializer| self.state.infer(initializer));
            // Mutable and constant explicitly typed locals promote to an
            // initializer's type of interest, such as `NonNull(declared)`.
            let (ty, written) = match declared {
                Some(declared) => (declared, initialized),
                None => (initialized.unwrap_or(ResolvedType::UNKNOWN), None),
            };
            self.state.environment.declare_at(
                declarator.name().name().to_owned(),
                ty,
                declarator.name().span(),
            );
            if matches!(declaration.keyword(), VarKeyword::Var | VarKeyword::Const)
                && let Some(written) = &written
            {
                FlowContext {
                    model: &self.state.model,
                    signatures: &self.state.signatures,
                    type_parameters: &self.state.type_parameters,
                    field_promotions: self.state.private_field_promotion,
                }
                .assign_variable(
                    &mut self.state.environment,
                    declarator.name().name(),
                    written,
                );
            }
            if declaration.keyword() == VarKeyword::Const
                && let Some(initializer) = declarator.initializer()
            {
                let expected = declaration.var_type().map(|_| {
                    self.state
                        .environment
                        .declared_type(declarator.name().name())
                });
                let value = crate::constant_value::evaluate_constant(
                    initializer,
                    crate::constant_value::ConstantContext {
                        owner: self.state.constant_owner.as_ref(),
                        locals: Some(&self.state.visible_local_constants()),
                        expected: expected.as_ref(),
                        ..crate::constant_value::ConstantContext::new(
                            &self.state.model,
                            &self.state.signatures,
                        )
                    },
                );
                self.state.local_constants.push(LocalConstant {
                    depth: self.state.environment.depth() - 1,
                    declaration: declarator.name().span(),
                    name: declarator.name().name().to_owned(),
                    initializer,
                    value,
                });
            }
        }
    }

    fn iterable_element_type(&self, iterable: Expr<'a>, is_await: bool) -> ResolvedType {
        let iterable = self.state.infer(iterable);
        FlowContext {
            model: &self.state.model,
            signatures: &self.state.signatures,
            type_parameters: &self.state.type_parameters,
            field_promotions: self.state.private_field_promotion,
        }
        .iterable_element(&iterable, is_await)
    }

    /// Assignment patterns use the current promoted write targets as their
    /// downward schema; declarations have no existing targets.
    fn assignment_pattern_context(&self, pattern: Pattern<'a>) -> ResolvedType {
        match pattern {
            Pattern::Variable(node) => self.state.environment.lookup(node.name().name()),
            Pattern::ParenPattern(node) => self.assignment_pattern_context(node.inner()),
            Pattern::Record(record) => {
                let named_count = record
                    .fields()
                    .filter(|field| field.name().is_some())
                    .count();
                let mut positional = Vec::with_capacity(record.fields().len() - named_count);
                let mut named = Vec::with_capacity(named_count);
                for field in record.fields() {
                    let ty = self.assignment_pattern_context(field.pattern());
                    if let Some(name) = field.name() {
                        named.push((name.name().to_owned(), ty));
                    } else {
                        positional.push(ty);
                    }
                }
                named.sort_by(|left, right| left.0.cmp(&right.0));
                ResolvedType {
                    kind: ResolvedTypeKind::Record {
                        occurrence: crate::resolve::FunctionTypeAlias::default(),
                        positional: positional.into(),
                        named: named.into(),
                        nullable: false,
                    },
                }
            }
            Pattern::List(list) => {
                let mut element = ResolvedType::UNKNOWN;
                for child in list.elements() {
                    let ty = match child {
                        ListPatternElement::Pattern(child) => {
                            self.assignment_pattern_context(child)
                        }
                        ListPatternElement::Rest(rest) => {
                            let Some(child) = rest.pattern() else {
                                continue;
                            };
                            self.assignment_pattern_context(child)
                                .type_argument(0)
                                .unwrap_or(ResolvedType::UNKNOWN)
                        }
                    };
                    if matches!(
                        element,
                        ResolvedType {
                            kind: ResolvedTypeKind::Unknown
                        }
                    ) || self.state.signatures.relation(
                        &ty,
                        crate::TypeRelation::SubtypeOf(&element),
                        &self.state.model,
                    ) == crate::Truth::Yes
                    {
                        element = ty;
                    } else if !matches!(
                        ty,
                        ResolvedType {
                            kind: ResolvedTypeKind::Unknown
                        }
                    ) && self.state.signatures.relation(
                        &element,
                        crate::TypeRelation::SubtypeOf(&ty),
                        &self.state.model,
                    ) != crate::Truth::Yes
                    {
                        element = ResolvedType::NEVER;
                    }
                }
                let mut list = self.state.model.core_type("List", false);
                if let ResolvedType {
                    kind: ResolvedTypeKind::Interface { arguments, .. },
                } = &mut list
                {
                    *arguments = Arc::from([element]);
                }
                list
            }
            _ => ResolvedType::UNKNOWN,
        }
    }

    fn visit_matched_variable_pattern(&mut self, node: VariablePattern<'a>, matched: ResolvedType) {
        let name = node.name();
        let ty = node.type_().map_or(matched, |ty| {
            self.state
                .model
                .resolve_type_in(ty, &self.state.type_parameters)
        });
        if self.assigning_pattern {
            FlowContext {
                model: &self.state.model,
                signatures: &self.state.signatures,
                type_parameters: &self.state.type_parameters,
                field_promotions: self.state.private_field_promotion,
            }
            .assign_variable(&mut self.state.environment, name.name(), &ty);
        } else {
            self.state
                .environment
                .declare_at(name.name().to_owned(), ty, name.span());
        }
    }

    fn visit_matched_list_pattern(&mut self, list: ListPattern<'a>, matched: &ResolvedType) {
        let element = list
            .type_arg()
            .map(|ty| {
                self.state
                    .model
                    .resolve_type_in(ty, &self.state.type_parameters)
            })
            .or_else(|| {
                self.state
                    .signatures
                    .instantiated_supertype(matched, "dart:core", "List", &self.state.model)
                    .and_then(|ty| ty.type_argument(0))
            })
            .unwrap_or(ResolvedType::UNKNOWN);
        for child in list.elements() {
            match child {
                ListPatternElement::Pattern(child) => {
                    self.visit_matched_pattern(child, element.clone());
                }
                ListPatternElement::Rest(rest) => {
                    if let Some(child) = rest.pattern() {
                        self.visit_matched_pattern(child, matched.clone());
                    }
                }
            }
        }
    }

    fn visit_matched_object_pattern(&mut self, object: ObjectPattern<'a>, matched: &ResolvedType) {
        let object_type = FlowContext {
            model: &self.state.model,
            signatures: &self.state.signatures,
            type_parameters: &self.state.type_parameters,
            field_promotions: self.state.private_field_promotion,
        }
        .object_pattern_type(object.type_(), matched);
        for field in object.fields() {
            let field_type = self
                .state
                .signatures
                .resolved_field(&object_type, field.name().name(), &self.state.model)
                .or_else(|| {
                    self.state.signatures.resolved_extension_getter(
                        &object_type,
                        field.name().name(),
                        &self.state.model,
                    )
                })
                .map_or(ResolvedType::UNKNOWN, |(ty, substitutions)| {
                    self.state.model.substitute(&ty, &substitutions)
                });
            if let Some(child) = field.pattern() {
                self.visit_matched_pattern(child, field_type);
            } else if self.assigning_pattern {
                FlowContext {
                    model: &self.state.model,
                    signatures: &self.state.signatures,
                    type_parameters: &self.state.type_parameters,
                    field_promotions: self.state.private_field_promotion,
                }
                .assign_variable(
                    &mut self.state.environment,
                    field.name().name(),
                    &field_type,
                );
            } else {
                self.state.environment.declare_at(
                    field.name().name().to_owned(),
                    field_type,
                    field.name().span(),
                );
            }
        }
    }

    fn visit_matched_pattern(&mut self, pattern: Pattern<'a>, matched: ResolvedType) {
        self.state.matched.push(matched.clone());
        self.visitor
            .visit_pattern(pattern, Some(&matched), &self.state);
        match pattern {
            Pattern::Variable(node) => self.visit_matched_variable_pattern(node, matched),
            Pattern::Wildcard(_) | Pattern::Literal(_) | Pattern::Error(_) => {}
            Pattern::Const(constant) => {
                if let ConstPatternValue::Expr(expression) = constant.value() {
                    self.visit_expr(expression);
                }
            }
            Pattern::List(list) => self.visit_matched_list_pattern(list, &matched),
            Pattern::Record(record) => {
                let mut positional_index = 0;
                for field in record.fields() {
                    let field_type = field
                        .name()
                        .and_then(|name| matched.record_field(name.name()))
                        .unwrap_or_else(|| {
                            let ty = matched
                                .record_positional(positional_index)
                                .unwrap_or(ResolvedType::UNKNOWN);
                            positional_index += 1;
                            ty
                        });
                    self.visit_matched_pattern(field.pattern(), field_type);
                }
            }
            Pattern::Map(map) => {
                let value_type = self
                    .state
                    .signatures
                    .instantiated_supertype(&matched, "dart:core", "Map", &self.state.model)
                    .and_then(|ty| ty.type_argument(1))
                    .unwrap_or(ResolvedType::UNKNOWN);
                for entry in map.entries() {
                    self.visit_expr(entry.key());
                    self.visit_matched_pattern(entry.pattern(), value_type.clone());
                }
            }
            Pattern::Object(object) => self.visit_matched_object_pattern(object, &matched),
            Pattern::LogicalAnd(node) => {
                self.visit_matched_pattern(node.left(), matched.clone());
                self.visit_matched_pattern(node.right(), matched.clone());
            }
            Pattern::LogicalOr(node) => {
                self.visit_matched_pattern(node.left(), matched.clone());
                self.visit_matched_pattern(node.right(), matched.clone());
            }
            Pattern::Relational(node) => self.visit_expr(node.value()),
            Pattern::Cast(node) => {
                let cast = self
                    .state
                    .model
                    .resolve_type_in(node.cast_type(), &self.state.type_parameters);
                self.visit_matched_pattern(node.inner(), cast);
            }
            Pattern::NullCheck(node) => {
                self.visit_matched_pattern(node.inner(), matched.non_null());
            }
            Pattern::NullAssert(node) => {
                self.visit_matched_pattern(node.inner(), matched.non_null());
            }
            Pattern::ParenPattern(node) => {
                self.visit_matched_pattern(node.inner(), matched);
            }
        }
        self.state.matched.pop();
    }

    fn visit_for_init(&mut self, init: ForInit<'a>, is_await: bool) {
        match init {
            ForInit::VarDecl(declaration) => self.visit_local_declaration(declaration),
            ForInit::ForIn(node) if node.declares_variable() => {
                self.visit_declaration_annotations(node.annotations());
                self.visit_expr(node.iterable());
                let ty = node.var_type().map_or_else(
                    || self.iterable_element_type(node.iterable(), is_await),
                    |ty| {
                        self.state
                            .model
                            .resolve_type_in(ty, &self.state.type_parameters)
                    },
                );
                self.state.environment.declare_at(
                    node.name().name().to_owned(),
                    ty,
                    node.name().span(),
                );
            }
            // `for (x in xs)` writes an existing `x` at each iteration's start.
            ForInit::ForIn(node) => self.visit_expr(node.iterable()),
            ForInit::PatternForIn(node) => {
                self.visit_declaration_annotations(node.annotations());
                self.visit_expr(node.iterable());
                let matched = self.iterable_element_type(node.iterable(), is_await);
                self.visit_matched_pattern(node.pattern(), matched);
            }
            ForInit::Exprs(expressions) => {
                for expression in expressions.exprs() {
                    self.visit_expr(expression);
                }
            }
        }
    }

    /// Assigns a bare `for (x in xs)` loop's element to `x` as an iteration starts.
    fn assign_for_in(&mut self, init: Option<ForInit<'a>>, is_await: bool) {
        if let Some(ForInit::ForIn(node)) = init
            && !node.declares_variable()
        {
            let element = self.iterable_element_type(node.iterable(), is_await);
            FlowContext {
                model: &self.state.model,
                signatures: &self.state.signatures,
                type_parameters: &self.state.type_parameters,
                field_promotions: self.state.private_field_promotion,
            }
            .assign_variable(&mut self.state.environment, node.name().name(), &element);
        }
    }

    /// Enters a collection `for` element's loop; returns its exit state.
    fn visit_for_header(
        &mut self,
        header: ForHeader<'a>,
        region: usize,
        is_await: bool,
    ) -> TypeEnvironment {
        if let Some(init) = header.init() {
            self.visit_for_init(init, is_await);
        }
        let exit = self.enter_loop(region, header.condition()).1;
        self.assign_for_in(header.init(), is_await);
        exit
    }

    fn visit_if_condition(
        &mut self,
        condition: IfCondition<'a>,
        then_branch: impl FnOnce(&mut Self),
        else_branch: impl FnOnce(&mut Self),
        then_exits: bool,
        else_exits: bool,
    ) {
        match condition {
            IfCondition::Expr(expression) => {
                self.visit_expr(expression);
                self.visit_branches(expression, then_branch, else_branch, then_exits, else_exits);
            }
            IfCondition::Case(node) => {
                let value = node.expr();
                let pattern = node.pattern();
                let guard = node.guard();
                self.visit_expr(value);
                let matched = self.state.infer(value);
                let mut unmatched = None;
                self.with_scope(|walker| {
                    unmatched =
                        Some(walker.visit_pattern_condition(value, pattern, guard, matched));
                    then_branch(walker);
                });
                let mut unmatched = unmatched.expect("pattern failure");
                unmatched.pop_scope();
                let then_environment = std::mem::replace(&mut self.state.environment, unmatched);
                self.with_scope(else_branch);
                FlowContext::join_branches(
                    &mut self.state.environment,
                    then_environment,
                    then_exits,
                    else_exits,
                );
            }
        }
    }

    fn visit_element_with_context<E: Element<'a>>(
        &mut self,
        element: E,
        key_type: Option<&ResolvedType>,
        value_type: Option<&ResolvedType>,
    ) {
        match element.kind() {
            ElementKind::Expression(expression) => {
                self.with_context(key_type.cloned(), |walker| walker.visit_expr(expression));
            }
            ElementKind::NullAware(node) => {
                self.with_context(key_type.cloned(), |walker| walker.visit_expr(node.expr()));
            }
            ElementKind::Entry(entry) => {
                self.with_context(key_type.cloned(), |walker| walker.visit_expr(entry.key()));
                self.with_context(value_type.cloned(), |walker| {
                    walker.visit_expr(entry.value());
                });
            }
            ElementKind::Spread { expression, .. } => {
                if E::IS_MAP {
                    self.with_expected(ResolvedType::UNKNOWN, |walker| {
                        walker.visit_expr(expression);
                    });
                } else {
                    let expected = key_type.map(|element| ResolvedType {
                        kind: ResolvedTypeKind::Interface {
                            occurrence: crate::resolve::FunctionTypeAlias::default(),
                            identity: DeclarationIdentity::sdk("dart:core", "Iterable"),
                            arguments: Arc::from([element.clone()]),
                            nullable: false,
                            extension_type: false,
                        },
                    });
                    self.with_context(expected, |walker| walker.visit_expr(expression));
                }
            }
            ElementKind::If {
                condition,
                then_element,
                else_element,
            } => self.visit_if_condition(
                condition,
                |walker| walker.visit_element_with_context(then_element, key_type, value_type),
                |walker| {
                    if let Some(element) = else_element {
                        walker.visit_element_with_context(element, key_type, value_type);
                    }
                },
                false,
                false,
            ),
            ElementKind::For { header, body } => self.with_scope(|walker| {
                let exit =
                    walker.visit_for_header(header, element.span().start, element.is_await());
                walker.visit_element_with_context(body, key_type, value_type);
                for update in header.updates() {
                    walker.visit_expr(update);
                }
                walker.state.environment = exit;
            }),
        }
    }

    fn visit_field_initializers(&mut self, node: FieldDecl<'a>) {
        for declarator in node.declarators() {
            if let Some(initializer) = declarator.initializer() {
                // An untyped instance field may inherit its type from an
                // overridden member; only an untyped static field has no context.
                let expected = match node.field_type() {
                    Some(ty) => Some(
                        self.state
                            .model
                            .resolve_type_in(ty, &self.state.type_parameters),
                    ),
                    None if node.is_static() => None,
                    None => Some(ResolvedType::UNKNOWN),
                };
                self.with_context(expected, |walker| walker.visit_expr(initializer));
            }
        }
    }

    fn visit_constructor_initializers(
        &mut self,
        initializers: AstChildren<'a, ConstructorInitializer<'a>>,
    ) {
        for initializer in initializers {
            match initializer {
                ConstructorInitializer::SuperCall(_) | ConstructorInitializer::ThisCall(_) => {
                    let (args, call_name) = match initializer {
                        ConstructorInitializer::SuperCall(node) => (node.args(), node.call_name()),
                        ConstructorInitializer::ThisCall(node) => (node.args(), node.call_name()),
                        _ => unreachable!(),
                    };
                    let receiver = match initializer {
                        ConstructorInitializer::SuperCall(_) => self.state.superclass.as_ref(),
                        _ => self.state.enclosing_type.as_ref(),
                    };
                    let parameters = receiver.and_then(|receiver| {
                        self.state.constructor_parameter_types(
                            receiver,
                            call_name.map_or("new", |name| name.name()),
                        )
                    });
                    match parameters {
                        Some((positional, named)) => {
                            self.visit_call_arguments(args, &positional, &named);
                        }
                        None => self.visit_call_arguments(args, &[], &HashMap::new()),
                    }
                }
                ConstructorInitializer::FieldInit(node) => {
                    let field = node.field();
                    let value = node.value();
                    let expected = self
                        .state
                        .enclosing_type
                        .as_ref()
                        .and_then(|enclosing| {
                            self.state.signatures.resolved_field(
                                enclosing,
                                field.name(),
                                &self.state.model,
                            )
                        })
                        .map_or(ResolvedType::UNKNOWN, |(ty, substitutions)| {
                            self.state.model.substitute(&ty, &substitutions)
                        });
                    self.with_expected(expected, |walker| walker.visit_expr(value));
                }
                ConstructorInitializer::Assert(node) => {
                    self.visit_expr(node.condition());
                    if let Some(message) = node.message() {
                        self.visit_expr(message);
                    }
                }
            }
        }
    }

    /// Visits the arguments of a constructor invoked outside an expression
    /// (an enum constant or an annotation) against its instantiated parameters.
    fn visit_constructor_arguments(
        &mut self,
        identity: Option<&DeclarationIdentity>,
        name: &str,
        type_args: &AstChildren<'a, DartType<'a>>,
        args: ArgList<'a>,
    ) {
        let signature =
            identity.and_then(|identity| self.state.signatures.constructor(identity, name));
        let Some(signature) = signature else {
            return self.visit_call_arguments(args, &[], &HashMap::new());
        };
        let substitutions = self.state.signature_substitutions_with_result(
            (
                signature,
                &signature.owner_parameters,
                &signature.return_type,
                &FxHashMap::default(),
            ),
            type_args,
            args,
            None,
            None,
        );
        let (positional, named) = self.state.substituted_parameters(signature, &substitutions);
        self.visit_call_arguments(args, &positional, &named);
    }

    /// Annotations on declarations the walker visits through custom callable
    /// or initializer walks; their arguments are constant constructor arguments.
    fn visit_declaration_annotations(&mut self, annotations: AstChildren<'a, Annotation<'a>>) {
        for annotation in annotations {
            self.with_expected(ResolvedType::UNKNOWN, |walker| {
                walker.visit_annotation(annotation);
            });
        }
    }

    fn visit_call_arguments(
        &mut self,
        args: ArgList<'a>,
        positional: &[ResolvedType],
        named: &HashMap<String, ResolvedType>,
    ) {
        self.visitor
            .visit_call_arguments(args, positional, named, &self.state);
        // Arguments without a resolved parameter have an unknown context; they
        // must not inherit the context of the enclosing expression.
        for (index, argument) in args.positional().enumerate() {
            let expected = positional
                .get(index)
                .cloned()
                .unwrap_or(ResolvedType::UNKNOWN);
            self.with_expected(expected, |walker| walker.visit_expr(argument));
        }
        for argument in args.named() {
            let expected = named
                .get(argument.name().name())
                .cloned()
                .unwrap_or(ResolvedType::UNKNOWN);
            self.with_expected(expected, |walker| walker.visit_expr(argument.value()));
        }
    }
}

impl<'a, V: SemanticRuleVisitor<'a> + ?Sized> ScopedWalker<'_, 'a, V> {
    fn visit_if_statement(&mut self, statement: IfStmt<'a>) {
        if let IfCondition::Expr(condition) = statement.condition() {
            self.visit_expr(condition);
            self.visit_branches(
                condition,
                |walker| walker.visit_stmt(statement.then_branch()),
                |walker| {
                    if let Some(branch) = statement.else_branch() {
                        walker.visit_stmt(branch);
                    }
                },
                self.statement_exits(statement.then_branch()),
                statement
                    .else_branch()
                    .is_some_and(|branch| self.statement_exits(branch)),
            );
        } else {
            self.visit_if_condition(
                statement.condition(),
                |walker| walker.visit_stmt(statement.then_branch()),
                |walker| {
                    if let Some(branch) = statement.else_branch() {
                        walker.visit_stmt(branch);
                    }
                },
                self.statement_exits(statement.then_branch()),
                statement
                    .else_branch()
                    .is_some_and(|branch| self.statement_exits(branch)),
            );
        }
    }

    fn visit_for_statement(&mut self, statement: ForStmt<'a>) {
        self.with_scope(|walker| {
            if let Some(init) = statement.header().init() {
                walker.visit_for_init(init, statement.is_await());
            }
            let body = statement.body();
            walker.with_break_target(None, true, Stmt::For(statement), |walker| {
                let (_, exit) =
                    walker.enter_loop(statement.span().start, statement.header().condition());
                walker.assign_for_in(statement.header().init(), statement.is_await());
                walker.visit_stmt(body);
                for update in statement.header().updates() {
                    walker.visit_expr(update);
                }
                walker.state.environment = exit;
            });
        });
    }

    fn visit_switch_statement(&mut self, statement: SwitchStmt<'a>) {
        self.visit_expr(statement.subject());
        let matched = self.state.infer(statement.subject());
        // A `continue label;` may re-enter any labeled case.
        if statement.cases().any(|case| case.labels().next().is_some()) {
            self.enter_region(statement.span().start);
        }
        let exhaustive = statement.cases().any(|case| {
            case.cases()
                .any(|kind| matches!(kind, SwitchCaseKind::Default(_)))
        }) || FlowContext {
            model: &self.state.model,
            signatures: &self.state.signatures,
            type_parameters: &self.state.type_parameters,
            field_promotions: self.state.private_field_promotion,
        }
        .switch_is_exhaustive(statement, &matched);
        let before = self.state.environment.clone();
        let depth = before.depth();
        let mut ends = Vec::with_capacity(statement.cases().len() + 1);
        self.break_targets.push(BreakTarget {
            label: None,
            unlabeled: true,
            states: Vec::new(),
        });
        // An empty case's matches reach the next case's body.
        let mut matched_empty: Option<TypeEnvironment> = None;
        let mut unmatched = before.clone();
        for (index, case) in statement.cases().enumerate() {
            self.state.environment.clone_from(&unmatched);
            let falls_through = case.body().is_empty() && index + 1 < statement.cases().len();
            self.with_scope(|walker| {
                // Later heads start from the preceding head's failure.
                unmatched.clone_from(&walker.state.environment);
                let mut matched_paths = matched_empty.take();
                for kind in case.cases() {
                    walker.state.environment.clone_from(&unmatched);
                    match kind {
                        SwitchCaseKind::Pattern(label) => {
                            unmatched = walker.visit_pattern_condition(
                                statement.subject(),
                                label.pattern(),
                                label.guard(),
                                matched.clone(),
                            );
                        }
                        SwitchCaseKind::Expr(label) => walker.visit_expr(label.expr()),
                        SwitchCaseKind::Default(_) => {}
                    }
                    matched_paths = Some(match matched_paths {
                        Some(previous) => walker.state.environment.join(&previous),
                        None => walker.state.environment.clone(),
                    });
                }
                if let Some(matched_paths) = matched_paths {
                    walker.state.environment = matched_paths;
                }
                if falls_through {
                    matched_empty = Some(walker.state.environment.clone());
                    return;
                }
                for statement in case.body() {
                    walker.visit_stmt(statement);
                }
            });
            unmatched.pop_scope();
            if !falls_through && !self.block_exits(case.body()) {
                ends.push(std::mem::take(&mut self.state.environment));
            }
        }
        ends.extend(self.break_targets.pop().expect("switch target").states);
        // No case matching reaches the end unless the switch is
        // exhaustive; with no completing path the end is unreachable.
        if !exhaustive {
            ends.push(unmatched);
        }
        FlowContext::join_paths(
            &mut self.state.environment,
            ends.into_iter().map(|end| (Cow::Owned(end), true)),
            depth,
            false,
        );
    }

    fn visit_try_statement(&mut self, statement: TryCatchStmt<'a>) {
        let before = self.state.environment.clone();
        self.with_scope(|walker| walker.visit_block(statement.body()));
        let mut ends = Vec::with_capacity(statement.catches().len() + 1);
        let completes = !self.block_exits(statement.body().stmts());
        ends.push((
            Cow::Owned(std::mem::take(&mut self.state.environment)),
            completes,
        ));
        // `finally` may also start anywhere in the `try` or its catches.
        let before_finally = statement.finally().map(|_| before.clone());
        // A catch may start anywhere in the `try` body: only promotions
        // that hold before it and survive its writes remain.
        self.state.environment = before;
        self.enter_region(statement.span().start);
        let caught = self.state.environment.clone();
        for catch in statement.catches() {
            self.state.environment = caught.clone();
            self.with_scope(|walker| {
                if let Some(name) = catch.exception_var() {
                    let ty = catch.exception_type().map_or_else(
                        || walker.state.model.core_type("Object", false),
                        |ty| {
                            walker
                                .state
                                .model
                                .resolve_type_in(ty, &walker.state.type_parameters)
                        },
                    );
                    walker
                        .state
                        .environment
                        .declare_at(name.name().to_owned(), ty, name.span());
                }
                if let Some(name) = catch.stack_trace_var() {
                    let ty = walker.state.model.core_type("StackTrace", false);
                    walker
                        .state
                        .environment
                        .declare_at(name.name().to_owned(), ty, name.span());
                }
                walker.visit_block(catch.body());
            });
            let completes = !self.block_exits(catch.body().stmts());
            ends.push((
                Cow::Owned(std::mem::take(&mut self.state.environment)),
                completes,
            ));
        }
        self.state.environment = caught;
        let depth = self.state.environment.depth();
        FlowContext::join_paths(&mut self.state.environment, ends, depth, false);
        if let (Some(finally), Some(before)) = (statement.finally(), before_finally) {
            // `finally` runs after the body and catches complete, or
            // after any of their writes when they leave early.
            let final_states = std::mem::replace(&mut self.state.environment, before);
            self.enter_region(statement.span().start);
            for catch in statement.catches() {
                self.enter_region(catch.body().span().start);
            }
            let entry = final_states.join(&self.state.environment);
            self.state.environment = entry.clone();
            self.with_scope(|walker| walker.visit_block(finally));
            let end = std::mem::replace(&mut self.state.environment, final_states);
            self.state.environment.attach_finally(&entry, &end);
        }
    }

    fn visit_local_function_statement(&mut self, function: LocalFuncDecl<'a>) {
        self.visit_declaration_annotations(function.annotations());
        self.state
            .type_parameters
            .push(function.type_params(), &self.state.model);
        let mut inferred_return = ResolvedType::UNKNOWN;
        if function.return_type().is_none() && function.async_marker() == AsyncMarker::Sync {
            self.with_scope(|walker| {
                walker.bind_params(function.params(), None, false, None);
                if let FunctionBody::Arrow(body) = function.body() {
                    inferred_return = walker.state.infer(body.expr());
                }
            });
        }
        let ty = ResolvedType {
            kind: ResolvedTypeKind::Function {
                shape: Some(crate::resolve::FunctionTypeShape::new(
                    function
                        .params()
                        .named()
                        .filter(|param| matches!(param.kind(), ParamKind::Named { required: true }))
                        .map(|param| param.name().name().to_owned())
                        .collect(),
                    function
                        .type_params()
                        .filter_map(|param| self.state.type_parameters.lookup(param.name().name()))
                        .collect(),
                )),
                optional_positional: function
                    .params()
                    .positional()
                    .filter(|param| param.kind() == ParamKind::OptionalPositional)
                    .count(),
                alias: crate::resolve::FunctionTypeAlias::default(),
                return_type: std::sync::Arc::new(
                    function
                        .return_type()
                        .map(|ty| {
                            self.state
                                .model
                                .resolve_type_in(ty, &self.state.type_parameters)
                        })
                        .unwrap_or(inferred_return),
                ),
                positional: function
                    .params()
                    .positional()
                    .map(|param| {
                        self.state
                            .model
                            .resolve_formal_parameter_in(param, &self.state.type_parameters)
                    })
                    .collect(),
                named: function
                    .params()
                    .named()
                    .map(|param| {
                        (
                            param.name().name().to_owned(),
                            self.state
                                .model
                                .resolve_formal_parameter_in(param, &self.state.type_parameters),
                        )
                    })
                    .collect(),
                nullable: false,
            },
        };
        self.state.environment.declare_local_function_at(
            function.name().name().to_owned(),
            ty,
            function.name().span(),
        );
        self.state.type_parameters.pop();
        self.visit_function(
            Callable {
                nested: Some(function.span().start),
                type_parameters: function.type_params(),
                params: Some(CallableParams::List(function.params())),
                return_type: function.return_type(),
                body: Some(function.body()),
                async_marker: function.async_marker(),
            },
            false,
        );
    }

    fn visit_yield_statement(&mut self, statement: YieldStmt<'a>) {
        // `yield e` in a `Stream<T>`/`Iterable<T>` generator has
        // context `T`.
        let element = self.state.return_type.last().and_then(|ty| {
            (!statement.is_star()
                && (ty.interface("dart:async", "Stream") || ty.interface("dart:core", "Iterable")))
            .then(|| ty.type_argument(0))
            .flatten()
        });
        match element {
            Some(element) => {
                self.with_expected(element, |walker| walker.visit_expr(statement.value()));
            }
            None => self.visit_expr(statement.value()),
        }
    }

    fn enclosing_declaration_type(
        &self,
        node: TopLevelDecl<'a>,
        enclosing_name: Option<&str>,
        type_parameters: AstChildren<'a, TypeParam<'a>>,
    ) -> ResolvedType {
        let walker = self;
        // Instantiate the enclosing type with its own type parameters, so
        // member and constructor lookups have something to substitute.
        let arguments: Vec<ResolvedType> = type_parameters
            .map(|param| {
                walker
                    .state
                    .type_parameters
                    .lookup(param.name().name())
                    .unwrap_or(ResolvedType::UNKNOWN)
            })
            .collect();
        // `this` in an extension is its `on` type.
        match node {
            TopLevelDecl::Extension(extension) => walker
                .state
                .model
                .resolve_type_in(extension.on_type(), &walker.state.type_parameters),
            _ => enclosing_name
                .and_then(|name| walker.state.model.resolve_name(name))
                .map_or(ResolvedType::UNKNOWN, |identity| ResolvedType {
                    kind: ResolvedTypeKind::Interface {
                        occurrence: crate::resolve::FunctionTypeAlias::default(),
                        identity,
                        arguments: arguments.into(),
                        nullable: false,
                        extension_type: false,
                    },
                }),
        }
    }

    fn prepare_declaration_members(
        &mut self,
        effective_members: &EffectiveMembers<'a>,
        members: &AstChildren<'a, ClassMember<'a>>,
    ) {
        let walker = self;
        walker.declare_fields(effective_members.clone());
        walker.state.environment.declare_receiver(
            walker
                .state
                .enclosing_type
                .clone()
                .unwrap_or(ResolvedType::UNKNOWN),
        );
        for constructor in effective_members
            .constructors()
            .filter(|ctor| ctor.is_primary)
        {
            walker.with_primary_environment(|walker| {
                walker
                    .visitor
                    .visit_constructor_ref(constructor, &walker.state);
            });
        }
        if let Some(primary) = effective_members.primary
            && members.clone().any(|member| {
                matches!(
                    member,
                    ClassMember::PrimaryConstructorBody(body) if body.body().is_some()
                )
            })
        {
            walker.with_scope(|walker| {
                for param in primary.params().params() {
                    if effective_members.representation
                        || param.is_var()
                        || param.is_final()
                        || param.initializing().is_some()
                    {
                        continue;
                    }
                    let ty = walker
                        .state
                        .model
                        .resolve_formal_parameter_in(param, &walker.state.type_parameters);
                    walker.state.environment.declare_at(
                        param.name().name().to_owned(),
                        ty,
                        param.name().span(),
                    );
                }
                walker.primary_body_environment = Some(walker.state.environment.clone());
            });
        }
    }

    fn visit_scoped_top_level_declaration(
        &mut self,
        node: TopLevelDecl<'a>,
        enclosing_name: Option<&str>,
        superclass: Option<DartType<'a>>,
        effective_members: Option<EffectiveMembers<'a>>,
        type_parameters: AstChildren<'a, TypeParam<'a>>,
        members: &AstChildren<'a, ClassMember<'a>>,
    ) {
        let walker = self;
        let previous_unnamed_members = std::mem::replace(
            &mut walker.state.unnamed_extension_members,
            match node {
                TopLevelDecl::Extension(extension) if extension.name().is_none() => {
                    Some(extension.members())
                }
                _ => None,
            },
        );
        let previous_constant_owner = std::mem::replace(
            &mut walker.state.constant_owner,
            match node {
                TopLevelDecl::Extension(extension) => extension
                    .name()
                    .and_then(|name| walker.state.model.resolve_extension(name.name())),
                _ => enclosing_name.and_then(|name| walker.state.model.resolve_name(name)),
            },
        );
        walker
            .state
            .type_parameters
            .push(type_parameters.clone(), &walker.state.model);
        let enclosing = walker.enclosing_declaration_type(node, enclosing_name, type_parameters);
        let previous_enclosing = walker.state.enclosing_type.replace(enclosing);
        let previous_superclass = std::mem::replace(
            &mut walker.state.superclass,
            superclass.map(|superclass| {
                walker
                    .state
                    .model
                    .resolve_type_in(superclass, &walker.state.type_parameters)
            }),
        );
        let effective_members =
            effective_members.unwrap_or_else(|| EffectiveMembers::declared(members.clone()));
        let previous_primary = walker.primary_environment.take();
        let previous_primary_body = walker.primary_body_environment.take();
        walker.prepare_declaration_members(&effective_members, members);
        if matches!(node, TopLevelDecl::Enum(_)) {
            walker.with_expected(ResolvedType::UNKNOWN, |walker| {
                walk_top_level_decl(walker, node);
            });
        } else {
            walk_top_level_decl(walker, node);
        }
        walker.primary_environment = previous_primary;
        walker.primary_body_environment = previous_primary_body;
        walker.state.superclass = previous_superclass;
        walker.state.type_parameters.pop();
        walker.state.enclosing_type = previous_enclosing;
        walker.state.constant_owner = previous_constant_owner;
        walker.state.unnamed_extension_members = previous_unnamed_members;
    }
}

impl<'a, V: SemanticRuleVisitor<'a> + ?Sized> Visitor<'a> for ScopedWalker<'_, 'a, V> {
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        let enclosing_name = match node {
            TopLevelDecl::Class(declaration) => Some(declaration.name().name()),
            TopLevelDecl::Mixin(declaration) => Some(declaration.name().name()),
            TopLevelDecl::MixinClass(declaration) => Some(declaration.name().name()),
            TopLevelDecl::Enum(declaration) => Some(declaration.name().name()),
            TopLevelDecl::ExtensionType(declaration) => Some(declaration.name().name()),
            TopLevelDecl::ClassTypeAlias(_)
            | TopLevelDecl::Extension(_)
            | TopLevelDecl::Function(_)
            | TopLevelDecl::Variable(_)
            | TopLevelDecl::TypeAlias(_)
            | TopLevelDecl::Error(_) => None,
        };
        let superclass = match node {
            TopLevelDecl::Class(declaration) => declaration.extends(),
            TopLevelDecl::MixinClass(declaration) => declaration.extends(),
            _ => None,
        };
        let effective_members = match node {
            TopLevelDecl::Class(declaration) => Some(declaration.element_members()),
            TopLevelDecl::MixinClass(declaration) => Some(declaration.element_members()),
            TopLevelDecl::Enum(declaration) => Some(declaration.element_members()),
            TopLevelDecl::ExtensionType(declaration) => Some(declaration.element_members()),
            _ => None,
        };
        let scoped = match node {
            TopLevelDecl::Class(declaration) => {
                Some((declaration.type_params(), declaration.members()))
            }
            TopLevelDecl::Mixin(declaration) => {
                Some((declaration.type_params(), declaration.members()))
            }
            TopLevelDecl::MixinClass(declaration) => {
                Some((declaration.type_params(), declaration.members()))
            }
            TopLevelDecl::Enum(declaration) => {
                Some((declaration.type_params(), declaration.members()))
            }
            TopLevelDecl::Extension(declaration) => {
                Some((declaration.type_params(), declaration.members()))
            }
            TopLevelDecl::ExtensionType(declaration) => {
                Some((declaration.type_params(), declaration.members()))
            }
            _ => None,
        };
        if let Some((type_parameters, members)) = scoped {
            self.with_fresh_environment(|walker| {
                walker.visit_scoped_top_level_declaration(
                    node,
                    enclosing_name,
                    superclass,
                    effective_members,
                    type_parameters,
                    &members,
                );
            });
        } else if matches!(node, TopLevelDecl::Function(_)) {
            self.with_fresh_environment(|walker| walk_top_level_decl(walker, node));
        } else if let TopLevelDecl::Variable(variable) = node {
            self.visit_declaration_annotations(variable.annotations());
            for declarator in variable.declarators() {
                if let Some(initializer) = declarator.initializer() {
                    let expected = variable
                        .var_type()
                        .is_some()
                        .then(|| self.state.globals.get(declarator.name().name()).cloned())
                        .flatten();
                    self.with_context(expected, |walker| walker.visit_expr(initializer));
                }
            }
        } else {
            walk_top_level_decl(self, node);
        }
    }

    /// As `walk_enum_decl`, but each constant's arguments are visited against
    /// the enum constructor it invokes.
    fn visit_enum_decl(&mut self, node: EnumDecl<'a>) {
        for annotation in node.annotations() {
            self.visit_annotation(annotation);
        }
        self.visit_identifier(node.name());
        walk_type_params(self, node.type_params());
        if let Some(constructor) = node.primary_constructor() {
            self.visit_primary_constructor(constructor);
        }
        for ty in node.with_clause().chain(node.implements()) {
            self.visit_dart_type(ty);
        }
        let identity = self.state.model.resolve_name(node.name().name());
        for variant in node.variants() {
            for annotation in variant.annotations() {
                self.visit_annotation(annotation);
            }
            self.visit_identifier(variant.name());
            for ty in variant.type_args() {
                self.visit_dart_type(ty);
            }
            if let Some(args) = variant.args() {
                let name = variant.constructor_name().map_or("new", |name| name.name());
                self.visit_constructor_arguments(
                    identity.as_ref(),
                    name,
                    &variant.type_args(),
                    args,
                );
            }
        }
        for member in node.members() {
            self.visit_class_member(member);
        }
    }

    /// An annotation with arguments invokes a constant constructor.
    fn visit_annotation(&mut self, node: Annotation<'a>) {
        let Some(args) = node.args() else {
            return walk_annotation(self, node);
        };
        for ty in node.type_args() {
            self.visit_dart_type(ty);
        }
        let model = &self.state.model;
        let segments: Vec<&str> = node.name().map(Identifier::name).collect();
        let target = match node.constructor_name() {
            Some(name) => model
                .resolve_dotted(&segments)
                .map(|identity| (identity, name.name())),
            None => model
                .resolve_dotted(&segments)
                .map(|identity| (identity, "new"))
                .or_else(|| {
                    let (name, owner) = segments.split_last()?;
                    Some((model.resolve_dotted(owner)?, *name))
                }),
        };
        let (identity, name) = target.unzip();
        self.visit_constructor_arguments(
            identity.as_ref(),
            name.unwrap_or("new"),
            &node.type_args(),
            args,
        );
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) {
        self.visit_declaration_annotations(node.annotations());
        self.visit_function(
            Callable {
                nested: None,
                type_parameters: node.type_params(),
                params: node.params().map(CallableParams::List),
                return_type: node.return_type(),
                body: node.body(),
                async_marker: node.async_marker(),
            },
            false,
        );
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) {
        self.visit_member_callable(
            (!node.is_static()).then_some(node.name().name()),
            false,
            Callable {
                nested: None,
                type_parameters: node.type_params(),
                params: Some(CallableParams::List(node.params())),
                return_type: node.return_type(),
                body: node.body(),
                async_marker: node.async_marker(),
            },
        );
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) {
        let annotations = match node {
            ClassMember::Field(member) => Some(member.annotations()),
            ClassMember::Constructor(member) => Some(member.annotations()),
            ClassMember::Method(member) => Some(member.annotations()),
            ClassMember::Getter(member) => Some(member.annotations()),
            ClassMember::Setter(member) => Some(member.annotations()),
            ClassMember::Operator(member) => Some(member.annotations()),
            ClassMember::PrimaryConstructorBody(_) | ClassMember::Error(_) => None,
        };
        let annotations = annotations.unwrap_or_else(AstChildren::empty);
        self.visit_declaration_annotations(annotations);
        if let ClassMember::Operator(operator) = node {
            self.visit_member_callable(
                Some(operator.lookup_name()),
                false,
                Callable {
                    nested: None,
                    type_parameters: AstChildren::empty(),
                    params: Some(CallableParams::List(operator.params())),
                    return_type: operator.return_type(),
                    body: operator.body(),
                    async_marker: operator.async_marker(),
                },
            );
        } else {
            falcon_syntax::cst::visit::walk_class_member(self, node);
        }
    }

    fn visit_primary_constructor(&mut self, node: PrimaryConstructor<'a>) {
        self.with_primary_environment(|walker| {
            walker.visit_param_constants(node.params());
        });
    }

    fn visit_primary_constructor_body(&mut self, node: PrimaryConstructorBody<'a>) {
        let saved_expected = std::mem::take(&mut self.state.expected);
        let saved_returns = std::mem::take(&mut self.state.return_type);
        self.state
            .type_parameters
            .push(AstChildren::empty(), &self.state.model);
        self.state.return_type.push(ResolvedType::DYNAMIC);
        for annotation in node.annotations() {
            self.visit_annotation(annotation);
        }
        self.with_primary_environment(|walker| {
            walker.visit_constructor_initializers(node.initializers());
        });
        if let Some(body) = node.body() {
            // The body sees declaring formals as fields, not promotable locals.
            std::mem::swap(
                &mut self.primary_environment,
                &mut self.primary_body_environment,
            );
            self.with_primary_environment(|walker| walker.visit_body(body));
            std::mem::swap(
                &mut self.primary_environment,
                &mut self.primary_body_environment,
            );
        }
        self.state.return_type = saved_returns;
        self.state.expected = saved_expected;
        self.state.type_parameters.pop();
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) {
        if !node.is_static() && !node.is_late() && self.primary_environment.is_some() {
            self.with_primary_environment(|walker| walker.visit_field_initializers(node));
        } else {
            self.visit_field_initializers(node);
        }
    }

    fn visit_constructor_decl(&mut self, node: ConstructorDecl<'a>) {
        let saved_expected = std::mem::take(&mut self.state.expected);
        let saved_returns = std::mem::take(&mut self.state.return_type);
        self.state
            .type_parameters
            .push(AstChildren::empty(), &self.state.model);
        // A factory body returns its enclosing self type, including owner type
        // parameters; generative constructors have no value-return context.
        self.state.return_type.push(if node.is_factory() {
            self.state
                .enclosing_type
                .clone()
                .unwrap_or(ResolvedType::UNKNOWN)
        } else {
            ResolvedType::DYNAMIC
        });
        let redirects_to_this = node
            .initializers()
            .any(|initializer| matches!(initializer, ConstructorInitializer::ThisCall(_)));
        let super_constructor = (!redirects_to_this).then(|| {
            node.initializers()
                .find_map(|initializer| match initializer {
                    ConstructorInitializer::SuperCall(call) => {
                        Some(call.call_name().map_or("new", |name| name.name()))
                    }
                    _ => None,
                })
                .unwrap_or("new")
        });
        self.with_scope(|walker| {
            walker.bind_params(node.params(), super_constructor, false, None);
            walker.visitor.visit_constructor(node, &walker.state);
            walker.visitor.visit_constructor_ref(
                ConstructorRef {
                    annotations: node.annotations(),
                    is_const: node.is_const(),
                    is_factory: node.is_factory(),
                    is_external: node.is_external(),
                    is_primary: false,
                    constructor_name: node.constructor_name(),
                    params: node.params(),
                    initializers: node.initializers(),
                    redirect: node.redirect(),
                    body: node.body(),
                    name_span: node.name().span(),
                    span: node.span(),
                },
                &walker.state,
            );
            walker.visit_param_constants(node.params());
            walker.visit_constructor_initializers(node.initializers());
            if let Some(body) = node.body() {
                walker.visit_body(body);
            }
        });
        self.state.return_type = saved_returns;
        self.state.expected = saved_expected;
        self.state.type_parameters.pop();
    }

    fn visit_getter_decl(&mut self, node: GetterDecl<'a>) {
        self.visit_function(
            Callable {
                nested: None,
                type_parameters: AstChildren::empty(),
                params: None,
                return_type: node.return_type(),
                body: node.body(),
                async_marker: node.async_marker(),
            },
            false,
        );
    }

    fn visit_setter_decl(&mut self, node: SetterDecl<'a>) {
        self.visit_declaration_annotations(node.param_annotations());
        self.visit_member_callable(
            (!node.is_static()).then_some(node.name().name()),
            true,
            Callable {
                nested: None,
                type_parameters: AstChildren::empty(),
                params: Some(CallableParams::Setter(node)),
                return_type: None,
                body: node.body(),
                async_marker: AsyncMarker::Sync,
            },
        );
    }

    fn visit_block(&mut self, node: Block<'a>) {
        for statement in node.stmts() {
            self.visit_stmt(statement);
        }
        self.visitor.visit_block_end(node, &self.state);
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) {
        let enclosing_cascade = std::mem::replace(&mut self.state.inside_cascade, false);
        self.visitor.visit_stmt(node, &self.state);
        match node {
            Stmt::Block(block) => self.with_scope(|walker| walker.visit_block(block)),
            Stmt::LocalVar(declaration) => self.visit_local_declaration(declaration),
            Stmt::PatternDecl(declaration) => {
                self.visit_declaration_annotations(declaration.annotations());
                self.visit_expr(declaration.init());
                self.visit_matched_pattern(
                    declaration.pattern(),
                    self.state.infer(declaration.init()),
                );
            }
            Stmt::PatternAssign(assignment) => {
                let context = self.assignment_pattern_context(assignment.pattern());
                self.with_expected(context.clone(), |walker| {
                    walker.visit_expr(assignment.value());
                });
                let matched = self
                    .state
                    .infer_with_context(assignment.value(), Some(&context));
                let previous = std::mem::replace(&mut self.assigning_pattern, true);
                self.visit_matched_pattern(assignment.pattern(), matched);
                self.assigning_pattern = previous;
            }
            Stmt::If(statement) => self.visit_if_statement(statement),
            Stmt::For(statement) => self.visit_for_statement(statement),
            Stmt::While(statement) => self.with_break_target(None, true, node, |walker| {
                let (_, exit) =
                    walker.enter_loop(statement.span().start, Some(statement.condition()));
                walker.with_scope(|walker| walker.visit_stmt(statement.body()));
                walker.state.environment = exit;
            }),
            Stmt::DoWhile(statement) => self.with_break_target(None, true, node, |walker| {
                walker.with_scope(|walker| {
                    let (entry, _) = walker.enter_loop(statement.span().start, None);
                    walker.visit_stmt(statement.body());
                    // `continue` reaches the condition from any point of the body.
                    if loop_jumps(statement.body(), true) {
                        walker.state.environment = walker.state.environment.join(&entry);
                    }
                    walker.visit_expr(statement.condition());
                    walker.assume_condition(statement.condition(), false);
                });
            }),
            Stmt::Switch(statement) => self.visit_switch_statement(statement),
            Stmt::TryCatch(statement) => self.visit_try_statement(statement),
            Stmt::Return(statement) => {
                if let Some(value) = statement.value() {
                    let expected = self
                        .state
                        .return_type
                        .last()
                        .cloned()
                        .unwrap_or(ResolvedType::UNKNOWN);
                    self.with_expected(expected, |walker| walker.visit_expr(value));
                }
            }
            Stmt::Throw(statement) => self.visit_expr(statement.value()),
            Stmt::Labeled(statement) => self.with_break_target(
                Some(statement.label().name()),
                false,
                statement.stmt(),
                |walker| walker.visit_stmt(statement.stmt()),
            ),
            Stmt::LocalFunc(function) => self.visit_local_function_statement(function),
            Stmt::Assert(statement) => {
                self.visit_expr(statement.condition());
                if let Some(message) = statement.message() {
                    self.visit_expr(message);
                }
            }
            Stmt::Yield(statement) => self.visit_yield_statement(statement),
            Stmt::Expr(statement) => self.visit_expr(statement.expr()),
            Stmt::Break(statement) => self.record_break(statement.label()),
            Stmt::Continue(_) | Stmt::Rethrow(_) | Stmt::Empty(_) | Stmt::Error(_) => {}
        }
        self.state.inside_cascade = enclosing_cascade;
    }

    fn visit_expr(&mut self, node: Expr<'a>) {
        if matches!(
            node,
            Expr::Binary(_)
                | Expr::Conditional(_)
                | Expr::Field(_)
                | Expr::Index(_)
                | Expr::NullAssert(_)
        ) {
            let depth = self.state.inference_depth.get();
            self.state.inference_depth.set(depth + 1);
            match node {
                Expr::Binary(binary) => self.visit_binary_chain(binary),
                Expr::Conditional(conditional) => self.visit_conditional_chain(conditional),
                _ => self.visit_selector_chain(node),
            }
            self.state.inference_depth.set(depth);
            if depth == 0 {
                self.state.inference_cache.borrow_mut().clear();
                self.state.inference_environment.borrow_mut().take();
            }
            return;
        }
        self.visitor.visit_expr(node, &self.state);
        let enclosing_cascade = self.state.inside_cascade;
        if matches!(node, Expr::Cascade(_)) {
            self.state.inside_cascade = true;
        }
        match node {
            Expr::Paren(n) => self.visit_expr(n.expr()),
            Expr::Call(n) => self.visit_call_expression(n),
            Expr::New(n) => self.visit_new_expression(n),
            Expr::FuncExpr(n) => self.visit_function_expression(n),
            Expr::Conditional(_) => unreachable!("conditional chain handled above"),
            Expr::Assign(n) if matches!(n.op(), AssignOp::NullCoalesceEq) => {
                let target = n.target();
                let value = n.value();
                self.visit_expr(target);
                let before = self.state.environment.clone();
                let expected = self.state.writable_type(target);
                self.with_expected(expected, |walker| walker.visit_expr(value));
                self.state.environment = before.join(&self.state.environment);
            }
            Expr::Cascade(n) => self.visit_cascade_expression(n),
            Expr::Assign(n) if matches!(n.op(), AssignOp::Eq) => {
                let target = n.target();
                let value = n.value();
                self.visit_expr(target);
                let expected = self.state.writable_type(target);
                self.with_expected(expected, |walker| walker.visit_expr(value));
            }
            Expr::List(n) => {
                let context = self.element_context(n.type_arg());
                for element in n.elements() {
                    self.visit_element_with_context(element, context.as_ref(), None);
                }
            }
            Expr::Set(n) => {
                let context = self.element_context(n.type_arg());
                for element in n.elements() {
                    self.visit_element_with_context(element, context.as_ref(), None);
                }
            }
            Expr::Map(n) => self.visit_map_expression(n),
            // A record literal's fields take the context record's field types.
            Expr::Record(n) => self.visit_record_expression(n),
            Expr::Switch(n) => self.visit_switch_expression(n),
            _ => self.with_expected(ResolvedType::UNKNOWN, |walker| walk_expr(walker, node)),
        }
        self.finish_visited_expression(node);
        self.state.inside_cascade = enclosing_cascade;
    }

    fn visit_pattern(&mut self, node: Pattern<'a>) {
        self.visitor
            .visit_pattern(node, self.state.matched.last(), &self.state);
        walk_pattern(self, node);
    }
}

impl<'a, V: SemanticRuleVisitor<'a> + ?Sized> ScopedWalker<'_, 'a, V> {
    fn visit_call_expression(&mut self, n: CallExpr<'a>) {
        let callee = n.callee();
        let type_args = n.type_args();
        let args = n.args();
        // A dot shorthand takes its context from the complete
        // selector chain, not from the final method's return type.
        let expected = self
            .state
            .expected()
            .filter(|expected| {
                !matches!(
                    expected,
                    ResolvedType {
                        kind: ResolvedTypeKind::Unknown
                    }
                ) && crate::expression_has_dot_shorthand_base(callee)
            })
            .cloned();
        self.with_context(expected, |walker| walker.visit_expr(callee));
        if let Some((positional, named)) =
            self.state
                .call_parameter_types(callee, type_args, args, self.state.expected())
        {
            self.visit_call_arguments(args, &positional, &named);
        } else {
            let callee_type = self.state.infer(callee);
            let untyped_member = match callee {
                Expr::Field(field) => {
                    let receiver = self.state.infer_receiver(field.object());
                    matches!(
                        receiver,
                        ResolvedType {
                            kind: ResolvedTypeKind::Dynamic
                        }
                    ) || (field.field().name() == "call"
                        && receiver.interface("dart:core", "Function"))
                }
                _ => false,
            };
            let expected = if matches!(
                callee_type,
                ResolvedType {
                    kind: ResolvedTypeKind::Dynamic
                }
            ) || callee_type.interface("dart:core", "Function")
                || untyped_member
            {
                ResolvedType::DYNAMIC
            } else {
                ResolvedType::UNKNOWN
            };
            for argument in args.positional() {
                self.with_expected(expected.clone(), |walker| walker.visit_expr(argument));
            }
            for argument in args.named() {
                self.with_expected(expected.clone(), |walker| {
                    walker.visit_expr(argument.value());
                });
            }
        }
    }

    fn visit_new_expression(&mut self, n: NewExpr<'a>) {
        let node = Expr::New(n);
        let dart_type = n.dart_type();
        let constructor_name = n.constructor_name();
        let args = n.args();
        let constructor =
            crate::resolve_new_constructor(dart_type, constructor_name, &self.state.model);
        let resolved = self.state.infer_with_context(node, self.state.expected());
        let parameters = self.state.constructor_parameter_types(
            &resolved,
            constructor.as_ref().map_or("new", |(_, name)| *name),
        );
        match parameters {
            Some((positional, named)) => self.visit_call_arguments(args, &positional, &named),
            None => self.visit_call_arguments(args, &[], &HashMap::new()),
        }
    }

    fn visit_function_expression(&mut self, n: FuncExpr<'a>) {
        let node = Expr::FuncExpr(n);
        let callable = Callable {
            nested: Some(n.span().start),
            type_parameters: n.type_params(),
            params: Some(CallableParams::List(n.params())),
            return_type: None,
            body: Some(n.body()),
            async_marker: n.async_marker(),
        };
        let inferred = self.state.infer_with_context(node, self.state.expected());
        if let ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    return_type,
                    positional,
                    named,
                    nullable,
                    alias,
                    optional_positional,
                    shape,
                },
        } = inferred
        {
            self.with_expected(
                ResolvedType {
                    kind: ResolvedTypeKind::Function {
                        shape,
                        optional_positional,
                        return_type,
                        positional,
                        named,
                        nullable,
                        alias,
                    },
                },
                |walker| walker.visit_function(callable, true),
            );
        } else {
            self.visit_function(callable, true);
        }
    }

    fn visit_map_expression(&mut self, n: MapExpr<'a>) {
        let type_args = n.type_args();
        let elements = n.elements();
        let component_type = |ordinal| {
            type_args
                .clone()
                .nth(ordinal)
                .map(|ty| {
                    self.state
                        .model
                        .resolve_type_in(ty, &self.state.type_parameters)
                })
                .or_else(|| {
                    self.state.expected().map(|expected| {
                        if expected.interface("dart:core", "Map") {
                            expected
                                .type_argument(ordinal)
                                .unwrap_or(ResolvedType::UNKNOWN)
                        } else {
                            ResolvedType::UNKNOWN
                        }
                    })
                })
        };
        let key_type = component_type(0);
        let value_type = component_type(1);
        for element in elements {
            self.visit_element_with_context(element, key_type.as_ref(), value_type.as_ref());
        }
    }

    fn visit_record_expression(&mut self, n: RecordExpr<'a>) {
        let node = Expr::Record(n);
        let fields = n.fields();
        if let Some(
            expected @ ResolvedType {
                kind: ResolvedTypeKind::Record { .. },
            },
        ) = self.state.expected().cloned()
        {
            let mut positional_index = 0;
            for field in fields {
                let expected = if let Some(name) = field.name() {
                    expected.record_field(name.name())
                } else {
                    let ty = expected.record_positional(positional_index);
                    positional_index += 1;
                    ty
                };
                self.with_expected(expected.unwrap_or(ResolvedType::UNKNOWN), |walker| {
                    walker.visit_expr(field.value());
                });
            }
        } else {
            self.with_expected(ResolvedType::UNKNOWN, |walker| walk_expr(walker, node));
        }
    }

    fn visit_switch_expression(&mut self, n: SwitchExpr<'a>) {
        let subject = n.subject();
        let arms = n.arms();
        self.visit_expr(subject);
        let matched = self.state.infer(subject);
        let before = self.state.environment.clone();
        let depth = before.depth();
        let mut ends = Vec::with_capacity(arms.len());
        let mut unmatched = before.clone();
        for arm in arms {
            self.state.environment.clone_from(&unmatched);
            let mut completes = true;
            self.with_scope(|walker| {
                unmatched = walker.visit_pattern_condition(
                    subject,
                    arm.pattern(),
                    arm.guard(),
                    matched.clone(),
                );
                walker.visit_expr(arm.body());
                completes = walker.state.infer(arm.body()) != ResolvedType::NEVER;
            });
            unmatched.pop_scope();
            ends.push((
                Cow::Owned(std::mem::take(&mut self.state.environment)),
                completes,
            ));
        }
        // A switch expression is exhaustive; with no completing arm,
        // control never reaches its end.
        self.state.environment = before;
        FlowContext::join_paths(&mut self.state.environment, ends, depth, false);
    }

    fn finish_visited_expression(&mut self, node: Expr<'a>) {
        let assigned = match node {
            Expr::Assign(n) if matches!(n.target(), Expr::Ident(_)) => {
                let target = n.target();
                let op = n.op();
                let value = n.value();
                // The write must use the same declared context as the RHS
                // visit; otherwise a generic constructor can lose its type
                // arguments and incorrectly discard a non-null promotion.
                if matches!(op, AssignOp::Eq | AssignOp::NullCoalesceEq) {
                    let expected = self.state.writable_type(target);
                    let inferred = self.state.infer_with_context(value, Some(&expected));
                    Some(if inferred == ResolvedType::UNKNOWN {
                        self.state.infer(value)
                    } else {
                        inferred
                    })
                } else {
                    Some(self.state.infer(value))
                }
            }
            _ => None,
        };
        FlowContext {
            model: &self.state.model,
            signatures: &self.state.signatures,
            type_parameters: &self.state.type_parameters,
            field_promotions: self.state.private_field_promotion,
        }
        .finish_expression(&mut self.state.environment, node, assigned);
    }

    fn visit_cascade_call(
        &mut self,
        n: CascadeCall<'a>,
        mut current: ResolvedType,
    ) -> ResolvedType {
        let name = n.name();
        let type_args = n.type_args();
        let args = n.args();
        if let Some((signature, owner)) =
            self.state
                .signatures
                .resolved_member(&current, name.name(), &self.state.model)
        {
            let substitutions = self
                .state
                .signature_substitutions(signature, &owner, &type_args, args, None);
            let (positional, named) = self.state.substituted_parameters(signature, &substitutions);
            let returned = self
                .state
                .model
                .substitute(&signature.return_type, &substitutions);
            self.visit_call_arguments(args, &positional, &named);
            current = returned;
        } else {
            for argument in args.positional() {
                self.with_expected(ResolvedType::UNKNOWN, |walker| walker.visit_expr(argument));
            }
            for argument in args.named() {
                self.with_expected(ResolvedType::UNKNOWN, |walker| {
                    walker.visit_expr(argument.value());
                });
            }
            current = self
                .state
                .signatures
                .instantiated_member_read(&current, name.name(), &self.state.model)
                .as_ref()
                .map_or_else(
                    || {
                        if current == ResolvedType::DYNAMIC {
                            ResolvedType::DYNAMIC
                        } else {
                            ResolvedType::UNKNOWN
                        }
                    },
                    SemanticState::callable_return_type,
                );
        }
        self.state.environment.demote_captured();
        current
    }

    fn visit_cascade_invoke(
        &mut self,
        n: CascadeInvoke<'a>,
        mut current: ResolvedType,
    ) -> ResolvedType {
        let type_args = n.type_args();
        let args = n.args();
        if let Some((signature, owner)) =
            self.state
                .signatures
                .resolved_member(&current, "call", &self.state.model)
        {
            let substitutions = self
                .state
                .signature_substitutions(signature, &owner, &type_args, args, None);
            let (positional, named) = self.state.substituted_parameters(signature, &substitutions);
            let returned = self
                .state
                .model
                .substitute(&signature.return_type, &substitutions);
            self.visit_call_arguments(args, &positional, &named);
            current = returned;
        } else {
            let returned = current.function_return_type();
            if let ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        positional, named, ..
                    },
            } = current
            {
                self.visit_call_arguments(args, &positional, &named.iter().cloned().collect());
                current = returned.unwrap_or(ResolvedType::UNKNOWN);
            } else {
                self.visit_call_arguments(args, &[], &HashMap::new());
                current = SemanticState::callable_return_type(&current);
            }
        }
        self.state.environment.demote_captured();
        current
    }

    fn visit_cascade_index(
        &mut self,
        n: CascadeIndex<'a>,
        mut current: ResolvedType,
    ) -> ResolvedType {
        let index = n.index();
        if let Some((signature, owner)) =
            self.state
                .signatures
                .resolved_member(&current, "[]", &self.state.model)
        {
            let expected = signature
                .positional
                .first()
                .map_or(ResolvedType::UNKNOWN, |ty| {
                    self.state.model.substitute(ty, &owner)
                });
            let returned = self.state.model.substitute(&signature.return_type, &owner);
            self.with_expected(expected, |walker| walker.visit_expr(index));
            current = returned;
        } else {
            self.with_expected(ResolvedType::UNKNOWN, |walker| walker.visit_expr(index));
            if current != ResolvedType::DYNAMIC {
                current = ResolvedType::UNKNOWN;
            }
        }
        current
    }

    fn visit_cascade_assignment_operation(
        &mut self,
        operation: CascadeOp<'a>,
        current: &ResolvedType,
        section: Span,
    ) {
        let (target, operator, value) = match operation {
            CascadeOp::Assign(n) => (n.target(), n.op(), n.value()),
            CascadeOp::IndexAssign(n) => (n.target(), n.op(), n.value()),
            _ => unreachable!(),
        };
        let indexed = matches!(operation, CascadeOp::IndexAssign(_));
        self.visitor.visit_cascade_assignment(
            CascadeAssignment {
                target,
                value,
                operator,
                indexed,
            },
            current,
            section,
            &self.state,
        );
        let (index_context, value_context) = if indexed {
            self.state
                .signatures
                .resolved_member(current, "[]=", &self.state.model)
                .map_or(
                    (ResolvedType::UNKNOWN, ResolvedType::UNKNOWN),
                    |(signature, owner)| {
                        let resolve = |position| {
                            signature
                                .positional
                                .get(position)
                                .map_or(ResolvedType::UNKNOWN, |ty| {
                                    self.state.model.substitute(ty, &owner)
                                })
                        };
                        (resolve(0), resolve(1))
                    },
                )
        } else if let Expr::Ident(identifier) = target {
            let expected = self
                .state
                .signatures
                .resolved_writable_field(current, identifier.name(), &self.state.model)
                .map_or(ResolvedType::UNKNOWN, |(ty, owner)| {
                    self.state.model.substitute(&ty, &owner)
                });
            (ResolvedType::UNKNOWN, expected)
        } else {
            (ResolvedType::UNKNOWN, ResolvedType::UNKNOWN)
        };
        self.with_expected(index_context, |walker| walker.visit_expr(target));
        self.with_expected(value_context, |walker| walker.visit_expr(value));
    }

    fn walk_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        mut current: ResolvedType,
        section: Span,
    ) -> ResolvedType {
        if match operation {
            CascadeOp::Field(n) => n.is_null_aware(),
            CascadeOp::Index(n) => n.is_null_aware(),
            _ => false,
        } {
            current = current.non_null();
        }
        self.visitor
            .visit_cascade_operation(operation, &current, section, &self.state);
        match operation {
            CascadeOp::NullAssert(_) => current = current.non_null(),
            CascadeOp::Field(n) => {
                let name = n.name();
                current = self
                    .state
                    .signatures
                    .instantiated_member_read(&current, name.name(), &self.state.model)
                    .unwrap_or_else(|| {
                        if current == ResolvedType::DYNAMIC {
                            ResolvedType::DYNAMIC
                        } else {
                            ResolvedType::UNKNOWN
                        }
                    });
            }
            CascadeOp::Call(n) => current = self.visit_cascade_call(n, current),
            CascadeOp::Invoke(n) => current = self.visit_cascade_invoke(n, current),
            CascadeOp::Index(n) => current = self.visit_cascade_index(n, current),
            CascadeOp::Assign(_) | CascadeOp::IndexAssign(_) => {
                self.visit_cascade_assignment_operation(operation, &current, section);
            }
        }
        current
    }

    fn visit_cascade_expression(&mut self, n: CascadeExpr<'a>) {
        let object = n.object();
        let sections = n.sections();
        let is_null_aware = n.is_null_aware();
        self.visit_expr(object);
        let receiver = self.state.infer_with_context(object, self.state.expected());
        let receiver = if is_null_aware {
            receiver.non_null()
        } else {
            receiver
        };
        for section in sections {
            let mut current = receiver.clone();
            for operation in section.ops() {
                current = self.walk_cascade_operation(operation, current, section.span());
            }
        }
    }
}

#[must_use]
pub fn returned_expressions(body: FunctionBody<'_>) -> Vec<Expr<'_>> {
    fn collect<'a>(statements: AstChildren<'a, Stmt<'a>>, expressions: &mut Vec<Expr<'a>>) {
        for statement in statements {
            collect_statement(statement, expressions);
        }
    }

    fn collect_statement<'a>(statement: Stmt<'a>, expressions: &mut Vec<Expr<'a>>) {
        match statement {
            Stmt::Return(node) => {
                if let Some(value) = node.value() {
                    expressions.push(value);
                }
            }
            Stmt::Block(block) => collect(block.stmts(), expressions),
            Stmt::If(node) => {
                collect_statement(node.then_branch(), expressions);
                if let Some(branch) = node.else_branch() {
                    collect_statement(branch, expressions);
                }
            }
            Stmt::For(node) => collect_statement(node.body(), expressions),
            Stmt::While(node) => collect_statement(node.body(), expressions),
            Stmt::DoWhile(node) => collect_statement(node.body(), expressions),
            Stmt::Switch(node) => {
                for case in node.cases() {
                    collect(case.body(), expressions);
                }
            }
            Stmt::TryCatch(node) => {
                collect(node.body().stmts(), expressions);
                for catch in node.catches() {
                    collect(catch.body().stmts(), expressions);
                }
                if let Some(block) = node.finally() {
                    collect(block.stmts(), expressions);
                }
            }
            Stmt::Labeled(node) => collect_statement(node.stmt(), expressions),
            // Expressions contain statements only inside excluded function expressions.
            Stmt::LocalFunc(_)
            | Stmt::Throw(_)
            | Stmt::Rethrow(_)
            | Stmt::Empty(_)
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::LocalVar(_)
            | Stmt::PatternDecl(_)
            | Stmt::PatternAssign(_)
            | Stmt::Assert(_)
            | Stmt::Yield(_)
            | Stmt::Expr(_)
            | Stmt::Error(_) => {}
        }
    }

    let mut expressions = Vec::new();
    match body {
        FunctionBody::Block(block) => collect(block.stmts(), &mut expressions),
        FunctionBody::Arrow(body) => expressions.push(body.expr()),
        FunctionBody::Native(_) => {}
    }
    expressions
}

fn collect_type_parameter_ids(
    ty: &ResolvedType,
    by_owner: &mut HashMap<ParameterOwner, Vec<TypeParameterId>>,
) {
    match ty {
        ResolvedType {
            kind:
                ResolvedTypeKind::TypeParameter { id, bound, .. }
                | ResolvedTypeKind::NonNullTypeParameter { id, bound },
        } => {
            let parameters = by_owner.entry(id.owner).or_default();
            if !parameters.contains(id) {
                parameters.push(id.clone());
            }
            collect_type_parameter_ids(bound, by_owner);
        }
        ResolvedType {
            kind: ResolvedTypeKind::Interface { arguments, .. },
        } => {
            for argument in arguments.iter() {
                collect_type_parameter_ids(argument, by_owner);
            }
        }
        ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    return_type,
                    positional,
                    named,
                    ..
                },
        } => {
            collect_type_parameter_ids(return_type, by_owner);
            for parameter in positional.iter() {
                collect_type_parameter_ids(parameter, by_owner);
            }
            for (_, parameter) in named.iter() {
                collect_type_parameter_ids(parameter, by_owner);
            }
        }
        ResolvedType {
            kind: ResolvedTypeKind::Record {
                positional, named, ..
            },
        } => {
            for field in positional.iter() {
                collect_type_parameter_ids(field, by_owner);
            }
            for (_, field) in named.iter() {
                collect_type_parameter_ids(field, by_owner);
            }
        }
        _ => {}
    }
}
