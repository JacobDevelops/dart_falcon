use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use falcon_analyze::constant_value::LocalConstants;
use falcon_analyze::resolve::flow::{FlowContext, loop_jumps};
use falcon_analyze::resolve::flow_writes::FlowWrites;
use falcon_analyze::{
    AnalyzeContext, ConstantValue, DeclarationIdentity, Feature, InferenceContext, ParameterOwner,
    QualifiedName, ResolvedSignature, ResolvedType, SemanticModel, SignatureIndex, TypeEnvironment,
    TypeParameterId, TypeParameterScope,
};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_annotation, walk_expr, walk_formal_param_annotations, walk_function_body,
    walk_pattern, walk_top_level_decl, walk_type_params,
};

/// Local constants retain both source syntax and their declaration-time value.
struct LocalConstant {
    depth: usize,
    declaration: Span,
    name: String,
    initializer: Expr,
    value: Option<ConstantValue>,
}

/// Assignment syntax with the shared walker's index-selector classification.
pub(crate) struct CascadeAssignment<'a> {
    pub(crate) target: &'a Expr,
    pub(crate) value: &'a Expr,
    pub(crate) operator: AssignOp,
    pub(crate) indexed: bool,
}

pub(crate) trait SemanticRuleVisitor {
    fn visit_expr(&mut self, _node: &Expr, _state: &SemanticState<'_>) {}

    fn visit_call_arguments(
        &mut self,
        _args: &ArgList,
        _positional: &[ResolvedType],
        _named: &HashMap<String, ResolvedType>,
        _state: &SemanticState<'_>,
    ) {
    }

    fn visit_stmt(&mut self, _node: &Stmt, _state: &SemanticState<'_>) {}

    fn visit_constructor(&mut self, _node: &ConstructorDecl, _state: &SemanticState<'_>) {}

    /// Called before shared cascade inference advances to the next receiver.
    fn visit_cascade_operation(
        &mut self,
        _operation: &CascadeOp,
        _receiver: &ResolvedType,
        _section: Span,
        _state: &SemanticState<'_>,
    ) {
    }

    /// Assignment targets retain the shared walker's index-selector classification.
    fn visit_cascade_assignment(
        &mut self,
        _assignment: CascadeAssignment<'_>,
        _receiver: &ResolvedType,
        _section: Span,
        _state: &SemanticState<'_>,
    ) {
    }

    fn visit_pattern(
        &mut self,
        _node: &Pattern,
        _matched: Option<&ResolvedType>,
        _state: &SemanticState<'_>,
    ) {
    }
}

pub(crate) struct SemanticState<'a> {
    pub(crate) model: SemanticModel<'a>,
    // Only the single-file fallback owns signatures.
    pub(crate) signatures: Cow<'a, SignatureIndex>,
    pub(crate) environment: TypeEnvironment,
    pub(crate) type_parameters: TypeParameterScope,
    /// Context types of the enclosing expressions; `None` marks a position
    /// without a context type (an untyped declaration's initializer), as
    /// opposed to `Some(Unknown)` for a context falcon cannot resolve.
    expected: Vec<Option<ResolvedType>>,
    return_type: Vec<ResolvedType>,
    matched: Vec<ResolvedType>,
    pub(crate) enclosing_type: Option<ResolvedType>,
    /// Lexical constant namespace, distinct from an extension's `this` on-type.
    pub(crate) constant_owner: Option<DeclarationIdentity>,
    superclass: Option<ResolvedType>,
    globals: &'a HashMap<String, ResolvedType>,
    /// `const` local variables in open scopes, with the environment scope
    /// index that binds them.
    local_constants: Vec<LocalConstant>,
    /// Variables each loop, switch, `try` and `catch` region writes, keyed by
    /// statement start, and each nested callable's executable writes.
    writes: &'a FlowWrites,
    private_field_promotion: bool,
    inference_depth: std::cell::Cell<usize>,
    inference_cache: std::cell::RefCell<HashMap<*const Expr, ResolvedType>>,
}

impl<'a> SemanticState<'a> {
    pub(crate) fn new(program: &Program, ctx: &'a AnalyzeContext<'a>) -> Option<Self> {
        let identities = ctx.identities?;
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let signatures = ctx
            .signatures
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(SignatureIndex::from_program(program, &model)));
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
            globals: ctx.globals(program),
            writes: ctx.flow_writes(program),
            superclass: None,
            local_constants: Vec::new(),
            private_field_promotion,
            inference_depth: std::cell::Cell::new(0),
            inference_cache: std::cell::RefCell::new(HashMap::new()),
        })
    }

    pub(crate) fn infer(&self, expression: &Expr) -> ResolvedType {
        if matches!(expression, Expr::This { .. }) {
            return self.enclosing_type.clone().unwrap_or(ResolvedType::Unknown);
        }
        self.inference_query(|| {
            let key = expression as *const Expr;
            if let Some(ty) = self.inference_cache.borrow().get(&key).cloned() {
                return ty;
            }
            let ty = self.infer_with_context(expression, None);
            self.inference_cache.borrow_mut().insert(key, ty.clone());
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
        }
        result
    }

    pub(crate) fn infer_with_context(
        &self,
        expression: &Expr,
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

    /// Official omission policy distinguishes generic top/local functions from
    /// ordinary methods and function-valued variables.
    pub(crate) fn generic_function_returns_type_parameter(&self, expression: &Expr) -> bool {
        let Expr::Call {
            callee, type_args, ..
        } = expression
        else {
            return false;
        };
        if !type_args.is_empty() || matches!(callee.as_ref(), Expr::GenericInstantiation { .. }) {
            return false;
        }
        match callee.as_ref() {
            Expr::Ident(identifier) if self.environment.is_local_function(&identifier.name) => {
                matches!(self.environment.lookup(&identifier.name),
                    ResolvedType::Function { return_type, .. } if matches!(return_type.as_ref(), ResolvedType::TypeParameter { .. }))
            }
            target => falcon_analyze::expression_segments(target, true)
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
                    matches!(signature.return_type, ResolvedType::TypeParameter { .. })
                }),
        }
    }

    /// Invocation results include null-chain lifting.
    pub(crate) fn infer_invocation(&self, expression: &Expr) -> ResolvedType {
        self.infer(expression)
    }

    /// The static type of a member access's receiver: `super` is the
    /// superclass, `x!` the non-nullable type of `x`, anything else
    /// [`Self::infer_invocation`].
    pub(crate) fn infer_receiver(&self, object: &Expr) -> ResolvedType {
        match object {
            Expr::Paren { expr, .. } => self.infer_receiver(expr),
            Expr::Super { .. } => self.superclass.clone().unwrap_or(ResolvedType::Unknown),
            Expr::NullAssert { operand, .. } => self.infer_receiver(operand).non_null(),
            other => self.infer_invocation(other),
        }
    }

    pub(crate) fn infer_access_receiver(
        &self,
        object: &Expr,
        name: &str,
        is_null_safe: bool,
    ) -> ResolvedType {
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

    pub(crate) fn callable_return_type(callable: &ResolvedType) -> ResolvedType {
        if let Some(return_type) = callable.function_return_type() {
            return return_type;
        }
        match callable {
            ResolvedType::Dynamic => ResolvedType::Dynamic,
            ty if ty.interface("dart:core", "Function") => ResolvedType::Dynamic,
            _ => ResolvedType::Unknown,
        }
    }

    pub(crate) fn cascade_invoke_return_type(
        &self,
        receiver: &ResolvedType,
        type_args: &[DartType],
        args: &ArgList,
    ) -> ResolvedType {
        self.signatures
            .resolved_member(receiver, "call", &self.model)
            .map(|(signature, owner)| {
                self.instantiate_return_type(signature, &owner, type_args, args, None)
            })
            .unwrap_or_else(|| Self::callable_return_type(receiver))
    }

    /// Whether the enclosing type itself declares a member `name`.
    fn enclosing_type_declares(&self, name: &str) -> bool {
        matches!(
            &self.enclosing_type,
            Some(ResolvedType::Interface { identity, .. })
                if !self.signatures.declared_member_facts(identity, name).is_empty()
        )
    }

    /// The type of an unqualified name that is not a local, in lexical lookup
    /// order: a member the enclosing type itself declares, library and imported
    /// declarations, then an inherited field or getter (through `this`) or a
    /// static field.
    fn implicit_member_type(&self, name: &str) -> Option<ResolvedType> {
        if self.environment.is_bound(name) {
            return None;
        }
        if let Some(promoted) = self.environment.promoted_this_field(name) {
            return Some(promoted);
        }
        if let Some(ResolvedType::Interface { identity, .. }) = &self.enclosing_type
            && self
                .signatures
                .declared_member_facts(identity, name)
                .iter()
                .any(|facts| matches!(facts.kind, falcon_analyze::MemberKind::Method))
        {
            // Generic tear-offs need the individual expression's context,
            // not the enclosing declaration's expected type.
            return None;
        }
        let member = || {
            let receiver = self.enclosing_type.as_ref()?;
            if let Some(ty) = self
                .signatures
                .instantiated_data_read(receiver, name, &self.model)
            {
                return Some(ty);
            }
            if let ResolvedType::Interface { identity, .. } = receiver
                && let Some((_, ty)) = self.signatures.field(identity, name)
            {
                return Some(ty.clone());
            }
            None
        };
        if self.enclosing_type_declares(name) {
            return Some(member().unwrap_or(ResolvedType::Unknown));
        }
        if self.model.resolve_value(name).is_some() || self.model.resolve_name(name).is_some() {
            return self
                .globals
                .get(name)
                .cloned()
                .filter(|ty| *ty != ResolvedType::Unknown);
        }
        self.globals.get(name).cloned().or_else(member)
    }

    pub(crate) fn expected(&self) -> Option<&ResolvedType> {
        self.expected.last()?.as_ref()
    }

    /// The initializer of the `const` local variable `name` refers to, when
    /// its innermost binding is one.
    pub(crate) fn local_constant(&self, name: &str) -> Option<&Expr> {
        let declaration = self.environment.binding_declaration(name)?;
        self.local_constants
            .iter()
            .rev()
            .find(|constant| constant.declaration == declaration)
            .map(|constant| &constant.initializer)
    }

    /// Declaration-time values and current promoted types of visible constants.
    pub(crate) fn visible_local_constants(&self) -> LocalConstants<'_> {
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
    pub(crate) fn expected_unknown(&self) -> bool {
        matches!(self.expected.last(), Some(Some(ResolvedType::Unknown)))
    }

    /// Whether the current expression `node` is implicitly torn off as
    /// `node.call`: an instance with a `call` method in a function-typed (or
    /// `Function`, possibly `FutureOr`-wrapped) context, or such an instance
    /// instantiated with type arguments (`c<int>`). The analyzer wraps such an
    /// expression in an `ImplicitCallReference`.
    pub(crate) fn implicit_call_tearoff(&self, node: &Expr) -> bool {
        match node {
            Expr::Field { field, .. } if field.name == "call" => return false,
            Expr::GenericInstantiation { target, .. } => {
                return self.has_call_method(&self.infer(target));
            }
            _ => {}
        }
        let Some(expected) = self.expected() else {
            return false;
        };
        let function_context = |ty: &ResolvedType| {
            matches!(ty, ResolvedType::Function { .. }) || ty.interface("dart:core", "Function")
        };
        let function_context = function_context(expected)
            || expected.interface("dart:async", "FutureOr")
                && expected.arguments().first().is_some_and(function_context);
        function_context && self.has_call_method(&self.infer(node))
    }

    /// Whether instances of `ty` have an instance `call` method.
    fn has_call_method(&self, ty: &ResolvedType) -> bool {
        let is_call = |fact: &falcon_analyze::MemberFacts| {
            fact.kind == falcon_analyze::MemberKind::Method && !fact.is_static
        };
        match ty {
            ResolvedType::TypeParameter { bound, .. } => self.has_call_method(bound),
            ResolvedType::Function { .. } | ResolvedType::Unknown | ResolvedType::Dynamic => false,
            ResolvedType::Interface { identity, .. }
                if self.signatures.type_kind(identity)
                    == Some(falcon_analyze::resolve::TypeKind::ExtensionType) =>
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

    pub(crate) fn is_own_field(&self, name: &str) -> bool {
        self.environment.is_bound_in_root_scope(name)
            && self.enclosing_type.as_ref().is_some_and(|enclosing| {
                self.signatures
                    .resolved_member_owner(enclosing, name, &self.model)
                    .is_some()
            })
    }

    /// Whether an unqualified read denotes storage rather than an explicit
    /// getter. Follow the same local/member/library order as type inference.
    pub(crate) fn is_variable_read(&self, name: &str) -> bool {
        if self.environment.is_bound(name) {
            return !self.environment.is_local_function(name);
        }
        let member_is_field = || {
            self.enclosing_type.as_ref().is_some_and(|receiver| {
                self.signatures
                    .resolved_member_facts(receiver, name, &self.model)
                    .is_some_and(|facts| {
                        facts.iter().any(|fact| {
                            matches!(fact.kind, falcon_analyze::MemberKind::Field { .. })
                        }) && !facts
                            .iter()
                            .any(|fact| fact.kind == falcon_analyze::MemberKind::Getter)
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

    pub(crate) fn writable_type(&self, target: &Expr) -> ResolvedType {
        let member = match target {
            Expr::Ident(identifier) if self.environment.is_bound(&identifier.name) => {
                return self.environment.declared_type(&identifier.name);
            }
            Expr::Ident(identifier) if !self.environment.is_bound(&identifier.name) => {
                if let Some(ty) = self.globals.get(&identifier.name) {
                    return ty.clone();
                }
                self.enclosing_type
                    .as_ref()
                    .map(|receiver| (receiver.clone(), identifier.name.as_str()))
            }
            Expr::Field {
                object,
                field,
                is_null_safe,
                ..
            } => {
                if let Expr::Ident(identifier) = object.as_ref()
                    && !self.environment.is_bound(&identifier.name)
                    && let Some(identity) = self.model.resolve_name(&identifier.name)
                    && let Some((_, ty)) = self.signatures.field(&identity, &field.name)
                {
                    return ty.clone();
                }
                Some((
                    self.infer_access_receiver(object, &field.name, *is_null_safe),
                    field.name.as_str(),
                ))
            }
            Expr::Index {
                object,
                is_null_safe,
                ..
            } => {
                return self
                    .signatures
                    .resolved_member(
                        &self.infer_access_receiver(object, "[]=", *is_null_safe),
                        "[]=",
                        &self.model,
                    )
                    .and_then(|(signature, substitutions)| {
                        signature
                            .positional
                            .get(1)
                            .map(|ty| self.model.substitute(ty, &substitutions))
                    })
                    .unwrap_or(ResolvedType::Unknown);
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

    pub(crate) fn named_extension(
        &self,
        callee: &Expr,
    ) -> Option<falcon_analyze::DeclarationIdentity> {
        let segments = falcon_analyze::expression_segments(callee, true)?;
        let first = segments.first()?;
        if self.environment.is_bound(first)
            || self.enclosing_type.as_ref().is_some_and(|receiver| {
                self.signatures
                    .resolved_readable_member_owner(receiver, first, &self.model)
                    .is_some()
            })
        {
            return None;
        }
        self.model
            .resolve_extension(QualifiedName::from_segments(&segments)?)
    }

    pub(crate) fn call_signature(
        &self,
        callee: &Expr,
        type_args: &[DartType],
        args: &ArgList,
        expected: Option<&ResolvedType>,
    ) -> Option<ResolvedSignature> {
        let target = self.call_target(callee, type_args)?;
        let substitutions = self.signature_substitutions_with_result(
            target.inference_signature(),
            &target.owner,
            target.type_args,
            args,
            expected,
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
                    positional,
                    named,
                    return_type,
                    defaults: signature.defaults.clone(),
                    positional_parameter_names: signature.positional_parameter_names.clone(),
                    non_optional_defaults: signature.non_optional_defaults.clone(),
                }
            }
            Cow::Owned(mut signature) => {
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

    pub(crate) fn call_return_type(
        &self,
        callee: &Expr,
        type_args: &[DartType],
        args: &ArgList,
        expected: Option<&ResolvedType>,
    ) -> Option<ResolvedType> {
        let target = self.call_target(callee, type_args)?;
        let substitutions = self.signature_substitutions_with_result(
            target.inference_signature(),
            &target.owner,
            target.type_args,
            args,
            expected,
        );
        Some(
            self.model
                .substitute(target.inference_signature().2, &substitutions),
        )
    }

    fn call_parameter_types(
        &self,
        callee: &Expr,
        type_args: &[DartType],
        args: &ArgList,
        expected: Option<&ResolvedType>,
    ) -> Option<(Vec<ResolvedType>, HashMap<String, ResolvedType>)> {
        let target = self.call_target(callee, type_args)?;
        let substitutions = self.signature_substitutions_with_result(
            target.inference_signature(),
            &target.owner,
            target.type_args,
            args,
            expected,
        );
        Some(self.substituted_parameters(&target.signature, &substitutions))
    }

    fn call_target<'s>(
        &'s self,
        callee: &'s Expr,
        type_args: &'s [DartType],
    ) -> Option<falcon_analyze::ResolvedInvocation<'s>> {
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
        let ResolvedType::Interface { identity, .. } = receiver else {
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
    pub(crate) fn signature_substitutions(
        &self,
        signature: &ResolvedSignature,
        owner: &HashMap<TypeParameterId, ResolvedType>,
        type_args: &[DartType],
        args: &ArgList,
        expected: Option<&ResolvedType>,
    ) -> HashMap<TypeParameterId, ResolvedType> {
        self.signature_substitutions_with_result(
            (
                signature,
                &signature.call_parameters,
                &signature.return_type,
            ),
            owner,
            type_args,
            args,
            expected,
        )
    }

    fn signature_substitutions_with_result(
        &self,
        signature: (&ResolvedSignature, &[TypeParameterId], &ResolvedType),
        owner: &HashMap<TypeParameterId, ResolvedType>,
        type_args: &[DartType],
        args: &ArgList,
        expected: Option<&ResolvedType>,
    ) -> HashMap<TypeParameterId, ResolvedType> {
        let context = InferenceContext {
            model: &self.model,
            signatures: &self.signatures,
            type_parameters: &self.type_parameters,
            lookup: &|name| self.implicit_member_type(name),
            argument_fallback: None,
        };
        self.environment.infer_signature_substitutions_with_result(
            signature, args, type_args, &context, expected, owner,
        )
    }

    fn instantiate_return_type(
        &self,
        signature: &ResolvedSignature,
        owner: &HashMap<TypeParameterId, ResolvedType>,
        type_args: &[DartType],
        args: &ArgList,
        expected: Option<&ResolvedType>,
    ) -> ResolvedType {
        let substitutions =
            self.signature_substitutions(signature, owner, type_args, args, expected);
        self.model
            .substitute(&signature.return_type, &substitutions)
    }

    pub(crate) fn substituted_parameters(
        &self,
        signature: &ResolvedSignature,
        substitutions: &HashMap<TypeParameterId, ResolvedType>,
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
pub(crate) fn explicit_call_receiver(callee: &Expr) -> Option<&Expr> {
    if let Expr::Field { object, field, .. } = callee
        && field.name == "call"
    {
        Some(object)
    } else {
        None
    }
}

pub(crate) fn visit_program<'a, V: SemanticRuleVisitor>(
    visitor: &mut V,
    program: &Program,
    state: SemanticState<'a>,
) -> SemanticState<'a> {
    let mut walker = ScopedWalker {
        visitor,
        state,
        assigning_pattern: false,
        break_targets: Vec::new(),
        primary_environment: None,
        primary_body_environment: None,
    };
    walker.visit_program(program);
    walker.state
}

/// The type a super parameter `param` of the constructor taking `params`
/// inherits from the super constructor `constructor_name` of `superclass`.
/// Named super parameters match by name; positional ones by their position
/// among the positional super parameters (a super call cannot also pass
/// positional arguments).
pub(crate) fn resolved_super_param_type(
    param: &FormalParam,
    params: &FormalParamList,
    superclass: &ResolvedType,
    constructor_name: &str,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> Option<ResolvedType> {
    let ResolvedType::Interface { identity, .. } = superclass else {
        return None;
    };
    let signature = signatures.constructor(identity, constructor_name)?;
    let ty = if matches!(param.kind, ParamKind::Named { .. }) {
        signature.named.get(&param.name.name)?
    } else {
        let index = params
            .positional()
            .filter(|positional| positional.initializing == Some(InitializingFormal::Super))
            .position(|positional| std::ptr::eq(positional, param))?;
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

struct ScopedWalker<'a, 'model, V> {
    visitor: &'a mut V,
    state: SemanticState<'model>,
    assigning_pattern: bool,
    /// Open `break` targets of the current callable, innermost last.
    break_targets: Vec<BreakTarget>,
    /// All primary formals are visible in constructor initializers.
    primary_environment: Option<TypeEnvironment>,
    /// Body blocks bind only non-declaring, non-initializing primary formals.
    primary_body_environment: Option<TypeEnvironment>,
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
struct Callable<'f> {
    /// The start of a local function or function literal.
    nested: Option<usize>,
    type_parameters: &'f [TypeParam],
    params: Option<&'f FormalParamList>,
    return_type: Option<&'f DartType>,
    body: Option<&'f FunctionBody>,
    async_marker: AsyncMarker,
}

impl<V: SemanticRuleVisitor> ScopedWalker<'_, '_, V> {
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

    fn assume_condition(&mut self, expression: &Expr, truth: bool) {
        FlowContext {
            model: &self.state.model,
            signatures: &self.state.signatures,
            type_parameters: &self.state.type_parameters,
            field_promotions: self.state.private_field_promotion,
        }
        .assume_condition(&mut self.state.environment, expression, truth);
    }

    fn assume_pattern(&mut self, value: &Expr, pattern: &Pattern, truth: bool) {
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
        value: &Expr,
        pattern: &Pattern,
        guard: Option<&Expr>,
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

    fn statement_exits(&self, statement: &Stmt) -> bool {
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
        condition: &Expr,
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
        if else_exits {
            self.state.environment = then_environment;
        } else if !then_exits {
            self.state.environment = then_environment.join(&self.state.environment);
        }
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
        statement: &Stmt,
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
                !matches!(statement.condition, Expr::BoolLit { value: true, .. })
            }
            Stmt::DoWhile(statement) => {
                !matches!(statement.condition, Expr::BoolLit { value: true, .. })
            }
            Stmt::For(statement)
                if matches!(
                    statement.header.init,
                    None | Some(ForInit::VarDecl(_) | ForInit::Exprs(_))
                ) =>
            {
                statement
                    .header
                    .condition
                    .as_ref()
                    .is_some_and(|condition| {
                        !matches!(condition, Expr::BoolLit { value: true, .. })
                    })
            }
            _ => true,
        };
        let completes = condition_exit && !self.statement_exits(statement);
        self.join_states(states, depth, completes);
    }

    /// Joins `states` (possibly from scopes deeper than `depth`) into the
    /// current state at `depth`; when the current state is unreachable
    /// (`reachable` false), they replace it.
    fn join_states(&mut self, states: Vec<TypeEnvironment>, depth: usize, reachable: bool) {
        let mut states = states.into_iter();
        if !reachable && let Some(mut first) = states.next() {
            while first.depth() > depth {
                first.pop_scope();
            }
            self.state.environment = first;
        }
        for state in states {
            self.state.environment = self.state.environment.join(&state);
        }
    }

    fn record_break(&mut self, label: Option<&Identifier>) {
        let target = self
            .break_targets
            .iter_mut()
            .rev()
            .find(|target| match label {
                Some(label) => target.label.as_deref() == Some(label.name.as_str()),
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
        condition: Option<&Expr>,
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

    fn block_exits(&self, statements: &[Stmt]) -> bool {
        statements
            .iter()
            .any(|statement| self.statement_exits(statement))
    }

    fn with_expected(&mut self, expected: ResolvedType, f: impl FnOnce(&mut Self)) {
        self.with_context(Some(expected), f);
    }

    fn with_context(&mut self, expected: Option<ResolvedType>, f: impl FnOnce(&mut Self)) {
        self.state.expected.push(expected);
        f(self);
        self.state.expected.pop();
    }

    fn without_expected(&mut self, f: impl FnOnce(&mut Self)) {
        self.with_context(None, f);
    }

    fn with_cleared_expected(&mut self, f: impl FnOnce(&mut Self)) {
        let outer = std::mem::take(&mut self.state.expected);
        f(self);
        self.state.expected = outer;
    }

    /// The context type of a list or set literal's elements: its type
    /// argument, else the element type of an iterable context. `None` when the
    /// literal has no context, `Some(Unknown)` when it is unresolved.
    fn element_context(&self, type_arg: Option<&DartType>) -> Option<ResolvedType> {
        if let Some(type_arg) = type_arg {
            return Some(
                self.state
                    .model
                    .resolve_type_in(type_arg, &self.state.type_parameters),
            );
        }
        let expected = self.state.expected()?;
        if matches!(expected, ResolvedType::Unknown) {
            return Some(ResolvedType::Unknown);
        }
        self.state
            .signatures
            .instantiated_supertype(expected, "dart:core", "Iterable", &self.state.model)
            .and_then(|iterable| iterable.type_argument(0))
            .or(Some(ResolvedType::Unknown))
    }

    fn visit_body(&mut self, body: &FunctionBody) {
        match body {
            FunctionBody::Arrow(expression, _) => {
                let expected = self
                    .state
                    .return_type
                    .last()
                    .cloned()
                    .unwrap_or(ResolvedType::Unknown);
                self.with_expected(expected, |walker| walker.visit_expr(expression));
            }
            _ => walk_function_body(self, body),
        }
    }

    /// Declaration bodies use their lexical owner's completed signature.
    fn visit_member_callable(&mut self, name: Option<&str>, setter: bool, callable: Callable<'_>) {
        let contextual = name.and_then(|name| {
            let receiver = self.state.enclosing_type.as_ref()?;
            let ResolvedType::Interface { identity, .. } = receiver else {
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
                return Some(ResolvedType::Function {
                    optional_positional: 0,
                    alias: Default::default(),
                    return_type: ResolvedType::Void.into(),
                    positional: Arc::from([self.state.model.substitute(&ty, &substitutions)]),
                    named: Default::default(),
                    nullable: false,
                });
            }
            let signature = self.state.signatures.member(identity, name)?;
            Some(signature.function_type())
        });
        if let Some(expected) = contextual {
            self.with_expected(expected, |walker| {
                walker.visit_function_context(callable, true, true)
            });
        } else {
            self.visit_function(callable, false);
        }
    }

    /// Visits a callable; one with a `nested` start keeps the enclosing
    /// promotions of variables its executable never writes.
    fn visit_function(&mut self, callable: Callable<'_>, contextual: bool) {
        self.visit_function_context(callable, contextual, false);
    }

    fn visit_function_context(
        &mut self,
        callable: Callable<'_>,
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
            &[][..]
        } else {
            type_parameters
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
            .push(type_parameters, &self.state.model);
        self.with_cleared_expected(|walker| walk_type_params(walker, type_parameters));
        let saved_expected = (!contextual).then(|| std::mem::take(&mut self.state.expected));
        let saved_returns = (!contextual).then(|| std::mem::take(&mut self.state.return_type));
        // Match the context's generic owner once for both parameters and return.
        let contextual_substitutions = contextual
            .then(|| {
                if contextual_parameters.is_empty() {
                    return Some(HashMap::new());
                }
                let expected = self.state.expected()?;
                if !matches!(expected, ResolvedType::Function { .. }) {
                    return None;
                }
                let mut by_owner = HashMap::new();
                collect_type_parameter_ids(expected, &mut by_owner);
                let parameters = by_owner.values().find(|parameters| {
                    contextual_parameters
                        .iter()
                        .enumerate()
                        .all(|(ordinal, declared)| {
                            parameters.iter().any(|parameter| {
                                parameter.ordinal == ordinal
                                    && parameter.name.as_ref() == declared.name.name
                            })
                        })
                })?;
                Some(
                    parameters
                        .iter()
                        .filter_map(|parameter| {
                            contextual_parameters
                                .get(parameter.ordinal)
                                .and_then(|declared| {
                                    self.state.type_parameters.lookup(&declared.name.name)
                                })
                                .map(|resolved| (parameter.clone(), resolved))
                        })
                        .collect(),
                )
            })
            .flatten();
        let resolved_return = return_type
            .map(|ty| {
                self.state
                    .model
                    .resolve_type_in(ty, &self.state.type_parameters)
            })
            .or_else(|| {
                contextual.then(|| {
                    self.contextual_closure_type(contextual_substitutions.as_ref(), None)
                        .unwrap_or_else(|| self.uncontextual_type())
                })
            })
            .unwrap_or(ResolvedType::Dynamic);
        let resolved_return = match resolved_return {
            ResolvedType::Interface {
                identity,
                arguments,
                occurrence,
                ..
            } if async_marker == AsyncMarker::Async && identity.is_sdk("dart:async", "Future") => {
                ResolvedType::Interface {
                    occurrence,
                    identity: falcon_analyze::DeclarationIdentity::sdk("dart:async", "FutureOr"),
                    arguments,
                    nullable: false,
                    extension_type: false,
                }
            }
            resolved_return => resolved_return,
        };
        self.state.return_type.push(resolved_return);
        self.with_scope(|walker| {
            if let Some(params) = params {
                walker.bind_params(params, None, contextual, contextual_substitutions.as_ref());
                walker.visit_param_constants(params);
            }
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
        substitutions: Option<&HashMap<TypeParameterId, ResolvedType>>,
        parameter: Option<(&FormalParam, usize)>,
    ) -> Option<ResolvedType> {
        let substitutions = substitutions?;
        let ResolvedType::Function {
            return_type,
            positional,
            named,
            ..
        } = self.state.expected()?
        else {
            return None;
        };
        let contextual_type = match parameter {
            None => return_type.as_ref(),
            Some((parameter, ordinal)) => match parameter.kind {
                ParamKind::Named { .. } => named
                    .iter()
                    .find(|(name, _)| name == &parameter.name.name)
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
                    Some(ResolvedType::TypeParameter { id: scoped, .. }) if scoped == *id)
            })
            .then_some(contextual_type)
    }

    fn bind_params(
        &mut self,
        params: &FormalParamList,
        super_constructor: Option<&str>,
        contextual: bool,
        contextual_substitutions: Option<&HashMap<TypeParameterId, ResolvedType>>,
    ) {
        let mut positional_ordinal = 0;
        for param in params.params.iter() {
            let mut ty = self
                .state
                .model
                .resolve_formal_parameter_in(param, &self.state.type_parameters);
            if param.param_type.is_none() && param.function_params.is_none() {
                if param.initializing == Some(InitializingFormal::Super) {
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
                        .unwrap_or(ResolvedType::Unknown);
                } else if param.initializing == Some(InitializingFormal::This) {
                    ty = self.state.environment.lookup(&param.name.name);
                    if matches!(ty, ResolvedType::Unknown) {
                        ty = self
                            .state
                            .enclosing_type
                            .as_ref()
                            .and_then(|enclosing| {
                                self.state.signatures.resolved_field(
                                    enclosing,
                                    &param.name.name,
                                    &self.state.model,
                                )
                            })
                            .map(|(ty, substitutions)| {
                                self.state.model.substitute(&ty, &substitutions)
                            })
                            .unwrap_or(ResolvedType::Unknown);
                    }
                }
                if contextual {
                    ty = self
                        .contextual_closure_type(
                            contextual_substitutions,
                            Some((param, positional_ordinal)),
                        )
                        .unwrap_or_else(|| self.uncontextual_type());
                }
            }
            self.state
                .environment
                .declare_at(param.name.name.clone(), ty, param.name.span);
            if !matches!(param.kind, ParamKind::Named { .. }) {
                positional_ordinal += 1;
            }
        }
    }

    fn visit_param_constants(&mut self, params: &FormalParamList) {
        for param in params.params.iter() {
            self.with_cleared_expected(|walker| walk_formal_param_annotations(walker, param));
            if let Some(value) = &param.default_value {
                let expected = self.state.environment.lookup(&param.name.name);
                self.with_expected(expected, |walker| walker.visit_expr(value));
            }
        }
    }

    fn declare_fields(&mut self, members: EffectiveMembers<'_>) {
        for param in members.declaring_params() {
            let ty = self
                .state
                .model
                .resolve_formal_parameter_in(param, &self.state.type_parameters);
            self.declare_field(&param.name.name, ty);
        }
        if let Some(primary) = members.primary {
            let super_constructor = members
                .declared
                .iter()
                .find_map(|member| {
                    let ClassMember::PrimaryConstructorBody(body) = member else {
                        return None;
                    };
                    body.initializers
                        .iter()
                        .find_map(|initializer| match initializer {
                            ConstructorInitializer::SuperCall { call_name, .. } => {
                                Some(call_name.as_ref().map_or("new", |name| name.name.as_str()))
                            }
                            _ => None,
                        })
                })
                .unwrap_or("new");
            self.with_scope(|walker| {
                walker.bind_params(&primary.params, Some(super_constructor), false, None);
                walker.state.environment.declare_unpromotable(
                    "this",
                    walker
                        .state
                        .enclosing_type
                        .clone()
                        .unwrap_or(ResolvedType::Unknown),
                );
                walker.primary_environment = Some(walker.state.environment.clone());
            });
        }
        for member in members.declared {
            let ClassMember::Field(field) = member else {
                continue;
            };
            for declarator in &field.declarators {
                let ty = field
                    .field_type
                    .as_ref()
                    .map(|ty| {
                        self.state
                            .model
                            .resolve_type_in(ty, &self.state.type_parameters)
                    })
                    .or_else(|| {
                        declarator.initializer.as_ref().map(|initializer| {
                            let mut inferred = ResolvedType::Unknown;
                            if !field.is_static && !field.is_late {
                                self.with_primary_environment(|walker| {
                                    inferred = walker.state.infer_invocation(initializer);
                                });
                            } else {
                                inferred = self.state.infer_invocation(initializer);
                            }
                            inferred
                        })
                    })
                    .unwrap_or(ResolvedType::Unknown);
                self.declare_field(&declarator.name.name, ty);
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

    fn visit_local_declaration(&mut self, declaration: &LocalVarDecl) {
        self.visit_declaration_annotations(&declaration.annotations);
        for declarator in &declaration.declarators {
            let declared = declaration.var_type.as_ref().map(|ty| {
                self.state
                    .model
                    .resolve_type_in(ty, &self.state.type_parameters)
            });
            if let Some(initializer) = &declarator.initializer {
                self.with_context(declared.clone(), |walker| walker.visit_expr(initializer));
            }
            let initialized = declarator
                .initializer
                .as_ref()
                .map(|initializer| self.state.infer_invocation(initializer));
            // Mutable and constant explicitly typed locals promote to an
            // initializer's type of interest, such as `NonNull(declared)`.
            let (ty, written) = match declared {
                Some(declared) => (declared, initialized),
                None => (initialized.unwrap_or(ResolvedType::Unknown), None),
            };
            self.state.environment.declare_at(
                declarator.name.name.clone(),
                ty,
                declarator.name.span,
            );
            if matches!(declaration.keyword, VarKeyword::Var | VarKeyword::Const)
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
                    &declarator.name.name,
                    written,
                );
            }
            if declaration.keyword == VarKeyword::Const
                && let Some(initializer) = &declarator.initializer
            {
                let expected = declaration
                    .var_type
                    .as_ref()
                    .map(|_| self.state.environment.declared_type(&declarator.name.name));
                let value =
                    falcon_analyze::constant_value::evaluate_constant_with_local_values_in_context(
                        initializer,
                        self.state.constant_owner.as_ref(),
                        &self.state.visible_local_constants(),
                        &self.state.model,
                        &self.state.signatures,
                        expected.as_ref(),
                    );
                self.state.local_constants.push(LocalConstant {
                    depth: self.state.environment.depth() - 1,
                    declaration: declarator.name.span,
                    name: declarator.name.name.clone(),
                    initializer: initializer.clone(),
                    value,
                });
            }
        }
    }

    fn iterable_element_type(&self, iterable: &Expr, is_await: bool) -> ResolvedType {
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
    fn assignment_pattern_context(&self, pattern: &Pattern) -> ResolvedType {
        match pattern {
            Pattern::Variable { name, .. } => self.state.environment.lookup(&name.name),
            Pattern::ParenPattern { inner, .. } => self.assignment_pattern_context(inner),
            Pattern::Record(record) => {
                let named_count = record
                    .fields
                    .iter()
                    .filter(|field| field.name.is_some())
                    .count();
                let mut positional = Vec::with_capacity(record.fields.len() - named_count);
                let mut named = Vec::with_capacity(named_count);
                for field in &record.fields {
                    let ty = self.assignment_pattern_context(&field.pattern);
                    if let Some(name) = &field.name {
                        named.push((name.name.clone(), ty));
                    } else {
                        positional.push(ty);
                    }
                }
                named.sort_by(|left, right| left.0.cmp(&right.0));
                ResolvedType::Record {
                    occurrence: Default::default(),
                    positional: positional.into(),
                    named: named.into(),
                    nullable: false,
                }
            }
            Pattern::List(list) => {
                let mut element = ResolvedType::Unknown;
                for child in &list.elements {
                    let ty = match child {
                        ListPatternElement::Pattern(child) => {
                            self.assignment_pattern_context(child)
                        }
                        ListPatternElement::Rest(Some(child), _) => self
                            .assignment_pattern_context(child)
                            .type_argument(0)
                            .unwrap_or(ResolvedType::Unknown),
                        ListPatternElement::Rest(None, _) => continue,
                    };
                    if matches!(element, ResolvedType::Unknown)
                        || self
                            .state
                            .signatures
                            .subtype(&ty, &element, &self.state.model)
                            == falcon_analyze::Truth::Yes
                    {
                        element = ty;
                    } else if !matches!(ty, ResolvedType::Unknown)
                        && self
                            .state
                            .signatures
                            .subtype(&element, &ty, &self.state.model)
                            != falcon_analyze::Truth::Yes
                    {
                        element = ResolvedType::Never;
                    }
                }
                let mut list = self.state.model.core_type("List", false);
                if let ResolvedType::Interface { arguments, .. } = &mut list {
                    *arguments = Arc::from([element]);
                }
                list
            }
            _ => ResolvedType::Unknown,
        }
    }

    fn visit_matched_pattern(&mut self, pattern: &Pattern, matched: ResolvedType) {
        self.state.matched.push(matched.clone());
        self.visitor
            .visit_pattern(pattern, Some(&matched), &self.state);
        match pattern {
            Pattern::Variable { type_, name, .. } => {
                let ty = type_
                    .as_ref()
                    .map(|ty| {
                        self.state
                            .model
                            .resolve_type_in(ty, &self.state.type_parameters)
                    })
                    .unwrap_or(matched);
                if self.assigning_pattern {
                    FlowContext {
                        model: &self.state.model,
                        signatures: &self.state.signatures,
                        type_parameters: &self.state.type_parameters,
                        field_promotions: self.state.private_field_promotion,
                    }
                    .assign_variable(
                        &mut self.state.environment,
                        &name.name,
                        &ty,
                    );
                } else {
                    self.state
                        .environment
                        .declare_at(name.name.clone(), ty, name.span);
                }
            }
            Pattern::Wildcard { .. } | Pattern::Literal(_) | Pattern::Error { .. } => {}
            Pattern::Const(constant) => {
                if let ConstPatternValue::Expr(expression) = &constant.value {
                    self.visit_expr(expression);
                }
            }
            Pattern::List(list) => {
                let element = list
                    .type_arg
                    .as_ref()
                    .map(|ty| {
                        self.state
                            .model
                            .resolve_type_in(ty, &self.state.type_parameters)
                    })
                    .or_else(|| {
                        self.state
                            .signatures
                            .instantiated_supertype(
                                &matched,
                                "dart:core",
                                "List",
                                &self.state.model,
                            )
                            .and_then(|ty| ty.type_argument(0))
                    })
                    .unwrap_or(ResolvedType::Unknown);
                for child in &list.elements {
                    match child {
                        ListPatternElement::Pattern(child) => {
                            self.visit_matched_pattern(child, element.clone());
                        }
                        ListPatternElement::Rest(Some(child), _) => {
                            self.visit_matched_pattern(child, matched.clone());
                        }
                        ListPatternElement::Rest(None, _) => {}
                    }
                }
            }
            Pattern::Record(record) => {
                let mut positional_index = 0;
                for field in &record.fields {
                    let field_type = field
                        .name
                        .as_ref()
                        .and_then(|name| matched.record_field(&name.name))
                        .unwrap_or_else(|| {
                            let ty = matched
                                .record_positional(positional_index)
                                .unwrap_or(ResolvedType::Unknown);
                            positional_index += 1;
                            ty
                        });
                    self.visit_matched_pattern(&field.pattern, field_type);
                }
            }
            Pattern::Map(map) => {
                let value_type = self
                    .state
                    .signatures
                    .instantiated_supertype(&matched, "dart:core", "Map", &self.state.model)
                    .and_then(|ty| ty.type_argument(1))
                    .unwrap_or(ResolvedType::Unknown);
                for entry in &map.entries {
                    self.visit_expr(&entry.key);
                    self.visit_matched_pattern(&entry.pattern, value_type.clone());
                }
            }
            Pattern::Object(object) => {
                let object_type = FlowContext {
                    model: &self.state.model,
                    signatures: &self.state.signatures,
                    type_parameters: &self.state.type_parameters,
                    field_promotions: self.state.private_field_promotion,
                }
                .object_pattern_type(&object.type_, &matched);
                for field in &object.fields {
                    let field_type = self
                        .state
                        .signatures
                        .resolved_field(&object_type, &field.name.name, &self.state.model)
                        .or_else(|| {
                            self.state.signatures.resolved_extension_getter(
                                &object_type,
                                &field.name.name,
                                &self.state.model,
                            )
                        })
                        .map(|(ty, substitutions)| self.state.model.substitute(&ty, &substitutions))
                        .unwrap_or(ResolvedType::Unknown);
                    if let Some(child) = &field.pattern {
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
                            &field.name.name,
                            &field_type,
                        );
                    } else {
                        self.state.environment.declare_at(
                            field.name.name.clone(),
                            field_type,
                            field.name.span,
                        );
                    }
                }
            }
            Pattern::LogicalAnd { left, right, .. } | Pattern::LogicalOr { left, right, .. } => {
                self.visit_matched_pattern(left, matched.clone());
                self.visit_matched_pattern(right, matched.clone());
            }
            Pattern::Relational { value, .. } => self.visit_expr(value),
            Pattern::Cast {
                inner, cast_type, ..
            } => {
                let cast = self
                    .state
                    .model
                    .resolve_type_in(cast_type, &self.state.type_parameters);
                self.visit_matched_pattern(inner, cast);
            }
            Pattern::NullCheck { inner, .. } | Pattern::NullAssert { inner, .. } => {
                self.visit_matched_pattern(inner, matched.non_null());
            }
            Pattern::ParenPattern { inner, .. } => {
                self.visit_matched_pattern(inner, matched);
            }
        }
        self.state.matched.pop();
    }

    fn visit_for_init(&mut self, init: &ForInit, is_await: bool) {
        match init {
            ForInit::VarDecl(declaration) => self.visit_local_declaration(declaration),
            ForInit::ForIn {
                annotations,
                var_type,
                name,
                iterable,
                declares_variable: true,
                ..
            } => {
                self.visit_declaration_annotations(annotations);
                self.visit_expr(iterable);
                let ty = var_type
                    .as_ref()
                    .map(|ty| {
                        self.state
                            .model
                            .resolve_type_in(ty, &self.state.type_parameters)
                    })
                    .unwrap_or_else(|| self.iterable_element_type(iterable, is_await));
                self.state
                    .environment
                    .declare_at(name.name.clone(), ty, name.span);
            }
            // `for (x in xs)` writes an existing `x` at each iteration's start.
            ForInit::ForIn {
                declares_variable: false,
                iterable,
                ..
            } => self.visit_expr(iterable),
            ForInit::PatternForIn {
                annotations,
                pattern,
                iterable,
                ..
            } => {
                self.visit_declaration_annotations(annotations);
                self.visit_expr(iterable);
                let matched = self.iterable_element_type(iterable, is_await);
                self.visit_matched_pattern(pattern, matched);
            }
            ForInit::Exprs(expressions) => {
                for expression in expressions {
                    self.visit_expr(expression);
                }
            }
        }
    }

    /// Assigns a bare `for (x in xs)` loop's element to `x` as an iteration starts.
    fn assign_for_in(&mut self, init: Option<&ForInit>, is_await: bool) {
        if let Some(ForInit::ForIn {
            declares_variable: false,
            name,
            iterable,
            ..
        }) = init
        {
            let element = self.iterable_element_type(iterable, is_await);
            FlowContext {
                model: &self.state.model,
                signatures: &self.state.signatures,
                type_parameters: &self.state.type_parameters,
                field_promotions: self.state.private_field_promotion,
            }
            .assign_variable(&mut self.state.environment, &name.name, &element);
        }
    }

    /// Enters a collection `for` element's loop; returns its exit state.
    fn visit_for_header(
        &mut self,
        header: &ForHeader,
        region: usize,
        is_await: bool,
    ) -> TypeEnvironment {
        if let Some(init) = &header.init {
            self.visit_for_init(init, is_await);
        }
        let exit = self.enter_loop(region, header.condition.as_ref()).1;
        self.assign_for_in(header.init.as_ref(), is_await);
        exit
    }

    fn visit_if_condition(
        &mut self,
        condition: &IfCondition,
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
            IfCondition::Case(value, pattern, guard) => {
                self.visit_expr(value);
                let matched = self.state.infer(value);
                let mut unmatched = None;
                self.with_scope(|walker| {
                    unmatched = Some(walker.visit_pattern_condition(
                        value,
                        pattern,
                        guard.as_deref(),
                        matched,
                    ));
                    then_branch(walker);
                });
                let mut unmatched = unmatched.expect("pattern failure");
                unmatched.pop_scope();
                let then_environment = std::mem::replace(&mut self.state.environment, unmatched);
                self.with_scope(else_branch);
                if else_exits {
                    self.state.environment = then_environment;
                } else if !then_exits {
                    self.state.environment = then_environment.join(&self.state.environment);
                }
            }
        }
    }

    /// Visits a list or set element; expressions get the element context,
    /// spreads an unresolved one.
    fn visit_collection_element(
        &mut self,
        element: &CollectionElement,
        context: &Option<ResolvedType>,
    ) {
        match element {
            CollectionElement::Expr(expression)
            | CollectionElement::NullAware {
                expr: expression, ..
            } => self.with_context(context.clone(), |walker| walker.visit_expr(expression)),
            CollectionElement::Spread {
                expr: expression, ..
            } => {
                let expected = context.as_ref().map(|element| ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: DeclarationIdentity::sdk("dart:core", "Iterable"),
                    arguments: Arc::from([element.clone()]),
                    nullable: false,
                    extension_type: false,
                });
                self.with_context(expected, |walker| walker.visit_expr(expression));
            }
            CollectionElement::If {
                condition,
                then_elem,
                else_elem,
                ..
            } => self.visit_if_condition(
                condition,
                |walker| walker.visit_collection_element(then_elem, context),
                |walker| {
                    if let Some(element) = else_elem {
                        walker.visit_collection_element(element, context);
                    }
                },
                false,
                false,
            ),
            CollectionElement::For {
                is_await,
                header,
                element,
                span,
                ..
            } => self.with_scope(|walker| {
                let exit = walker.visit_for_header(header, span.start, *is_await);
                walker.visit_collection_element(element, context);
                for update in &header.updates {
                    walker.visit_expr(update);
                }
                walker.state.environment = exit;
            }),
        }
    }

    fn visit_map_element(
        &mut self,
        element: &MapElement,
        key_type: &Option<ResolvedType>,
        value_type: &Option<ResolvedType>,
    ) {
        match element {
            MapElement::Entry(entry) => {
                self.with_context(key_type.clone(), |walker| walker.visit_expr(&entry.key));
                self.with_context(value_type.clone(), |walker| walker.visit_expr(&entry.value));
            }
            MapElement::Spread { expr, .. } => {
                self.with_expected(ResolvedType::Unknown, |walker| walker.visit_expr(expr));
            }
            MapElement::If {
                condition,
                then_entry,
                else_entry,
                ..
            } => self.visit_if_condition(
                condition,
                |walker| walker.visit_map_element(then_entry, key_type, value_type),
                |walker| {
                    if let Some(entry) = else_entry {
                        walker.visit_map_element(entry, key_type, value_type);
                    }
                },
                false,
                false,
            ),
            MapElement::For {
                is_await,
                header,
                entry,
                span,
                ..
            } => self.with_scope(|walker| {
                let exit = walker.visit_for_header(header, span.start, *is_await);
                walker.visit_map_element(entry, key_type, value_type);
                for update in &header.updates {
                    walker.visit_expr(update);
                }
                walker.state.environment = exit;
            }),
        }
    }

    fn visit_field_initializers(&mut self, node: &FieldDecl) {
        for declarator in &node.declarators {
            if let Some(initializer) = &declarator.initializer {
                // An untyped instance field may inherit its type from an
                // overridden member; only an untyped static field has no context.
                let expected = match &node.field_type {
                    Some(ty) => Some(
                        self.state
                            .model
                            .resolve_type_in(ty, &self.state.type_parameters),
                    ),
                    None if node.is_static => None,
                    None => Some(ResolvedType::Unknown),
                };
                self.with_context(expected, |walker| walker.visit_expr(initializer));
            }
        }
    }

    fn visit_constructor_initializers(&mut self, initializers: &[ConstructorInitializer]) {
        for initializer in initializers {
            match initializer {
                ConstructorInitializer::SuperCall {
                    args, call_name, ..
                }
                | ConstructorInitializer::ThisCall {
                    args, call_name, ..
                } => {
                    let receiver = match initializer {
                        ConstructorInitializer::SuperCall { .. } => self.state.superclass.as_ref(),
                        _ => self.state.enclosing_type.as_ref(),
                    };
                    let parameters = receiver.and_then(|receiver| {
                        self.state.constructor_parameter_types(
                            receiver,
                            call_name.as_ref().map_or("new", |name| name.name.as_str()),
                        )
                    });
                    match parameters {
                        Some((positional, named)) => {
                            self.visit_call_arguments(args, &positional, &named)
                        }
                        None => self.visit_call_arguments(args, &[], &HashMap::new()),
                    }
                }
                ConstructorInitializer::FieldInit { field, value, .. } => {
                    let expected = self
                        .state
                        .enclosing_type
                        .as_ref()
                        .and_then(|enclosing| {
                            self.state.signatures.resolved_field(
                                enclosing,
                                &field.name,
                                &self.state.model,
                            )
                        })
                        .map(|(ty, substitutions)| self.state.model.substitute(&ty, &substitutions))
                        .unwrap_or(ResolvedType::Unknown);
                    self.with_expected(expected, |walker| walker.visit_expr(value));
                }
                ConstructorInitializer::Assert {
                    condition, message, ..
                } => {
                    self.visit_expr(condition);
                    if let Some(message) = message {
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
        identity: Option<DeclarationIdentity>,
        name: &str,
        type_args: &[DartType],
        args: &ArgList,
    ) {
        let signature = identity
            .as_ref()
            .and_then(|identity| self.state.signatures.constructor(identity, name));
        let Some(signature) = signature else {
            return self.visit_call_arguments(args, &[], &HashMap::new());
        };
        let substitutions = self.state.signature_substitutions_with_result(
            (
                signature,
                &signature.owner_parameters,
                &signature.return_type,
            ),
            &HashMap::new(),
            type_args,
            args,
            None,
        );
        let (positional, named) = self.state.substituted_parameters(signature, &substitutions);
        self.visit_call_arguments(args, &positional, &named);
    }

    /// Annotations on declarations the walker visits through custom callable
    /// or initializer walks; their arguments are constant constructor arguments.
    fn visit_declaration_annotations(&mut self, annotations: &[Annotation]) {
        for annotation in annotations {
            self.with_expected(ResolvedType::Unknown, |walker| {
                walker.visit_annotation(annotation)
            });
        }
    }

    fn visit_call_arguments(
        &mut self,
        args: &ArgList,
        positional: &[ResolvedType],
        named: &HashMap<String, ResolvedType>,
    ) {
        self.visitor
            .visit_call_arguments(args, positional, named, &self.state);
        // Arguments without a resolved parameter have an unknown context; they
        // must not inherit the context of the enclosing expression.
        for (index, argument) in args.positional.iter().enumerate() {
            let expected = positional
                .get(index)
                .cloned()
                .unwrap_or(ResolvedType::Unknown);
            self.with_expected(expected, |walker| walker.visit_expr(argument));
        }
        for argument in &args.named {
            let expected = named
                .get(&argument.name.name)
                .cloned()
                .unwrap_or(ResolvedType::Unknown);
            self.with_expected(expected, |walker| walker.visit_expr(&argument.value));
        }
    }
}

impl<V: SemanticRuleVisitor> Visitor for ScopedWalker<'_, '_, V> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        let enclosing_name = match node {
            TopLevelDecl::Class(declaration) => Some(&declaration.name.name),
            TopLevelDecl::Mixin(declaration) => Some(&declaration.name.name),
            TopLevelDecl::MixinClass(declaration) => Some(&declaration.name.name),
            TopLevelDecl::Enum(declaration) => Some(&declaration.name.name),
            TopLevelDecl::ExtensionType(declaration) => Some(&declaration.name.name),
            TopLevelDecl::ClassTypeAlias(_)
            | TopLevelDecl::Extension(_)
            | TopLevelDecl::Function(_)
            | TopLevelDecl::Variable(_)
            | TopLevelDecl::TypeAlias(_)
            | TopLevelDecl::Error(_) => None,
        };
        let superclass = match node {
            TopLevelDecl::Class(declaration) => declaration.extends.as_ref(),
            TopLevelDecl::MixinClass(declaration) => declaration.extends.as_ref(),
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
            TopLevelDecl::Class(declaration) => Some((
                declaration.type_params.as_slice(),
                declaration.members.as_slice(),
            )),
            TopLevelDecl::Mixin(declaration) => Some((
                declaration.type_params.as_slice(),
                declaration.members.as_slice(),
            )),
            TopLevelDecl::MixinClass(declaration) => Some((
                declaration.type_params.as_slice(),
                declaration.members.as_slice(),
            )),
            TopLevelDecl::Enum(declaration) => Some((
                declaration.type_params.as_slice(),
                declaration.members.as_slice(),
            )),
            TopLevelDecl::Extension(declaration) => Some((
                declaration.type_params.as_slice(),
                declaration.members.as_slice(),
            )),
            TopLevelDecl::ExtensionType(declaration) => Some((
                declaration.type_params.as_slice(),
                declaration.members.as_slice(),
            )),
            _ => None,
        };
        if let Some((type_parameters, members)) = scoped {
            self.with_fresh_environment(|walker| {
                let previous_constant_owner = std::mem::replace(
                    &mut walker.state.constant_owner,
                    match node {
                        TopLevelDecl::Extension(extension) => extension
                            .name
                            .as_ref()
                            .and_then(|name| walker.state.model.resolve_extension(&name.name)),
                        _ => enclosing_name.and_then(|name| walker.state.model.resolve_name(name)),
                    },
                );
                walker
                    .state
                    .type_parameters
                    .push(type_parameters, &walker.state.model);
                // Instantiate the enclosing type with its own type parameters, so
                // member and constructor lookups have something to substitute.
                let arguments: Vec<ResolvedType> = type_parameters
                    .iter()
                    .map(|param| {
                        walker
                            .state
                            .type_parameters
                            .lookup(&param.name.name)
                            .unwrap_or(ResolvedType::Unknown)
                    })
                    .collect();
                // `this` in an extension is its `on` type.
                let enclosing = match node {
                    TopLevelDecl::Extension(extension) => walker
                        .state
                        .model
                        .resolve_type_in(&extension.on_type, &walker.state.type_parameters),
                    _ => enclosing_name
                        .and_then(|name| walker.state.model.resolve_name(name))
                        .map(|identity| ResolvedType::Interface {
                            occurrence: Default::default(),
                            identity,
                            arguments: arguments.into(),
                            nullable: false,
                            extension_type: false,
                        })
                        .unwrap_or(ResolvedType::Unknown),
                };
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
                    effective_members.unwrap_or_else(|| EffectiveMembers::declared(members));
                let previous_primary = walker.primary_environment.take();
                let previous_primary_body = walker.primary_body_environment.take();
                walker.declare_fields(effective_members);
                walker.state.environment.declare_unpromotable(
                    "this",
                    walker
                        .state
                        .enclosing_type
                        .clone()
                        .unwrap_or(ResolvedType::Unknown),
                );
                if let Some(primary) = effective_members.primary
                    && members.iter().any(|member| {
                        matches!(
                            member,
                            ClassMember::PrimaryConstructorBody(body) if body.body.is_some()
                        )
                    })
                {
                    walker.with_scope(|walker| {
                        for param in &primary.params.params {
                            if effective_members.representation
                                || param.is_var
                                || param.is_final
                                || param.initializing.is_some()
                            {
                                continue;
                            }
                            let ty = walker
                                .state
                                .model
                                .resolve_formal_parameter_in(param, &walker.state.type_parameters);
                            walker
                                .state
                                .environment
                                .declare(param.name.name.clone(), ty);
                        }
                        walker.primary_body_environment = Some(walker.state.environment.clone());
                    });
                }
                if matches!(node, TopLevelDecl::Enum(_)) {
                    walker.with_expected(ResolvedType::Unknown, |walker| {
                        walk_top_level_decl(walker, node)
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
            });
        } else if matches!(node, TopLevelDecl::Function(_)) {
            self.with_fresh_environment(|walker| walk_top_level_decl(walker, node));
        } else if let TopLevelDecl::Variable(variable) = node {
            self.visit_declaration_annotations(&variable.annotations);
            for declarator in &variable.declarators {
                if let Some(initializer) = &declarator.initializer {
                    let expected = variable
                        .var_type
                        .is_some()
                        .then(|| self.state.globals.get(&declarator.name.name).cloned())
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
    fn visit_enum_decl(&mut self, node: &EnumDecl) {
        for annotation in &node.annotations {
            self.visit_annotation(annotation);
        }
        self.visit_identifier(&node.name);
        walk_type_params(self, &node.type_params);
        if let Some(constructor) = &node.primary_constructor {
            self.visit_primary_constructor(constructor);
        }
        for ty in node.with_clause.iter().chain(&node.implements) {
            self.visit_dart_type(ty);
        }
        let identity = self.state.model.resolve_name(node.name.name.as_str());
        for variant in &node.variants {
            for annotation in &variant.annotations {
                self.visit_annotation(annotation);
            }
            self.visit_identifier(&variant.name);
            for ty in &variant.type_args {
                self.visit_dart_type(ty);
            }
            if let Some(args) = &variant.args {
                let name = variant
                    .constructor_name
                    .as_ref()
                    .map_or("new", |name| name.name.as_str());
                self.visit_constructor_arguments(identity.clone(), name, &variant.type_args, args);
            }
        }
        for member in &node.members {
            self.visit_class_member(member);
        }
    }

    /// An annotation with arguments invokes a constant constructor.
    fn visit_annotation(&mut self, node: &Annotation) {
        let Some(args) = &node.args else {
            return walk_annotation(self, node);
        };
        for ty in &node.type_args {
            self.visit_dart_type(ty);
        }
        let model = &self.state.model;
        let segments: Vec<&str> = node
            .name
            .iter()
            .map(|segment| segment.name.as_str())
            .collect();
        let target = match &node.constructor_name {
            Some(name) => model
                .resolve_dotted(&segments)
                .map(|identity| (identity, name.name.as_str())),
            None => model
                .resolve_dotted(&segments)
                .map(|identity| (identity, "new"))
                .or_else(|| {
                    let (name, owner) = segments.split_last()?;
                    Some((model.resolve_dotted(owner)?, *name))
                }),
        };
        let (identity, name) = target.unzip();
        self.visit_constructor_arguments(identity, name.unwrap_or("new"), &node.type_args, args);
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.visit_declaration_annotations(&node.annotations);
        self.visit_function(
            Callable {
                nested: None,
                type_parameters: &node.type_params,
                params: node.params(),
                return_type: node.return_type.as_ref(),
                body: node.body.as_ref(),
                async_marker: node.async_marker,
            },
            false,
        );
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.visit_member_callable(
            (!node.is_static).then_some(node.name.name.as_str()),
            false,
            Callable {
                nested: None,
                type_parameters: &node.type_params,
                params: Some(&node.params),
                return_type: node.return_type.as_ref(),
                body: node.body.as_ref(),
                async_marker: node.async_marker,
            },
        );
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        let annotations = match node {
            ClassMember::Field(member) => member.annotations.as_slice(),
            ClassMember::Constructor(member) => &member.annotations,
            ClassMember::Method(member) => &member.annotations,
            ClassMember::Getter(member) => &member.annotations,
            ClassMember::Setter(member) => &member.annotations,
            ClassMember::Operator(member) => &member.annotations,
            ClassMember::PrimaryConstructorBody(_) | ClassMember::Error(_) => &[],
        };
        self.visit_declaration_annotations(annotations);
        if let ClassMember::Operator(operator) = node {
            self.visit_member_callable(
                Some(operator.lookup_name()),
                false,
                Callable {
                    nested: None,
                    type_parameters: &[],
                    params: Some(&operator.params),
                    return_type: operator.return_type.as_ref(),
                    body: operator.body.as_ref(),
                    async_marker: operator.async_marker,
                },
            );
        } else {
            falcon_syntax::visitor::walk_class_member(self, node);
        }
    }

    fn visit_primary_constructor(&mut self, node: &PrimaryConstructor) {
        self.with_primary_environment(|walker| {
            walker.visit_param_constants(&node.params);
        });
    }

    fn visit_primary_constructor_body(&mut self, node: &PrimaryConstructorBody) {
        let saved_expected = std::mem::take(&mut self.state.expected);
        let saved_returns = std::mem::take(&mut self.state.return_type);
        self.state.type_parameters.push(&[], &self.state.model);
        self.state.return_type.push(ResolvedType::Dynamic);
        for annotation in &node.annotations {
            self.visit_annotation(annotation);
        }
        self.with_primary_environment(|walker| {
            walker.visit_constructor_initializers(&node.initializers);
        });
        if let Some(body) = &node.body {
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

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        if !node.is_static && !node.is_late && self.primary_environment.is_some() {
            self.with_primary_environment(|walker| walker.visit_field_initializers(node));
        } else {
            self.visit_field_initializers(node);
        }
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        let saved_expected = std::mem::take(&mut self.state.expected);
        let saved_returns = std::mem::take(&mut self.state.return_type);
        self.state.type_parameters.push(&[], &self.state.model);
        // A factory body returns its enclosing self type, including owner type
        // parameters; generative constructors have no value-return context.
        self.state.return_type.push(if node.is_factory {
            self.state
                .enclosing_type
                .clone()
                .unwrap_or(ResolvedType::Unknown)
        } else {
            ResolvedType::Dynamic
        });
        let redirects_to_this = node
            .initializers
            .iter()
            .any(|initializer| matches!(initializer, ConstructorInitializer::ThisCall { .. }));
        let super_constructor = (!redirects_to_this).then(|| {
            node.initializers
                .iter()
                .find_map(|initializer| match initializer {
                    ConstructorInitializer::SuperCall { call_name, .. } => {
                        Some(call_name.as_ref().map_or("new", |name| name.name.as_str()))
                    }
                    _ => None,
                })
                .unwrap_or("new")
        });
        self.with_scope(|walker| {
            walker.bind_params(&node.params, super_constructor, false, None);
            walker.visitor.visit_constructor(node, &walker.state);
            walker.visit_param_constants(&node.params);
            walker.visit_constructor_initializers(&node.initializers);
            if let Some(body) = &node.body {
                walker.visit_body(body);
            }
        });
        self.state.return_type = saved_returns;
        self.state.expected = saved_expected;
        self.state.type_parameters.pop();
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        self.visit_function(
            Callable {
                nested: None,
                type_parameters: &[],
                params: None,
                return_type: node.return_type.as_ref(),
                body: node.body.as_ref(),
                async_marker: node.async_marker,
            },
            false,
        );
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        self.visit_declaration_annotations(&node.param_annotations);
        let params = FormalParamList {
            params: vec![FormalParam {
                annotations: Vec::new(),
                kind: falcon_syntax::ast::ParamKind::Positional,
                is_covariant: false,
                is_final: false,
                is_var: false,
                initializing: None,
                param_type: node.param_type.clone(),
                name: node.param.clone(),
                default_value: None,
                type_params: Vec::new(),
                function_params: None,
                span: node.param.span,
            }],
            span: node.span,
        };
        self.visit_member_callable(
            (!node.is_static).then_some(node.name.name.as_str()),
            true,
            Callable {
                nested: None,
                type_parameters: &[],
                params: Some(&params),
                return_type: None,
                body: node.body.as_ref(),
                async_marker: AsyncMarker::Sync,
            },
        );
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        self.visitor.visit_stmt(node, &self.state);
        match node {
            Stmt::Block(block) => self.with_scope(|walker| {
                for statement in &block.stmts {
                    walker.visit_stmt(statement);
                }
            }),
            Stmt::LocalVar(declaration) => self.visit_local_declaration(declaration),
            Stmt::PatternDecl(declaration) => {
                self.visit_declaration_annotations(&declaration.annotations);
                self.visit_expr(&declaration.init);
                self.visit_matched_pattern(
                    &declaration.pattern,
                    self.state.infer_invocation(&declaration.init),
                );
            }
            Stmt::PatternAssign(assignment) => {
                let context = self.assignment_pattern_context(&assignment.pattern);
                self.with_expected(context.clone(), |walker| {
                    walker.visit_expr(&assignment.value)
                });
                let matched = self
                    .state
                    .infer_with_context(&assignment.value, Some(&context));
                let previous = std::mem::replace(&mut self.assigning_pattern, true);
                self.visit_matched_pattern(&assignment.pattern, matched);
                self.assigning_pattern = previous;
            }
            Stmt::If(statement) => {
                if let IfCondition::Expr(condition) = &statement.condition {
                    self.visit_expr(condition);
                    self.visit_branches(
                        condition,
                        |walker| walker.visit_stmt(&statement.then_branch),
                        |walker| {
                            if let Some(branch) = &statement.else_branch {
                                walker.visit_stmt(branch);
                            }
                        },
                        self.statement_exits(&statement.then_branch),
                        statement
                            .else_branch
                            .as_deref()
                            .is_some_and(|branch| self.statement_exits(branch)),
                    );
                } else {
                    self.visit_if_condition(
                        &statement.condition,
                        |walker| walker.visit_stmt(&statement.then_branch),
                        |walker| {
                            if let Some(branch) = &statement.else_branch {
                                walker.visit_stmt(branch);
                            }
                        },
                        self.statement_exits(&statement.then_branch),
                        statement
                            .else_branch
                            .as_deref()
                            .is_some_and(|branch| self.statement_exits(branch)),
                    );
                }
            }
            Stmt::For(statement) => self.with_scope(|walker| {
                if let Some(init) = &statement.header.init {
                    walker.visit_for_init(init, statement.is_await);
                }
                let body = &statement.body;
                walker.with_break_target(None, true, node, |walker| {
                    let (_, exit) = walker
                        .enter_loop(statement.span.start, statement.header.condition.as_ref());
                    walker.assign_for_in(statement.header.init.as_ref(), statement.is_await);
                    walker.visit_stmt(body);
                    for update in &statement.header.updates {
                        walker.visit_expr(update);
                    }
                    walker.state.environment = exit;
                });
            }),
            Stmt::While(statement) => self.with_break_target(None, true, node, |walker| {
                let (_, exit) = walker.enter_loop(statement.span.start, Some(&statement.condition));
                walker.with_scope(|walker| walker.visit_stmt(&statement.body));
                walker.state.environment = exit;
            }),
            Stmt::DoWhile(statement) => self.with_break_target(None, true, node, |walker| {
                walker.with_scope(|walker| {
                    let (entry, _) = walker.enter_loop(statement.span.start, None);
                    walker.visit_stmt(&statement.body);
                    // `continue` reaches the condition from any point of the body.
                    if loop_jumps(&statement.body, true) {
                        walker.state.environment = walker.state.environment.join(&entry);
                    }
                    walker.visit_expr(&statement.condition);
                    walker.assume_condition(&statement.condition, false);
                })
            }),
            Stmt::Switch(statement) => {
                self.visit_expr(&statement.subject);
                let matched = self.state.infer(&statement.subject);
                // A `continue label;` may re-enter any labeled case.
                if statement.cases.iter().any(|case| !case.labels.is_empty()) {
                    self.enter_region(statement.span.start);
                }
                let exhaustive = statement.cases.iter().any(|case| {
                    case.cases
                        .iter()
                        .any(|kind| matches!(kind, SwitchCaseKind::Default))
                }) || FlowContext {
                    model: &self.state.model,
                    signatures: &self.state.signatures,
                    type_parameters: &self.state.type_parameters,
                    field_promotions: self.state.private_field_promotion,
                }
                .switch_is_exhaustive(statement, &matched);
                let before = self.state.environment.clone();
                let depth = before.depth();
                let mut ends = Vec::with_capacity(statement.cases.len() + 1);
                self.break_targets.push(BreakTarget {
                    label: None,
                    unlabeled: true,
                    states: Vec::new(),
                });
                // An empty case's matches reach the next case's body.
                let mut matched_empty: Option<TypeEnvironment> = None;
                let mut unmatched = before.clone();
                for (index, case) in statement.cases.iter().enumerate() {
                    self.state.environment.clone_from(&unmatched);
                    let falls_through = case.body.is_empty() && index + 1 < statement.cases.len();
                    self.with_scope(|walker| {
                        // Later heads start from the preceding head's failure.
                        unmatched.clone_from(&walker.state.environment);
                        let mut matches = matched_empty.take();
                        for kind in &case.cases {
                            walker.state.environment.clone_from(&unmatched);
                            match kind {
                                SwitchCaseKind::Pattern(pattern, guard) => {
                                    unmatched = walker.visit_pattern_condition(
                                        &statement.subject,
                                        pattern,
                                        guard.as_deref(),
                                        matched.clone(),
                                    );
                                }
                                SwitchCaseKind::Expr(expression) => walker.visit_expr(expression),
                                SwitchCaseKind::Default => {}
                            }
                            matches = Some(match matches {
                                Some(previous) => walker.state.environment.join(&previous),
                                None => walker.state.environment.clone(),
                            });
                        }
                        if let Some(matches) = matches {
                            walker.state.environment = matches;
                        }
                        if falls_through {
                            matched_empty = Some(walker.state.environment.clone());
                            return;
                        }
                        for statement in &case.body {
                            walker.visit_stmt(statement);
                        }
                    });
                    unmatched.pop_scope();
                    if !falls_through && !self.block_exits(&case.body) {
                        ends.push(std::mem::take(&mut self.state.environment));
                    }
                }
                ends.extend(self.break_targets.pop().expect("switch target").states);
                // No case matching reaches the end unless the switch is
                // exhaustive; with no completing path the end is unreachable.
                if !exhaustive {
                    ends.push(unmatched);
                }
                self.join_states(ends, depth, false);
            }
            Stmt::TryCatch(statement) => {
                let before = self.state.environment.clone();
                self.with_scope(|walker| {
                    for statement in &statement.body.stmts {
                        walker.visit_stmt(statement);
                    }
                });
                let mut ends = Vec::with_capacity(statement.catches.len() + 1);
                if !self.block_exits(&statement.body.stmts) {
                    ends.push(std::mem::take(&mut self.state.environment));
                }
                // `finally` may also start anywhere in the `try` or its catches.
                let before_finally = statement.finally.as_ref().map(|_| before.clone());
                // A catch may start anywhere in the `try` body: only promotions
                // that hold before it and survive its writes remain.
                self.state.environment = before;
                self.enter_region(statement.span.start);
                let caught = self.state.environment.clone();
                for catch in &statement.catches {
                    self.state.environment = caught.clone();
                    self.with_scope(|walker| {
                        if let Some(name) = &catch.exception_var {
                            let ty = catch.exception_type.as_ref().map_or_else(
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
                                .declare_at(name.name.clone(), ty, name.span);
                        }
                        if let Some(name) = &catch.stack_trace_var {
                            let ty = walker.state.model.core_type("StackTrace", false);
                            walker
                                .state
                                .environment
                                .declare_at(name.name.clone(), ty, name.span);
                        }
                        for statement in &catch.body.stmts {
                            walker.visit_stmt(statement);
                        }
                    });
                    if !self.block_exits(&catch.body.stmts) {
                        ends.push(std::mem::take(&mut self.state.environment));
                    }
                }
                self.state.environment = ends
                    .into_iter()
                    .reduce(|joined, end| joined.join(&end))
                    .unwrap_or(caught);
                if let (Some(finally), Some(before)) = (&statement.finally, before_finally) {
                    // `finally` runs after the body and catches complete, or
                    // after any of their writes when they leave early.
                    let completed = std::mem::replace(&mut self.state.environment, before);
                    self.enter_region(statement.span.start);
                    for catch in &statement.catches {
                        self.enter_region(catch.body.span.start);
                    }
                    let entry = completed.join(&self.state.environment);
                    self.state.environment = entry.clone();
                    self.with_scope(|walker| {
                        for statement in &finally.stmts {
                            walker.visit_stmt(statement);
                        }
                    });
                    let end = std::mem::replace(&mut self.state.environment, completed);
                    self.state.environment.attach_finally(&entry, &end);
                }
            }
            Stmt::Return(statement) => {
                if let Some(value) = &statement.value {
                    let expected = self
                        .state
                        .return_type
                        .last()
                        .cloned()
                        .unwrap_or(ResolvedType::Unknown);
                    self.with_expected(expected, |walker| walker.visit_expr(value));
                }
            }
            Stmt::Throw(statement) => self.visit_expr(&statement.value),
            Stmt::Labeled(statement) => self.with_break_target(
                Some(&statement.label.name),
                false,
                &statement.stmt,
                |walker| walker.visit_stmt(&statement.stmt),
            ),
            Stmt::LocalFunc(function) => {
                self.visit_declaration_annotations(&function.annotations);
                self.state
                    .type_parameters
                    .push(&function.type_params, &self.state.model);
                let mut inferred_return = ResolvedType::Unknown;
                if function.return_type.is_none() && function.async_marker == AsyncMarker::Sync {
                    self.with_scope(|walker| {
                        walker.bind_params(&function.params, None, false, None);
                        if let FunctionBody::Arrow(expression, _) = &function.body {
                            inferred_return = walker.state.infer_invocation(expression);
                        }
                    });
                }
                let ty = ResolvedType::Function {
                    optional_positional: function
                        .params
                        .positional()
                        .filter(|param| param.kind == ParamKind::OptionalPositional)
                        .count(),
                    alias: Default::default(),
                    return_type: std::sync::Arc::new(
                        function
                            .return_type
                            .as_ref()
                            .map(|ty| {
                                self.state
                                    .model
                                    .resolve_type_in(ty, &self.state.type_parameters)
                            })
                            .unwrap_or(inferred_return),
                    ),
                    positional: function
                        .params
                        .positional()
                        .map(|param| {
                            self.state
                                .model
                                .resolve_formal_parameter_in(param, &self.state.type_parameters)
                        })
                        .collect(),
                    named: function
                        .params
                        .named()
                        .map(|param| {
                            (
                                param.name.name.clone(),
                                self.state.model.resolve_formal_parameter_in(
                                    param,
                                    &self.state.type_parameters,
                                ),
                            )
                        })
                        .collect(),
                    nullable: false,
                };
                self.state.environment.declare_local_function_at(
                    function.name.name.clone(),
                    ty,
                    function.name.span,
                );
                self.state.type_parameters.pop();
                self.visit_function(
                    Callable {
                        nested: Some(function.span.start),
                        type_parameters: &function.type_params,
                        params: Some(&function.params),
                        return_type: function.return_type.as_ref(),
                        body: Some(&function.body),
                        async_marker: function.async_marker,
                    },
                    false,
                );
            }
            Stmt::Assert(statement) => {
                self.visit_expr(&statement.condition);
                if let Some(message) = &statement.message {
                    self.visit_expr(message);
                }
            }
            Stmt::Yield(statement) => {
                // `yield e` in a `Stream<T>`/`Iterable<T>` generator has
                // context `T`.
                let element = self.state.return_type.last().and_then(|ty| {
                    (!statement.is_star
                        && (ty.interface("dart:async", "Stream")
                            || ty.interface("dart:core", "Iterable")))
                    .then(|| ty.type_argument(0))
                    .flatten()
                });
                match element {
                    Some(element) => {
                        self.with_expected(element, |walker| walker.visit_expr(&statement.value));
                    }
                    None => self.visit_expr(&statement.value),
                }
            }
            Stmt::Expr(statement) => self.visit_expr(&statement.expr),
            Stmt::Break(statement) => self.record_break(statement.label.as_ref()),
            Stmt::Continue(_) | Stmt::Rethrow(_) | Stmt::Empty(_) | Stmt::Error(_) => {}
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        self.visitor.visit_expr(node, &self.state);
        match node {
            Expr::Paren { expr, .. } => self.visit_expr(expr),
            Expr::Call {
                callee,
                type_args,
                args,
                ..
            } => {
                // A dot shorthand takes its context from the complete
                // selector chain, not from the final method's return type.
                let expected = self
                    .state
                    .expected()
                    .filter(|expected| {
                        !matches!(expected, ResolvedType::Unknown)
                            && falcon_analyze::expression_has_dot_shorthand_base(callee)
                    })
                    .cloned();
                self.with_context(expected, |walker| walker.visit_expr(callee));
                match self.state.call_parameter_types(
                    callee,
                    type_args,
                    args,
                    self.state.expected(),
                ) {
                    Some((positional, named)) => {
                        self.visit_call_arguments(args, &positional, &named)
                    }
                    None => {
                        let callee_type = self.state.infer_invocation(callee);
                        let untyped_member = match callee.as_ref() {
                            Expr::Field { object, field, .. } => {
                                let receiver = self.state.infer_receiver(object);
                                matches!(receiver, ResolvedType::Dynamic)
                                    || (field.name == "call"
                                        && receiver.interface("dart:core", "Function"))
                            }
                            _ => false,
                        };
                        let expected = if matches!(callee_type, ResolvedType::Dynamic)
                            || callee_type.interface("dart:core", "Function")
                            || untyped_member
                        {
                            ResolvedType::Dynamic
                        } else {
                            ResolvedType::Unknown
                        };
                        for argument in &args.positional {
                            self.with_expected(expected.clone(), |walker| {
                                walker.visit_expr(argument)
                            });
                        }
                        for argument in &args.named {
                            self.with_expected(expected.clone(), |walker| {
                                walker.visit_expr(&argument.value)
                            });
                        }
                    }
                }
            }
            Expr::Field { object, .. } => {
                let expected = self
                    .state
                    .expected()
                    .filter(|expected| {
                        !matches!(expected, ResolvedType::Unknown)
                            && falcon_analyze::expression_has_dot_shorthand_base(object)
                    })
                    .cloned();
                self.with_context(expected, |walker| walker.visit_expr(object));
            }
            Expr::New {
                dart_type,
                constructor_name,
                args,
                ..
            } => {
                let constructor = falcon_analyze::resolve_new_constructor(
                    dart_type,
                    constructor_name.as_ref(),
                    &self.state.model,
                );
                let resolved = self.state.infer_with_context(node, self.state.expected());
                let parameters = self.state.constructor_parameter_types(
                    &resolved,
                    constructor.as_ref().map_or("new", |(_, name)| *name),
                );
                match parameters {
                    Some((positional, named)) => {
                        self.visit_call_arguments(args, &positional, &named)
                    }
                    None => self.visit_call_arguments(args, &[], &HashMap::new()),
                }
            }
            Expr::FuncExpr {
                type_params,
                params,
                body,
                async_marker,
                span,
                ..
            } => {
                let callable = Callable {
                    nested: Some(span.start),
                    type_parameters: type_params,
                    params: Some(params),
                    return_type: None,
                    body: Some(body),
                    async_marker: *async_marker,
                };
                let inferred = self.state.infer_with_context(node, self.state.expected());
                if let ResolvedType::Function {
                    return_type,
                    positional,
                    named,
                    nullable,
                    alias,
                    optional_positional,
                } = inferred
                {
                    self.with_expected(
                        ResolvedType::Function {
                            optional_positional,
                            return_type,
                            positional,
                            named,
                            nullable,
                            alias,
                        },
                        |walker| walker.visit_function(callable, true),
                    );
                } else {
                    self.visit_function(callable, true);
                }
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                self.without_expected(|walker| walker.visit_expr(condition));
                self.visit_branches(
                    condition,
                    |walker| walker.visit_expr(then_expr),
                    |walker| walker.visit_expr(else_expr),
                    matches!(then_expr.as_ref(), Expr::Throw { .. }),
                    matches!(else_expr.as_ref(), Expr::Throw { .. }),
                );
            }
            Expr::Binary {
                op: BinaryOp::And | BinaryOp::Or,
                left,
                right,
                ..
            } => {
                self.with_expected(ResolvedType::Unknown, |walker| walker.visit_expr(left));
                let before = self.state.environment.clone();
                self.assume_condition(
                    left,
                    matches!(
                        node,
                        Expr::Binary {
                            op: BinaryOp::And,
                            ..
                        }
                    ),
                );
                self.with_expected(ResolvedType::Unknown, |walker| walker.visit_expr(right));
                self.state.environment = before.join(&self.state.environment);
            }
            Expr::Binary {
                op: BinaryOp::NullCoalesce,
                left,
                right,
                ..
            } => {
                self.with_expected(ResolvedType::Unknown, |walker| walker.visit_expr(left));
                let before = self.state.environment.clone();
                let expected = self
                    .state
                    .expected()
                    .filter(|ty| !matches!(ty, ResolvedType::Unknown | ResolvedType::Dynamic))
                    .cloned()
                    .unwrap_or_else(|| self.state.infer(left));
                self.with_expected(expected, |walker| walker.visit_expr(right));
                self.state.environment = before.join(&self.state.environment);
            }
            Expr::Binary {
                op, left, right, ..
            } => {
                // A binary expression's result context does not flow into
                // its receiver; the right operand uses the operator parameter.
                self.without_expected(|walker| walker.visit_expr(left));
                let name = op
                    .method_name()
                    .expect("logical and coalescing operators are visited separately");
                let expected = self
                    .state
                    .signatures
                    .resolved_member(&self.state.infer(left), name, &self.state.model)
                    .and_then(|(signature, owner)| {
                        signature
                            .positional
                            .first()
                            .map(|ty| self.state.model.substitute(ty, &owner))
                    })
                    .unwrap_or(ResolvedType::Unknown);
                self.with_expected(expected, |walker| walker.visit_expr(right));
            }
            Expr::Assign {
                target,
                op: AssignOp::NullCoalesceEq,
                value,
                ..
            } => {
                self.visit_expr(target);
                let before = self.state.environment.clone();
                let expected = self.state.writable_type(target);
                self.with_expected(expected, |walker| walker.visit_expr(value));
                self.state.environment = before.join(&self.state.environment);
            }
            Expr::Cascade {
                object,
                sections,
                is_null_aware,
                ..
            } => {
                self.visit_expr(object);
                let receiver = self.state.infer_with_context(object, self.state.expected());
                let receiver = if *is_null_aware {
                    receiver.non_null()
                } else {
                    receiver
                };
                for section in sections {
                    let mut current = receiver.clone();
                    for operation in &section.ops {
                        if matches!(
                            operation,
                            CascadeOp::Field {
                                is_null_aware: true,
                                ..
                            } | CascadeOp::Index {
                                is_null_aware: true,
                                ..
                            }
                        ) {
                            current = current.non_null();
                        }
                        self.visitor.visit_cascade_operation(
                            operation,
                            &current,
                            section.span,
                            &self.state,
                        );
                        match operation {
                            CascadeOp::NullAssert { .. } => {
                                current = current.non_null();
                            }
                            CascadeOp::Field { name, .. } => {
                                current = self
                                    .state
                                    .signatures
                                    .instantiated_member_read(
                                        &current,
                                        &name.name,
                                        &self.state.model,
                                    )
                                    .unwrap_or_else(|| {
                                        if current == ResolvedType::Dynamic {
                                            ResolvedType::Dynamic
                                        } else {
                                            ResolvedType::Unknown
                                        }
                                    });
                            }
                            CascadeOp::Call(name, type_args, args) => {
                                if let Some((signature, owner)) = self
                                    .state
                                    .signatures
                                    .resolved_member(&current, &name.name, &self.state.model)
                                {
                                    let substitutions = self.state.signature_substitutions(
                                        signature, &owner, type_args, args, None,
                                    );
                                    let (positional, named) = self
                                        .state
                                        .substituted_parameters(signature, &substitutions);
                                    let returned = self
                                        .state
                                        .model
                                        .substitute(&signature.return_type, &substitutions);
                                    self.visit_call_arguments(args, &positional, &named);
                                    current = returned;
                                } else {
                                    for argument in &args.positional {
                                        self.with_expected(ResolvedType::Unknown, |walker| {
                                            walker.visit_expr(argument)
                                        });
                                    }
                                    for argument in &args.named {
                                        self.with_expected(ResolvedType::Unknown, |walker| {
                                            walker.visit_expr(&argument.value)
                                        });
                                    }
                                    current = self
                                        .state
                                        .signatures
                                        .instantiated_member_read(
                                            &current,
                                            &name.name,
                                            &self.state.model,
                                        )
                                        .as_ref()
                                        .map(SemanticState::callable_return_type)
                                        .unwrap_or_else(|| {
                                            if current == ResolvedType::Dynamic {
                                                ResolvedType::Dynamic
                                            } else {
                                                ResolvedType::Unknown
                                            }
                                        });
                                }
                                self.state.environment.demote_captured();
                            }
                            CascadeOp::Invoke(type_args, args) => {
                                if let Some((signature, owner)) = self
                                    .state
                                    .signatures
                                    .resolved_member(&current, "call", &self.state.model)
                                {
                                    let substitutions = self.state.signature_substitutions(
                                        signature, &owner, type_args, args, None,
                                    );
                                    let (positional, named) = self
                                        .state
                                        .substituted_parameters(signature, &substitutions);
                                    let returned = self
                                        .state
                                        .model
                                        .substitute(&signature.return_type, &substitutions);
                                    self.visit_call_arguments(args, &positional, &named);
                                    current = returned;
                                } else {
                                    let returned = current.function_return_type();
                                    if let ResolvedType::Function {
                                        positional, named, ..
                                    } = current
                                    {
                                        self.visit_call_arguments(
                                            args,
                                            &positional,
                                            &named.iter().cloned().collect(),
                                        );
                                        current = returned.unwrap_or(ResolvedType::Unknown);
                                    } else {
                                        self.visit_call_arguments(args, &[], &HashMap::new());
                                        current = SemanticState::callable_return_type(&current);
                                    }
                                }
                                self.state.environment.demote_captured();
                            }
                            CascadeOp::Index { index, .. } => {
                                if let Some((signature, owner)) = self
                                    .state
                                    .signatures
                                    .resolved_member(&current, "[]", &self.state.model)
                                {
                                    let expected = signature
                                        .positional
                                        .first()
                                        .map(|ty| self.state.model.substitute(ty, &owner))
                                        .unwrap_or(ResolvedType::Unknown);
                                    let returned =
                                        self.state.model.substitute(&signature.return_type, &owner);
                                    self.with_expected(expected, |walker| walker.visit_expr(index));
                                    current = returned;
                                } else {
                                    self.with_expected(ResolvedType::Unknown, |walker| {
                                        walker.visit_expr(index)
                                    });
                                    if current != ResolvedType::Dynamic {
                                        current = ResolvedType::Unknown;
                                    }
                                }
                            }
                            CascadeOp::Assign(target, operator, value)
                            | CascadeOp::IndexAssign(target, operator, value) => {
                                let indexed = matches!(operation, CascadeOp::IndexAssign(..));
                                self.visitor.visit_cascade_assignment(
                                    CascadeAssignment {
                                        target,
                                        value,
                                        operator: *operator,
                                        indexed,
                                    },
                                    &current,
                                    section.span,
                                    &self.state,
                                );
                                let (index_context, value_context) = if indexed {
                                    self.state
                                        .signatures
                                        .resolved_member(&current, "[]=", &self.state.model)
                                        .map(|(signature, owner)| {
                                            let resolve = |position| {
                                                signature
                                                    .positional
                                                    .get(position)
                                                    .map(|ty| {
                                                        self.state.model.substitute(ty, &owner)
                                                    })
                                                    .unwrap_or(ResolvedType::Unknown)
                                            };
                                            (resolve(0), resolve(1))
                                        })
                                        .unwrap_or((ResolvedType::Unknown, ResolvedType::Unknown))
                                } else if let Expr::Ident(identifier) = target.as_ref() {
                                    let expected = self
                                        .state
                                        .signatures
                                        .resolved_writable_field(
                                            &current,
                                            &identifier.name,
                                            &self.state.model,
                                        )
                                        .map(|(ty, owner)| self.state.model.substitute(&ty, &owner))
                                        .unwrap_or(ResolvedType::Unknown);
                                    (ResolvedType::Unknown, expected)
                                } else {
                                    (ResolvedType::Unknown, ResolvedType::Unknown)
                                };
                                self.with_expected(index_context, |walker| {
                                    walker.visit_expr(target)
                                });
                                self.with_expected(value_context, |walker| {
                                    walker.visit_expr(value)
                                });
                            }
                        }
                    }
                }
            }
            Expr::Assign {
                target,
                op: AssignOp::Eq,
                value,
                ..
            } => {
                self.visit_expr(target);
                let expected = self.state.writable_type(target);
                self.with_expected(expected, |walker| walker.visit_expr(value));
            }
            Expr::List {
                type_arg, elements, ..
            }
            | Expr::Set {
                type_arg, elements, ..
            } => {
                let context = self.element_context(type_arg.as_ref());
                for element in elements {
                    self.visit_collection_element(element, &context);
                }
            }
            Expr::Map {
                type_args,
                elements,
                ..
            } => {
                let component_type = |ordinal| {
                    type_args
                        .get(ordinal)
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
                                        .unwrap_or(ResolvedType::Unknown)
                                } else {
                                    ResolvedType::Unknown
                                }
                            })
                        })
                };
                let key_type = component_type(0);
                let value_type = component_type(1);
                for element in elements {
                    self.visit_map_element(element, &key_type, &value_type);
                }
            }
            // A record literal's fields take the context record's field types.
            Expr::Record { fields, .. } => {
                if let Some(expected @ ResolvedType::Record { .. }) = self.state.expected().cloned()
                {
                    let mut positional_index = 0;
                    for field in fields {
                        let expected = match &field.name {
                            Some(name) => expected.record_field(&name.name),
                            None => {
                                let ty = expected.record_positional(positional_index);
                                positional_index += 1;
                                ty
                            }
                        };
                        self.with_expected(expected.unwrap_or(ResolvedType::Unknown), |walker| {
                            walker.visit_expr(&field.value)
                        });
                    }
                } else {
                    self.with_expected(ResolvedType::Unknown, |walker| walk_expr(walker, node));
                }
            }
            Expr::Switch { subject, arms, .. } => {
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
                            &arm.pattern,
                            arm.guard.as_ref(),
                            matched.clone(),
                        );
                        walker.visit_expr(&arm.body);
                        completes = walker.state.infer(&arm.body) != ResolvedType::Never;
                    });
                    unmatched.pop_scope();
                    if completes {
                        ends.push(std::mem::take(&mut self.state.environment));
                    }
                }
                // A switch expression is exhaustive; with no completing arm,
                // control never reaches its end.
                self.state.environment = before;
                self.join_states(ends, depth, false);
            }
            Expr::NullAssert { operand, .. } => {
                let expected = self
                    .state
                    .expected()
                    .map(|ty| ty.with_nullable(true))
                    .unwrap_or(ResolvedType::Unknown);
                self.with_expected(expected, |walker| walker.visit_expr(operand));
            }
            _ => self.with_expected(ResolvedType::Unknown, |walker| walk_expr(walker, node)),
        }
        let assigned = match node {
            Expr::Assign {
                target, op, value, ..
            } if matches!(target.as_ref(), Expr::Ident(_)) => {
                // The write must use the same declared context as the RHS
                // visit; otherwise a generic constructor can lose its type
                // arguments and incorrectly discard a non-null promotion.
                if matches!(op, AssignOp::Eq | AssignOp::NullCoalesceEq) {
                    let expected = self.state.writable_type(target);
                    let inferred = self.state.infer_with_context(value, Some(&expected));
                    Some(if inferred == ResolvedType::Unknown {
                        self.state.infer_invocation(value)
                    } else {
                        inferred
                    })
                } else {
                    Some(self.state.infer_invocation(value))
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

    fn visit_pattern(&mut self, node: &Pattern) {
        self.visitor
            .visit_pattern(node, self.state.matched.last(), &self.state);
        walk_pattern(self, node);
    }
}

pub(crate) fn returned_expressions(body: &FunctionBody) -> Vec<&Expr> {
    fn collect<'a>(statements: &'a [Stmt], expressions: &mut Vec<&'a Expr>) {
        for statement in statements {
            collect_statement(statement, expressions);
        }
    }

    fn collect_statement<'a>(statement: &'a Stmt, expressions: &mut Vec<&'a Expr>) {
        match statement {
            Stmt::Return(ReturnStmt {
                value: Some(value), ..
            }) => expressions.push(value),
            Stmt::Block(block) => collect(&block.stmts, expressions),
            Stmt::If(node) => {
                collect_statement(&node.then_branch, expressions);
                if let Some(branch) = &node.else_branch {
                    collect_statement(branch, expressions);
                }
            }
            Stmt::For(node) => collect_statement(&node.body, expressions),
            Stmt::While(node) => collect_statement(&node.body, expressions),
            Stmt::DoWhile(node) => collect_statement(&node.body, expressions),
            Stmt::Switch(node) => {
                for case in &node.cases {
                    collect(&case.body, expressions);
                }
            }
            Stmt::TryCatch(node) => {
                collect(&node.body.stmts, expressions);
                for catch in &node.catches {
                    collect(&catch.body.stmts, expressions);
                }
                if let Some(block) = &node.finally {
                    collect(&block.stmts, expressions);
                }
            }
            Stmt::Labeled(node) => collect_statement(&node.stmt, expressions),
            // Expressions contain statements only inside excluded function expressions.
            Stmt::LocalFunc(_)
            | Stmt::Return(_)
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
        FunctionBody::Block(block) => collect(&block.stmts, &mut expressions),
        FunctionBody::Arrow(expression, _) => expressions.push(expression.as_ref()),
        FunctionBody::Native(_, _) => {}
    }
    expressions
}

fn collect_type_parameter_ids(
    ty: &ResolvedType,
    by_owner: &mut HashMap<ParameterOwner, Vec<TypeParameterId>>,
) {
    match ty {
        ResolvedType::TypeParameter { id, bound, .. }
        | ResolvedType::NonNullTypeParameter { id, bound } => {
            let parameters = by_owner.entry(id.owner).or_default();
            if !parameters.contains(id) {
                parameters.push(id.clone());
            }
            collect_type_parameter_ids(bound, by_owner);
        }
        ResolvedType::Interface { arguments, .. } => {
            for argument in arguments.iter() {
                collect_type_parameter_ids(argument, by_owner);
            }
        }
        ResolvedType::Function {
            return_type,
            positional,
            named,
            ..
        } => {
            collect_type_parameter_ids(return_type, by_owner);
            for parameter in positional.iter() {
                collect_type_parameter_ids(parameter, by_owner);
            }
            for (_, parameter) in named.iter() {
                collect_type_parameter_ids(parameter, by_owner);
            }
        }
        ResolvedType::Record {
            positional, named, ..
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

#[cfg(test)]
mod tests {
    use falcon_dart_parser::parse;

    use super::*;

    #[test]
    fn inherited_implicit_generic_method_preserves_call_parameters() {
        use falcon_analyze::{IdentityIndex, IdentitySource, TypeIndex};
        use falcon_config::FalconConfig;
        use std::path::PathBuf;

        let source = r#"
class Token {}
class Base { T identity<T>(T value, {bool flag = false}) => value; }
class Child extends Base {
  void probe(Token value) {
    identity<Token>(value);
    identity(value);
  }
}
class Other {}
extension OtherMethods on Other {
  T identity<T>(T value, {bool flag = false}) => value;
  void probe(Token value) {
    identity<Token>(value);
    identity(value);
  }
}
"#;
        let path = PathBuf::from("/project/lib/main.dart");
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let program = parsed.program;
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_resolved_files(
            [(
                path.as_path(),
                falcon_analyze::LibrarySource {
                    program: &program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        );
        let signatures =
            SignatureIndex::from_project_files(&[(path.clone(), &program)], &identities, &types);
        let config = FalconConfig::default();
        let context = AnalyzeContext::new(&path, source, &config, &parsed.syntax)
            .with_identities(&identities)
            .with_types(&types)
            .with_signatures(&signatures);

        #[derive(Default)]
        struct Observed {
            calls: usize,
        }
        impl SemanticRuleVisitor for Observed {
            fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
                let Expr::Call {
                    callee,
                    type_args,
                    args,
                    ..
                } = node
                else {
                    return;
                };
                if !matches!(callee.as_ref(), Expr::Ident(name) if name.name == "identity") {
                    return;
                }
                let token =
                    ResolvedType::interface_identity(state.model.resolve_name("Token").unwrap());
                let target = state.call_target(callee, type_args).unwrap();
                assert_eq!(target.signature.call_parameters.len(), 1);
                assert!(target.signature.defaults.contains_key("flag"));
                let signature = state.call_signature(callee, type_args, args, None).unwrap();
                assert_eq!(
                    signature.positional.as_slice(),
                    std::slice::from_ref(&token)
                );
                assert_eq!(signature.return_type, token);
                assert_eq!(state.infer_invocation(node), token);
                self.calls += 1;
            }
        }
        let mut observed = Observed::default();
        visit_program(
            &mut observed,
            &program,
            SemanticState::new(&program, &context).unwrap(),
        );
        assert_eq!(observed.calls, 4);
    }

    #[test]
    fn nullsafe_accesses_preserve_call_and_assignment_contexts_without_an_sdk() {
        use falcon_analyze::{IdentityIndex, IdentitySource, TypeIndex};
        use falcon_config::FalconConfig;
        use std::path::PathBuf;

        let source = r#"
class Value { Value echo() => this; }
class Box {
  Value item;
  Value? maybe;
  Value take(Value value) => value;
  T generic<T>(T value) => value;
  Value operator [](int index) => item;
  void operator []=(int index, Value value) {}
}
class Parent { Box? box; }
class Child extends Parent {
  void check(Value value) {
    box?.take(value);
    super.box?.take(value);
    super.box?.item.echo();
    super.box?.take(value).echo();
    super.box?.generic<Value>.call(value).echo();
    super.box?.item;
    super.box?[0];
    super.box?.item = value;
    super.box?.maybe = value;
    super.box?[0] = value;
  }
}
void check(Box? box, Value value) {
  box?.take(value);
  box?.item = value;
  box?.maybe = value;
  box?[0] = value;
}
void records((Value,)? positional, ({Value slot})? named, (Value?,)? maybe) {
  positional?.$1.echo();
  named?.slot.echo();
  maybe?.$1;
}
"#;
        let path = PathBuf::from("/project/lib/main.dart");
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_resolved_files(
            [(
                path.as_path(),
                falcon_analyze::LibrarySource {
                    program: &program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        );
        let signatures =
            SignatureIndex::from_project_files(&[(path.clone(), &program)], &identities, &types);
        let config = FalconConfig::default();
        let context = AnalyzeContext::new(&path, source, &config, &syntax)
            .with_identities(&identities)
            .with_types(&types)
            .with_signatures(&signatures);

        fn assert_value(state: &SemanticState<'_>, ty: &ResolvedType, nullable: bool) {
            assert!(
                matches!(ty, ResolvedType::Interface { identity, .. }
                    if Some(identity.clone()) == state.model.resolve_name("Value")),
                "{ty:?}"
            );
            assert_eq!(ty.nullable(), nullable, "{ty:?}");
        }

        #[derive(Default)]
        struct Observed {
            calls: usize,
            assignments: usize,
            reads: usize,
            echoes: usize,
            record_fields: usize,
        }
        impl SemanticRuleVisitor for Observed {
            fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
                match node {
                    Expr::Call {
                        callee,
                        type_args,
                        args,
                        ..
                    } if matches!(callee.as_ref(), Expr::Field { field, .. } if field.name == "echo") =>
                    {
                        let Expr::Field { object, .. } = callee.as_ref() else {
                            unreachable!()
                        };
                        assert_value(
                            state,
                            &state.infer_access_receiver(object, "echo", false),
                            false,
                        );
                        let signature =
                            state.call_signature(callee, type_args, args, None).unwrap();
                        assert!(signature.positional.is_empty());
                        assert_value(state, &state.infer_with_context(node, None), true);
                        assert_value(state, &state.infer_invocation(node), true);
                        self.echoes += 1;
                    }
                    Expr::Call {
                        callee,
                        type_args,
                        args,
                        ..
                    } if matches!(callee.as_ref(), Expr::Field { field, .. } if field.name == "take") =>
                    {
                        let signature =
                            state.call_signature(callee, type_args, args, None).unwrap();
                        let (positional, named) = state
                            .call_parameter_types(callee, type_args, args, None)
                            .unwrap();
                        assert!(named.is_empty());
                        assert_eq!(positional.len(), 1);
                        assert_value(state, &positional[0], false);
                        assert_value(state, &signature.positional[0], false);
                        assert_value(state, &state.infer_invocation(node), true);
                        self.calls += 1;
                    }
                    Expr::Assign { target, .. } => {
                        let nullable = matches!(target.as_ref(), Expr::Field { field, .. } if field.name == "maybe");
                        assert_value(state, &state.writable_type(target), nullable);
                        self.assignments += 1;
                    }
                    Expr::Field {
                        object,
                        field,
                        is_null_safe: true,
                        ..
                    } if field.name == "$1" || field.name == "slot" => {
                        let nullable = matches!(object.as_ref(), Expr::Ident(identifier) if identifier.name == "maybe");
                        assert_value(
                            state,
                            &state.infer_access_receiver(node, "", false),
                            nullable,
                        );
                        assert_value(state, &state.infer_invocation(node), true);
                        self.record_fields += 1;
                    }
                    Expr::Field {
                        field,
                        is_null_safe: true,
                        ..
                    } if field.name == "item" || field.name == "maybe" => {
                        assert_value(state, &state.infer_invocation(node), true);
                        self.reads += 1;
                    }
                    Expr::Index {
                        is_null_safe: true, ..
                    } => {
                        assert_value(state, &state.infer_invocation(node), true);
                        self.reads += 1;
                    }
                    _ => {}
                }
            }
        }
        let mut observed = Observed::default();
        visit_program(
            &mut observed,
            &program,
            SemanticState::new(&program, &context).unwrap(),
        );
        assert_eq!(observed.calls, 4);
        assert_eq!(observed.assignments, 6);
        assert_eq!(observed.reads, 9);
        assert_eq!(observed.echoes, 5);
        assert_eq!(observed.record_fields, 3);
    }

    #[test]
    fn legacy_switch_case_expressions_reach_semantic_visitors() {
        use falcon_analyze::{IdentityIndex, IdentitySource};
        use falcon_config::FalconConfig;
        use std::path::PathBuf;

        let source = "// @dart=2.19\nvoid f(int x) { switch (x) { case 7: break; } }";
        let path = PathBuf::from("/project/lib/main.dart");
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let config = FalconConfig::default();
        let context =
            AnalyzeContext::new(&path, source, &config, &syntax).with_identities(&identities);
        #[derive(Default)]
        struct CaseOperand(usize);
        impl SemanticRuleVisitor for CaseOperand {
            fn visit_expr(&mut self, node: &Expr, _state: &SemanticState<'_>) {
                if matches!(node, Expr::IntLit { value, .. } if value == "7") {
                    self.0 += 1;
                }
            }
        }
        let mut operand = CaseOperand::default();
        visit_program(
            &mut operand,
            &program,
            SemanticState::new(&program, &context).unwrap(),
        );
        assert_eq!(operand.0, 1);
    }

    #[test]
    fn primary_parameters_and_locals_stay_in_constructor_scopes() {
        use falcon_analyze::{IdentityIndex, IdentitySource, TypeIndex};
        use falcon_config::FalconConfig;
        use std::path::PathBuf;

        // Analyzer ConstructorInitializerScope includes primary parameters in
        // non-late instance initializers, but not in methods or late/static fields.
        let source = r#"
bool Future = true;
class C(final int? _x, var int? y, int Future) {
  final x = Future;
  late bool delayed = Future;
  static bool shared = Future;
  bool get f => true;
  this { final f = 1; Future; if (y != null) y; }
  void later() { f; Future; }
  int promote() => _x != null ? _x.abs() : 0;
}
extension type E(int? y) {
  this { if (y != null) y; }
}
"#;
        let path = PathBuf::from("/project/lib/main.dart");
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_resolved_files(
            [(
                path.as_path(),
                falcon_analyze::LibrarySource {
                    program: &program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        );
        let signatures =
            SignatureIndex::from_project_files(&[(path.clone(), &program)], &identities, &types);
        let config = FalconConfig::default();
        let context = AnalyzeContext::new(&path, source, &config, &syntax)
            .with_identities(&identities)
            .with_types(&types)
            .with_signatures(&signatures);
        #[derive(Default)]
        struct Observed {
            future_types: Vec<ResolvedType>,
            member_types: Vec<ResolvedType>,
            promoted_field: bool,
            body_field_types: Vec<ResolvedType>,
        }
        impl SemanticRuleVisitor for Observed {
            fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
                if let Expr::Ident(identifier) = node {
                    let ty = state.infer(node);
                    match identifier.name.as_str() {
                        "Future" => self.future_types.push(ty),
                        "f" => self.member_types.push(ty),
                        "y" => self.body_field_types.push(ty),
                        "_x" => {
                            self.promoted_field |=
                                ty.interface("dart:core", "int") && !ty.nullable();
                        }
                        _ => {}
                    }
                }
            }
        }
        let mut observed = Observed::default();
        visit_program(
            &mut observed,
            &program,
            SemanticState::new(&program, &context).unwrap(),
        );
        assert_eq!(observed.future_types.len(), 5);
        for (ty, name) in observed
            .future_types
            .iter()
            .zip(["int", "bool", "bool", "int", "bool"])
        {
            assert!(ty.interface("dart:core", name), "{ty:?}");
        }
        assert_eq!(observed.member_types.len(), 1);
        assert!(observed.member_types[0].interface("dart:core", "bool"));
        assert!(observed.promoted_field);
        assert_eq!(observed.body_field_types.len(), 4);
        assert!(
            observed
                .body_field_types
                .iter()
                .all(|ty| { ty.interface("dart:core", "int") && ty.nullable() })
        );
    }

    #[test]
    fn binary_operands_use_operator_parameters_not_the_result_context() {
        use falcon_analyze::{AnalyzeContext, IdentityIndex, IdentitySource, Rule, TypeIndex};
        use falcon_config::FalconConfig;
        use std::path::PathBuf;

        let source = r#"
T pick<T extends num>(T value) => value;
class Numeric {
  double operator +(num value) => 0;
  double operator -(double value) => 0;
}
void run(Numeric number) {
  double broad = number + pick(1.0);
  double narrow = number - pick(2.0);
  double direct = pick(3.0);
}
"#;
        let path = PathBuf::from("/project/lib/main.dart");
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_resolved_files(
            [(
                path.as_path(),
                falcon_analyze::LibrarySource {
                    program: &program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        );
        let signatures =
            SignatureIndex::from_project_files(&[(path.clone(), &program)], &identities, &types);
        let config = FalconConfig::default();
        let context = AnalyzeContext::new(&path, source, &config, &syntax)
            .with_identities(&identities)
            .with_types(&types)
            .with_signatures(&signatures);
        let diagnostics =
            crate::lint::style::prefer_int_literals::PreferIntLiterals.analyze(&program, &context);
        let reported: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(reported, ["2.0", "3.0"]);
    }

    #[test]
    fn assignment_patterns_and_nullable_cascades_supply_precise_contexts() {
        use falcon_analyze::{AnalyzeContext, IdentityIndex, IdentitySource, TypeIndex};
        use falcon_config::FalconConfig;
        use std::path::PathBuf;

        let source = r#"
void f(dynamic d, dynamic other, void value) {
  if (d is! String) return;
  int a = 0;
  (d, a) = (other, a);
  d.length;
  [value] = [1];
  [value] = <dynamic>[1];
}
class C {}
extension Nullable on C? { void method() {} }
void cascades(C? value) {
  value?..method();
  value..method();
}
"#;
        let path = PathBuf::from("/project/lib/main.dart");
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_resolved_files(
            [(
                path.as_path(),
                falcon_analyze::LibrarySource {
                    program: &program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        );
        let signatures =
            SignatureIndex::from_project_files(&[(path.clone(), &program)], &identities, &types);
        let config = FalconConfig::default();
        let context = AnalyzeContext::new(&path, source, &config, &syntax)
            .with_identities(&identities)
            .with_types(&types)
            .with_signatures(&signatures);
        #[derive(Default)]
        struct Observed {
            promoted: bool,
            elements: Vec<ResolvedType>,
            cascade_nullable: Vec<bool>,
        }
        impl SemanticRuleVisitor for Observed {
            fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
                if let Expr::Field { object, field, .. } = node
                    && field.name == "length"
                {
                    self.promoted = state.infer(object).interface("dart:core", "String");
                }
                if let Expr::List { type_arg, .. } = node {
                    // Observe the schema directly without requiring SDK
                    // List-to-Iterable inheritance in this scope-only test.
                    let ty = if type_arg.is_some() {
                        state.infer_with_context(node, state.expected())
                    } else {
                        state.expected().cloned().unwrap_or(ResolvedType::Unknown)
                    };
                    self.elements
                        .push(ty.type_argument(0).unwrap_or(ResolvedType::Unknown));
                }
            }
            fn visit_cascade_operation(
                &mut self,
                _operation: &CascadeOp,
                receiver: &ResolvedType,
                _section: Span,
                _state: &SemanticState<'_>,
            ) {
                self.cascade_nullable.push(receiver.nullable());
            }
        }
        let mut observed = Observed::default();
        visit_program(
            &mut observed,
            &program,
            SemanticState::new(&program, &context).unwrap(),
        );
        assert!(observed.promoted);
        assert_eq!(
            observed.elements,
            [ResolvedType::Void, ResolvedType::Dynamic]
        );
        assert_eq!(observed.cascade_nullable, [false, true]);
    }

    #[test]
    fn finds_nested_returns_without_crossing_function_boundaries() {
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(
            "void f() {
                if (true) { return 1; } else { return 2; }
                for (;;) { return 3; }
                while (true) { return 4; }
                do { return 5; } while (true);
                switch (1) { case 1: return 6; default: return 7; }
                try { return 8; } catch (e) { return 9; } finally { return 10; }
                label: { return 11; }
                void local() { return 12; }
                () { return 13; };
                return () { return 14; };
            }",
        );
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Function(function) = &program.declarations[0] else {
            panic!()
        };
        let returns = returned_expressions(function.body.as_ref().unwrap());
        assert_eq!(returns.len(), 12);
        for (expression, expected) in returns
            .iter()
            .zip(["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"])
        {
            assert!(matches!(expression, Expr::IntLit { value, .. } if value == expected));
        }
        assert!(matches!(returns[11], Expr::FuncExpr { .. }));
    }

    #[test]
    fn returns_borrowed_block_and_arrow_expressions() {
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse("int block() { return 1; } int arrow() => 2;");
        assert!(errors.is_empty(), "{errors:?}");
        for declaration in &program.declarations {
            let TopLevelDecl::Function(function) = declaration else {
                panic!()
            };
            let body = function.body.as_ref().unwrap();
            let expression = match body {
                FunctionBody::Block(block) => {
                    let Stmt::Return(ReturnStmt {
                        value: Some(expression),
                        ..
                    }) = &block.stmts[0]
                    else {
                        panic!()
                    };
                    expression
                }
                FunctionBody::Arrow(expression, _) => expression.as_ref(),
                FunctionBody::Native(_, _) => panic!(),
            };
            let returns = returned_expressions(body);
            assert_eq!(returns.len(), 1);
            assert!(std::ptr::eq(returns[0], expression));
        }
    }
}
