//! Shared leak detection for `cancel-subscriptions` and `close-sinks`, after
//! the official linter's `LeakDetectorProcessors`.
//!
//! Locals are checked in their enclosing function body and fields (including
//! declaring primary-constructor parameters) in the compilation unit. Lexical
//! declaration spans and resolved member owners keep shadowing, inherited fields,
//! and mixin constraint fields tied to the resource, including qualified static writes.
//! Dynamic property and index write escapes require a dynamic receiver, not a
//! missing index entry;
//! non-`this` field writes require the exact declaring owner's identity.
//! Only method invocations (not constructor arguments), release-method tear-offs,
//! field-target cascades, local returns, and the official assignment/constructor
//! escape cases suppress a diagnostic.

use std::collections::HashSet;

use falcon_analyze::{
    AnalyzeContext, ResolvedType, SemanticModel, Truth, resolve_call_constructor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{
    self as visitor, Visitor, walk_expr, walk_formal_param, walk_program, walk_stmt,
};
use falcon_syntax::cst::*;

use falcon_analyze::FunctionBodies;
use falcon_analyze::{CascadeAssignment, SemanticRuleVisitor, SemanticState};

/// A resource type and the method that releases it.
pub(crate) struct Release {
    pub(crate) library: &'static str,
    pub(crate) type_name: &'static str,
    pub(crate) method: &'static str,
}

#[derive(Clone, Copy)]
pub(crate) struct LeakRule {
    pub(crate) message: &'static str,
    pub(crate) releases: &'static [Release],
}

impl LeakRule {
    pub(crate) fn semantic_visitor<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> Option<Box<dyn SemanticRuleVisitor<'a> + 'a>> {
        let model = SemanticModel::new(
            ctx.file_path(),
            ctx.parsed(),
            ctx.identities()?,
            ctx.types(),
        );
        Some(Box::new(LocalCandidates {
            rule: *self,
            program,
            ctx,
            bodies: ctx.function_bodies(program),
            references: References::new(program, ctx.source().len(), &model),
            candidates: Vec::new(),
            excluded_calls: Vec::new(),
        }))
    }

    /// The release methods that apply to a variable of type `ty`.
    fn methods(&self, ty: &ResolvedType, state: &SemanticState<'_>) -> Vec<&'static str> {
        // Share the analyzer's bound/extension-representation semantics.
        self.releases
            .iter()
            .filter(|release| {
                state.signatures.interface_check_sdk(
                    ty,
                    release.library,
                    release.type_name,
                    &state.model,
                ) == Truth::Yes
            })
            .map(|release| release.method)
            .collect()
    }

    fn candidate<'a>(
        &self,
        declarator: VarDeclarator<'a>,
        declared: Option<DartType<'a>>,
        kind: VariableKind,
        container: Span,
        state: &SemanticState<'a>,
    ) -> Option<Candidate<'a>> {
        // `var a = b;` only aliases a resource tracked through `b`.
        if matches!(declarator.initializer(), Some(Expr::Ident(_))) {
            return None;
        }
        let ty = match (declared, declarator.initializer()) {
            (Some(declared), _) => state
                .model
                .resolve_type_in(declared, &state.type_parameters),
            (None, Some(Expr::Cascade(cascade))) => state.infer_receiver(cascade.object()),
            (None, Some(initializer)) => state.infer_receiver(initializer),
            (None, None) => return None,
        };
        let methods = self.methods(&ty, state);
        (!methods.is_empty()).then(|| Candidate {
            name: declarator.name().name(),
            declarator: declarator.span(),
            container,
            kind,
            methods,
            valid: false,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VariableKind {
    Local,
    Field,
}

struct Candidate<'a> {
    name: &'a str,
    declarator: Span,
    container: Span,
    kind: VariableKind,
    methods: Vec<&'static str>,
    valid: bool,
}

impl Candidate<'_> {
    /// `name`, `name!`, or `this.name`.
    fn is_reference(&self, expr: Expr<'_>, references: &References<'_>) -> bool {
        match expr {
            Expr::Ident(_) => self.is_name(expr, references),
            Expr::NullAssert(assertion) => self.is_name(assertion.operand(), references),
            Expr::Field(access) if matches!(access.object(), Expr::This(_)) => {
                let field = access.field();
                references.resolve(field.name(), &field.span(), true) == Some(self.declarator)
            }
            _ => false,
        }
    }

    fn is_name(&self, expr: Expr<'_>, references: &References<'_>) -> bool {
        matches!(expr, Expr::Ident(identifier)
            if references.resolve(identifier.name(), &identifier.span(), false) == Some(self.declarator))
    }

    fn passed_in(&self, args: ArgList<'_>, references: &References<'_>) -> bool {
        self.kind == VariableKind::Local
            && args
                .positional()
                .any(|argument| self.is_name(argument, references))
    }

    fn is_valid_use(
        &self,
        node: Expr<'_>,
        references: &References<'_>,
        excluded_calls: &[(usize, usize)],
    ) -> bool {
        match node {
            Expr::Assign(assignment) if matches!(assignment.value(), Expr::Ident(_)) => {
                let target = assignment.target();
                self.is_name(target, references)
                    || matches!(target, Expr::Field(access)
                        if if matches!(access.object(), Expr::This(_)) {
                            references.resolve(access.field().name(), &access.field().span(), true) == Some(self.declarator)
                        } else {
                            references.resolved_fields.iter().any(|(reference, declaration)|
                                *reference == access.field().span() && *declaration == self.declarator)
                        })
                    // A `PropertyAccess` (not a prefixed identifier) setter
                    // named like the variable.
                    || matches!(target, Expr::Field(access)
                        if access.field().name() == self.name
                            && (access.is_null_safe() || !matches!(access.object(), Expr::Ident(_))))
                    // Only dynamic receivers have no analyzer write element;
                    // missing SDK/package index entries are not dynamic.
                    || (references.dynamic_writes.contains(&(target.span().start, target.span().end))
                        && self.is_name(assignment.value(), references))
            }
            Expr::Call(call)
                if !excluded_calls.contains(&(call.span().start, call.span().end)) => {
                let invoked = matches!(call.callee(), Expr::Field(access)
                    if (self.is_reference(access.object(), references)
                        && self.methods.contains(&access.field().name()))
                        || (self.methods.contains(&access.field().name())
                            && self.declarator.contains(call.span()))
                        // Only field/accessor elements match the official
                        // cascade helper for an arbitrary method name.
                        || (self.kind == VariableKind::Field && self.is_name(access.object(), references)));
                invoked || self.passed_in(call.args(), references)
            }
            Expr::Field(access) => {
                self.kind == VariableKind::Local
                    && self.is_name(access.object(), references)
                    && self.methods.contains(&access.field().name())
            }
            Expr::Cascade(cascade) => cascade.sections().any(|section| {
                matches!(section.ops().next(), Some(CascadeOp::Call(call))
                    if (self.kind == VariableKind::Field && self.is_name(cascade.object(), references))
                        || (self.is_reference(cascade.object(), references)
                            && self.methods.contains(&call.name().name()))
                        || (self.declarator.contains(node.span())
                            && self.methods.contains(&call.name().name())))
                    || section.ops().any(|op| match op {
                        CascadeOp::Call(call) => self.passed_in(call.args(), references),
                        CascadeOp::Invoke(call) => self.passed_in(call.args(), references),
                        _ => false,
                    })
                    || section.ops().any(|op| {
                        let (target, value, indexed) = match op {
                            CascadeOp::Assign(assignment) => (assignment.target(), assignment.value(), false),
                            CascadeOp::IndexAssign(assignment) => (assignment.target(), assignment.value(), true),
                            _ => return false,
                        };
                        matches!(value, Expr::Ident(_))
                            && ((!indexed
                                && references.cascade_properties.contains(&(target.span().start, target.span().end))
                                && matches!(target, Expr::Ident(name) if name.name() == self.name))
                                || references.dynamic_writes.contains(&(target.span().start, target.span().end))
                                    && self.is_name(value, references))
                    })
            }),
            _ => false,
        }
    }
}

struct LocalCandidates<'a> {
    rule: LeakRule,
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    bodies: &'a FunctionBodies<'a>,
    candidates: Vec<Candidate<'a>>,
    references: References<'a>,
    excluded_calls: Vec<(usize, usize)>,
}

impl<'a> SemanticRuleVisitor<'a> for LocalCandidates<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        let mut uses = ValidUses {
            candidates: self.candidates,
            references: self.references,
            excluded_calls: self.excluded_calls,
        };
        walk_program(&mut uses, self.program);
        uses.candidates
            .iter()
            .filter(|candidate| !candidate.valid)
            .map(|candidate| self.ctx.diagnostic(self.rule.message, candidate.declarator))
            .collect()
    }

    fn visit_program_end(&mut self, state: &SemanticState<'a>) {
        let mut fields = FieldCandidates {
            rule: &self.rule,
            state,
            file: Span::new(0, self.ctx.source().len()),
            candidates: std::mem::take(&mut self.candidates),
        };
        walk_program(&mut fields, self.program);
        self.candidates = fields.candidates;
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let enclosing = self
            .references
            .extension_receivers
            .iter()
            .find(|(scope, _)| scope.contains(node.span()))
            .map(|(_, receiver)| receiver)
            .or(state.enclosing_type.as_ref());
        let static_owner = if let Expr::Field(access) = node {
            falcon_analyze::expression_segments(access.object(), false).and_then(|segments| {
                let head = segments.first()?;
                if state.environment.is_bound(head) {
                    return None;
                }
                let identity = state.model.resolve_dotted(&segments)?;
                state
                    .signatures
                    .declared_member_facts(&identity, access.field().name())
                    .iter()
                    .any(|facts| {
                        facts.is_static
                            && matches!(facts.kind, falcon_analyze::MemberKind::Field { .. })
                    })
                    .then_some(identity)
            })
        } else {
            None
        };
        let reference = match node {
            Expr::Ident(identifier)
                if self.references.bindings.iter().any(|binding| {
                    binding.kind == BindingKind::Field && binding.name == identifier.name()
                }) && self
                    .references
                    .resolve(identifier.name(), &identifier.span(), false)
                    .is_none_or(|declaration| {
                        self.references.bindings.iter().any(|binding| {
                            binding.declaration == declaration && binding.kind == BindingKind::Field
                        })
                    }) =>
            {
                enclosing.map(|receiver| (identifier, receiver.clone()))
            }
            Expr::Field(access)
                if self.references.bindings.iter().any(|binding| {
                    binding.kind == BindingKind::Field && binding.name == access.field().name()
                }) =>
            {
                if matches!(access.object(), Expr::This(_)) {
                    enclosing.map(|receiver| (access.field(), receiver.clone()))
                } else {
                    Some((access.field(), state.infer_receiver(access.object())))
                }
            }
            _ => None,
        };
        if let Some((identifier, receiver)) = reference
            && let Some(owner) = static_owner
                .or_else(|| {
                    state.signatures.resolved_member_owner(
                        &receiver,
                        identifier.name(),
                        &state.model,
                    )
                })
                .or_else(|| {
                    self.references
                        .mixin_constraints
                        .iter()
                        .filter(|(scope, _)| scope.contains(node.span()))
                        .find_map(|(_, constraint)| {
                            state.signatures.resolved_member_owner(
                                constraint,
                                identifier.name(),
                                &state.model,
                            )
                        })
                })
            && let Some((scope, _)) = self
                .references
                .owners
                .iter()
                .find(|(_, name)| state.model.resolve_name(*name).as_ref() == Some(&owner))
            && let Some(binding) = self.references.bindings.iter().find(|binding| {
                binding.kind == BindingKind::Field
                    && binding.scope == *scope
                    && binding.name == identifier.name()
            })
        {
            self.references
                .resolved_fields
                .push((identifier.span(), binding.declaration));
        }
        if let Expr::Assign(assignment) = node
            && let Expr::Ident(identifier) = assignment.value()
            && self
                .references
                .resolve(identifier.name(), &identifier.span(), false)
                .is_some_and(|declaration| {
                    self.candidates
                        .iter()
                        .any(|candidate| candidate.declarator == declaration)
                        || self.references.bindings.iter().any(|binding| {
                            binding.declaration == declaration && binding.kind == BindingKind::Field
                        })
                })
            && let Some(object) = (match assignment.target() {
                Expr::Field(access) => Some(access.object()),
                Expr::Index(access) => Some(access.object()),
                _ => None,
            })
            && matches!(state.infer_receiver(object), ResolvedType::Dynamic)
        {
            let target = assignment.target();
            self.references
                .dynamic_writes
                .insert((target.span().start, target.span().end));
        }
        let Expr::Call(call) = node else {
            return;
        };
        let callee = call.callee();
        let span = call.span();
        let callable = match callee {
            Expr::GenericInstantiation(instantiation) => instantiation.target(),
            target => target,
        };
        let callable_value = match callable {
            Expr::Ident(name) => {
                let binding = self
                    .references
                    .resolve(name.name(), &name.span(), false)
                    .and_then(|declaration| {
                        self.references
                            .bindings
                            .iter()
                            .find(|binding| binding.declaration == declaration)
                    });
                match binding.map(|binding| binding.kind) {
                    Some(BindingKind::Local | BindingKind::Parameter) => true,
                    None | Some(BindingKind::Field) => enclosing.is_some_and(|receiver| {
                        state
                            .signatures
                            .resolved_field(receiver, name.name(), &state.model)
                            .or_else(|| {
                                state.signatures.resolved_extension_getter(
                                    receiver,
                                    name.name(),
                                    &state.model,
                                )
                            })
                            .is_some()
                    }),
                    Some(BindingKind::Method | BindingKind::LocalFunction) => false,
                }
            }
            Expr::Field(access) => {
                let receiver = state.infer_receiver(access.object());
                state
                    .signatures
                    .resolved_field(&receiver, access.field().name(), &state.model)
                    .or_else(|| {
                        state.signatures.resolved_extension_getter(
                            &receiver,
                            access.field().name(),
                            &state.model,
                        )
                    })
                    .is_some()
            }
            // Invoking any other expression is FunctionExpressionInvocation,
            // not a method invocation visited by the official leak detector.
            _ => true,
        };
        if callable_value {
            self.excluded_calls.push((span.start, span.end));
            return;
        }
        let identity = resolve_call_constructor(callee, &state.model, true);
        if identity.is_some_and(|(identity, name)| {
            self.references
                .resolve(identity.name(), &callee.span(), false)
                .is_none()
                && (name == "new"
                    || state.signatures.constructor(&identity, name).is_some()
                    || self
                        .references
                        .constructors
                        .iter()
                        .any(|(span, constructor)| {
                            *constructor == name
                                && self.references.owners.iter().any(|(owner, class)| {
                                    owner.contains(*span)
                                        && state.model.resolve_name(*class).as_ref()
                                            == Some(&identity)
                                })
                        }))
        }) {
            self.excluded_calls.push((span.start, span.end));
        }
    }

    fn visit_cascade_assignment(
        &mut self,
        assignment: CascadeAssignment<'a>,
        receiver: &ResolvedType,
        _section: Span,
        _state: &SemanticState<'a>,
    ) {
        let CascadeAssignment {
            target,
            value,
            indexed,
            ..
        } = assignment;
        if !matches!(value, Expr::Ident(_)) {
            return;
        }
        let span = (target.span().start, target.span().end);
        if !indexed {
            self.references.cascade_properties.insert(span);
        }
        if matches!(receiver, ResolvedType::Dynamic) {
            self.references.dynamic_writes.insert(span);
        }
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        let Stmt::LocalVar(declaration) = node else {
            return;
        };
        let Some(container) = self
            .bodies
            .enclosing(&declaration.span())
            .map(|body| body.span)
        else {
            return;
        };
        for declarator in declaration.declarators() {
            self.candidates.extend(self.rule.candidate(
                declarator,
                declaration.var_type(),
                VariableKind::Local,
                container,
                state,
            ));
        }
    }
}

struct FieldCandidates<'r, 's, 'a> {
    rule: &'r LeakRule,
    state: &'s SemanticState<'a>,
    file: Span,
    candidates: Vec<Candidate<'a>>,
}

impl<'a> Visitor<'a> for FieldCandidates<'_, '_, 'a> {
    fn visit_field_decl(&mut self, node: FieldDecl<'a>) {
        for declarator in node.declarators() {
            self.candidates.extend(self.rule.candidate(
                declarator,
                node.field_type(),
                VariableKind::Field,
                self.file,
                self.state,
            ));
        }
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        let elements = match node {
            TopLevelDecl::Class(class) => Some(class.element_members()),
            TopLevelDecl::MixinClass(class) => Some(class.element_members()),
            TopLevelDecl::Enum(class) => Some(class.element_members()),
            _ => None,
        };
        if let Some(elements) = elements {
            for param in elements.declaring_params() {
                let Some(ty) = param.param_type() else {
                    continue;
                };
                let ty = self.state.model.resolve_type(ty);
                let methods = self.rule.methods(&ty, self.state);
                if !methods.is_empty() {
                    self.candidates.push(Candidate {
                        name: param.name().name(),
                        declarator: param.span(),
                        container: self.file,
                        kind: VariableKind::Field,
                        methods,
                        valid: false,
                    });
                }
            }
        }
        visitor::walk_top_level_decl(self, node);
    }
}

struct ValidUses<'a> {
    candidates: Vec<Candidate<'a>>,
    references: References<'a>,
    excluded_calls: Vec<(usize, usize)>,
}

impl ValidUses<'_> {
    fn mark(&mut self, span: &Span, valid: impl Fn(&Candidate<'_>) -> bool) {
        for candidate in &mut self.candidates {
            if !candidate.valid && candidate.container.contains(*span) && valid(candidate) {
                candidate.valid = true;
            }
        }
    }
}

impl<'a> Visitor<'a> for ValidUses<'a> {
    fn visit_expr(&mut self, node: Expr<'a>) {
        let references = &self.references;
        let excluded_calls = &self.excluded_calls;
        for candidate in &mut self.candidates {
            if !candidate.valid
                && candidate.container.contains(node.span())
                && candidate.is_valid_use(node, references, excluded_calls)
            {
                candidate.valid = true;
            }
        }
        walk_expr(self, node);
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) {
        if let Stmt::Return(statement) = node
            && let Some(value) = statement.value()
        {
            let references = &self.references;
            for candidate in &mut self.candidates {
                if candidate.container.contains(statement.span())
                    && candidate.kind == VariableKind::Local
                    && candidate.is_name(value, references)
                {
                    candidate.valid = true;
                }
            }
        }
        walk_stmt(self, node);
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'a>) {
        for initializer in node.initializers {
            if let ConstructorInitializer::FieldInit(initializer) = initializer {
                let span = initializer.span();
                let binding = self
                    .references
                    .resolve(initializer.field().name(), &span, true);
                self.mark(&span, |candidate| {
                    candidate.kind == VariableKind::Field && binding == Some(candidate.declarator)
                });
            }
        }
    }

    fn visit_primary_constructor_body(&mut self, node: PrimaryConstructorBody<'a>) {
        for initializer in node.initializers() {
            if let ConstructorInitializer::FieldInit(initializer) = initializer {
                let span = initializer.span();
                let binding = self
                    .references
                    .resolve(initializer.field().name(), &span, true);
                self.mark(&span, |candidate| {
                    candidate.kind == VariableKind::Field && binding == Some(candidate.declarator)
                });
            }
        }
        visitor::walk_primary_constructor_body(self, node);
    }

    fn visit_formal_param(&mut self, node: FormalParam<'a>) {
        if matches!(node.initializing(), Some(InitializingFormal::This)) {
            let binding = self
                .references
                .resolve(node.name().name(), &node.span(), true);
            self.mark(&node.span(), |candidate| {
                candidate.kind == VariableKind::Field && binding == Some(candidate.declarator)
            });
        }
        walk_formal_param(self, node);
    }
}

/// A lexical identity index built exclusively through the shared CST visitor.
/// Parameters use their callable's scope, fields their class's scope, and
/// locals the innermost block/loop/callable containing their declaration.
struct References<'a> {
    scopes: Vec<Span>,
    classes: Vec<Span>,
    callables: Vec<Span>,
    bindings: Vec<Binding<'a>>,
    owners: Vec<(Span, &'a str)>,
    resolved_fields: Vec<(Span, Span)>,
    extension_receivers: Vec<(Span, ResolvedType)>,
    mixin_constraints: Vec<(Span, ResolvedType)>,
    constructors: Vec<(Span, &'a str)>,
    primary_parameters: Vec<(Span, Span)>,
    in_primary: bool,
    /// Writes whose receiver is semantically dynamic, including cascades.
    dynamic_writes: HashSet<(usize, usize)>,
    /// Cascade selectors represented as identifiers, not index expressions.
    cascade_properties: HashSet<(usize, usize)>,
}

struct Binding<'a> {
    name: &'a str,
    declaration: Span,
    scope: Span,
    kind: BindingKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BindingKind {
    Local,
    Parameter,
    Field,
    Method,
    LocalFunction,
}

impl<'a> References<'a> {
    fn new(program: Program<'a>, len: usize, model: &SemanticModel<'_>) -> Self {
        let file = Span::new(0, len);
        let mut references = Self {
            scopes: vec![file],
            classes: Vec::new(),
            callables: Vec::new(),
            bindings: Vec::new(),
            owners: Vec::new(),
            resolved_fields: Vec::new(),
            extension_receivers: Vec::new(),
            mixin_constraints: Vec::new(),
            constructors: Vec::new(),
            primary_parameters: Vec::new(),
            in_primary: false,
            dynamic_writes: HashSet::new(),
            cascade_properties: HashSet::new(),
        };
        struct Contexts<'r, 's, 'model, 'tree> {
            references: &'r mut References<'tree>,
            model: &'s SemanticModel<'model>,
        }
        impl<'tree> Visitor<'tree> for Contexts<'_, '_, '_, 'tree> {
            fn visit_top_level_decl(&mut self, node: TopLevelDecl<'tree>) {
                if let TopLevelDecl::Extension(extension) = node {
                    self.references.extension_receivers.push((
                        extension.span(),
                        self.model.resolve_type(extension.on_type()),
                    ));
                }
                if let TopLevelDecl::Mixin(mixin) = node {
                    self.references.mixin_constraints.extend(
                        mixin
                            .on_clause()
                            .map(|constraint| (mixin.span(), self.model.resolve_type(constraint))),
                    );
                }
                self.references.visit_top_level_decl(node);
            }
        }
        walk_program(
            &mut Contexts {
                references: &mut references,
                model,
            },
            program,
        );
        for binding in &mut references.bindings {
            if let Some((_, scope)) = references
                .primary_parameters
                .iter()
                .find(|(span, _)| *span == binding.declaration)
            {
                binding.scope = *scope;
                continue;
            }
            let scopes = match binding.kind {
                BindingKind::Local | BindingKind::LocalFunction => &references.scopes,
                BindingKind::Parameter => &references.callables,
                BindingKind::Field | BindingKind::Method => &references.classes,
            };
            binding.scope = scopes
                .iter()
                .copied()
                .filter(|scope| {
                    scope.contains(binding.declaration)
                        && !(binding.kind == BindingKind::LocalFunction
                            && *scope == binding.declaration)
                })
                .min_by_key(|scope| scope.end - scope.start)
                .unwrap_or(file);
        }
        references
    }

    fn bind(&mut self, name: Identifier<'a>, declaration: Span, kind: BindingKind) {
        self.bindings.push(Binding {
            name: name.name(),
            declaration,
            scope: declaration,
            kind,
        });
    }

    fn callable(&mut self, span: Span) {
        self.scopes.push(span);
        self.callables.push(span);
    }

    fn resolve(&self, name: &str, span: &Span, field: bool) -> Option<Span> {
        if let Some((_, declaration)) = self
            .resolved_fields
            .iter()
            .find(|(reference, _)| reference == span)
        {
            return Some(*declaration);
        }
        self.bindings
            .iter()
            .filter(|binding| {
                binding.name == name
                    && (!field || binding.kind == BindingKind::Field)
                    && binding.scope.contains(*span)
            })
            .min_by_key(|binding| binding.scope.end - binding.scope.start)
            .map(|binding| binding.declaration)
    }
}

impl<'a> Visitor<'a> for References<'a> {
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        let owner = match node {
            TopLevelDecl::Class(class) => Some(class.name()),
            TopLevelDecl::Mixin(class) => Some(class.name()),
            TopLevelDecl::MixinClass(class) => Some(class.name()),
            TopLevelDecl::Enum(class) => Some(class.name()),
            _ => None,
        };
        if let Some(owner) = owner {
            self.owners.push((node.span(), owner.name()));
        }
        match node {
            TopLevelDecl::Class(_)
            | TopLevelDecl::Mixin(_)
            | TopLevelDecl::MixinClass(_)
            | TopLevelDecl::Enum(_)
            | TopLevelDecl::Extension(_)
            | TopLevelDecl::ExtensionType(_) => {
                self.classes.push(node.span());
                self.scopes.push(node.span());
            }
            TopLevelDecl::Variable(declaration) => {
                for variable in declaration.declarators() {
                    self.bind(variable.name(), variable.span(), BindingKind::Local);
                }
            }
            _ => {}
        }
        let elements = match node {
            TopLevelDecl::Class(class) => Some(class.element_members()),
            TopLevelDecl::MixinClass(class) => Some(class.element_members()),
            TopLevelDecl::Enum(class) => Some(class.element_members()),
            TopLevelDecl::ExtensionType(class) => Some(class.element_members()),
            _ => None,
        };
        if let Some(mut elements) = elements
            && let Some(primary) = elements.primary
        {
            let body_scope = elements
                .declared
                .find_map(|member| match member {
                    ClassMember::PrimaryConstructorBody(body) => Some(body.span()),
                    _ => None,
                })
                .unwrap_or(primary.span());
            self.primary_parameters
                .reserve(primary.params().params().count());
            for param in primary.params().params() {
                if param.initializing() == Some(InitializingFormal::This) {
                    continue;
                }
                let field = elements.representation || param.is_var() || param.is_final();
                self.bind(
                    param.name(),
                    param.span(),
                    if field {
                        BindingKind::Field
                    } else {
                        BindingKind::Parameter
                    },
                );
                if !field {
                    self.primary_parameters.push((param.span(), body_scope));
                }
            }
        }
        visitor::walk_top_level_decl(self, node);
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) {
        self.callable(node.span());
        visitor::walk_function_decl(self, node);
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) {
        if let ClassMember::Method(method) = node {
            self.bind(method.name(), method.span(), BindingKind::Method);
        }
        if !matches!(node, ClassMember::Field(_)) {
            self.callable(node.span());
        }
        visitor::walk_class_member(self, node);
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) {
        for variable in node.declarators() {
            self.bind(variable.name(), variable.span(), BindingKind::Field);
        }
        visitor::walk_field_decl(self, node);
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'a>) {
        self.constructors.push((
            node.span,
            node.constructor_name.map_or("new", |name| name.name()),
        ));
    }

    fn visit_primary_constructor(&mut self, node: PrimaryConstructor<'a>) {
        self.in_primary = true;
        visitor::walk_primary_constructor(self, node);
        self.in_primary = false;
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) {
        for variable in node.declarators() {
            self.bind(variable.name(), variable.span(), BindingKind::Local);
        }
        visitor::walk_local_var(self, node);
    }

    fn visit_formal_param(&mut self, node: FormalParam<'a>) {
        // `this.field` parameters are field references, not shadowing locals.
        if !self.in_primary && node.initializing() != Some(InitializingFormal::This) {
            self.bind(node.name(), node.span(), BindingKind::Parameter);
        }
        if let Some(parameters) = node.function_params() {
            self.callables.push(parameters.span());
        }
        walk_formal_param(self, node);
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) {
        match node {
            Stmt::Block(block) => self.scopes.push(block.span()),
            Stmt::LocalFunc(function) => {
                self.bind(function.name(), function.span(), BindingKind::LocalFunction);
                self.callable(function.span());
            }
            Stmt::For(statement) => {
                self.scopes.push(statement.span());
                if let Some(ForInit::ForIn(initializer)) = statement.header().init()
                    && initializer.declares_variable()
                {
                    self.bind(
                        initializer.name(),
                        initializer.name().span(),
                        BindingKind::Local,
                    );
                }
            }
            Stmt::TryCatch(statement) => {
                self.scopes.push(statement.body().span());
                for catch in statement.catches() {
                    self.scopes.push(catch.span());
                    self.scopes.push(catch.body().span());
                    for name in [catch.exception_var(), catch.stack_trace_var()]
                        .into_iter()
                        .flatten()
                    {
                        self.bind(name, name.span(), BindingKind::Local);
                    }
                }
                if let Some(block) = statement.finally() {
                    self.scopes.push(block.span());
                }
            }
            _ => {}
        }
        walk_stmt(self, node);
    }

    fn visit_block(&mut self, node: Block<'a>) {
        self.scopes.push(node.span());
        visitor::walk_block(self, node);
    }

    fn visit_switch_case(&mut self, node: SwitchCase<'a>) {
        self.scopes.push(node.span());
        visitor::walk_switch_case(self, node);
    }

    fn visit_element<E: Element<'a>>(&mut self, node: E) {
        let kind = node.kind();
        if let ElementKind::For { header, .. } = kind {
            self.scopes.push(node.span());
            if let Some(ForInit::ForIn(initializer)) = header.init()
                && initializer.declares_variable()
            {
                self.bind(
                    initializer.name(),
                    initializer.name().span(),
                    BindingKind::Local,
                );
            }
        }
        visitor::walk_element(self, kind);
    }

    fn visit_expr(&mut self, node: Expr<'a>) {
        if matches!(node, Expr::FuncExpr(_)) {
            self.callable(node.span());
        }
        walk_expr(self, node);
    }
}
