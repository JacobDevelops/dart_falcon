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
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    self, Visitor, walk_expr, walk_formal_param, walk_program, walk_stmt,
};

use crate::lint::function_bodies::{FunctionBodies, contains};
use falcon_analyze::semantic_scope::{CascadeAssignment, SemanticRuleVisitor, SemanticState};

/// A resource type and the method that releases it.
pub(crate) struct Release {
    pub(crate) library: &'static str,
    pub(crate) type_name: &'static str,
    pub(crate) method: &'static str,
}

#[derive(Clone, Copy)]
pub(crate) struct LeakRule {
    pub(crate) name: &'static str,
    pub(crate) message: &'static str,
    pub(crate) releases: &'static [Release],
}

impl LeakRule {
    pub(crate) fn semantic_visitor<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> Option<Box<dyn SemanticRuleVisitor + 'a>> {
        let model = SemanticModel::new(ctx.file_path, ctx.identities?, ctx.types);
        Some(Box::new(LocalCandidates {
            rule: *self,
            program,
            ctx,
            bodies: FunctionBodies::new(program),
            references: References::new(program, ctx.source.len(), &model),
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

    fn candidate(
        &self,
        declarator: &VarDeclarator,
        declared: Option<&DartType>,
        kind: VariableKind,
        container: Span,
        state: &SemanticState<'_>,
    ) -> Option<Candidate> {
        // `var a = b;` only aliases a resource tracked through `b`.
        if matches!(&declarator.initializer, Some(Expr::Ident(_))) {
            return None;
        }
        let ty = match (declared, &declarator.initializer) {
            (Some(declared), _) => state
                .model
                .resolve_type_in(declared, &state.type_parameters),
            (None, Some(Expr::Cascade { object, .. })) => state.infer_receiver(object),
            (None, Some(initializer)) => state.infer_receiver(initializer),
            (None, None) => return None,
        };
        let methods = self.methods(&ty, state);
        (!methods.is_empty()).then(|| Candidate {
            name: declarator.name.name.clone(),
            declarator: declarator.span,
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

struct Candidate {
    name: String,
    declarator: Span,
    container: Span,
    kind: VariableKind,
    methods: Vec<&'static str>,
    valid: bool,
}

impl Candidate {
    /// `name`, `name!`, or `this.name`.
    fn is_reference(&self, expr: &Expr, references: &References) -> bool {
        match expr {
            Expr::Ident(_) => self.is_name(expr, references),
            Expr::NullAssert { operand, .. } => self.is_name(operand, references),
            Expr::Field { object, field, .. } if matches!(object.as_ref(), Expr::This { .. }) => {
                references.resolve(&field.name, &field.span, true) == Some(self.declarator)
            }
            _ => false,
        }
    }

    fn is_name(&self, expr: &Expr, references: &References) -> bool {
        matches!(expr, Expr::Ident(identifier)
            if references.resolve(&identifier.name, &identifier.span, false) == Some(self.declarator))
    }

    fn passed_in(&self, args: &ArgList, references: &References) -> bool {
        self.kind == VariableKind::Local
            && args
                .positional
                .iter()
                .any(|argument| self.is_name(argument, references))
    }

    fn is_valid_use(
        &self,
        node: &Expr,
        references: &References,
        excluded_calls: &[(usize, usize)],
    ) -> bool {
        match node {
            Expr::Assign { target, value, .. }
                if matches!(value.as_ref(), Expr::Ident(_)) => {
                self.is_name(target, references)
                    || matches!(target.as_ref(), Expr::Field { object, field, .. }
                        if if matches!(object.as_ref(), Expr::This { .. }) {
                            references.resolve(&field.name, &field.span, true) == Some(self.declarator)
                        } else {
                            references.resolved_fields.iter().any(|(reference, declaration)|
                                *reference == field.span && *declaration == self.declarator)
                        })
                    // A `PropertyAccess` (not a prefixed identifier) setter
                    // named like the variable.
                    || matches!(target.as_ref(), Expr::Field { object, field, is_null_safe, .. }
                        if field.name == self.name
                            && (*is_null_safe || !matches!(object.as_ref(), Expr::Ident(_))))
                    // Only dynamic receivers have no analyzer write element;
                    // missing SDK/package index entries are not dynamic.
                    || (references.dynamic_writes.contains(&(target.span().start, target.span().end))
                        && self.is_name(value, references))
            }
            Expr::Call {
                callee, args, span, ..
            } if !excluded_calls.contains(&(span.start, span.end)) => {
                let invoked = matches!(callee.as_ref(), Expr::Field { object, field, .. }
                    if (self.is_reference(object, references)
                        && self.methods.contains(&field.name.as_str()))
                        || (self.methods.contains(&field.name.as_str())
                            && contains(&self.declarator, span))
                        // Only field/accessor elements match the official
                        // cascade helper for an arbitrary method name.
                        || (self.kind == VariableKind::Field && self.is_name(object, references)));
                invoked || self.passed_in(args, references)
            }
            Expr::Field { object, field, .. } => {
                self.kind == VariableKind::Local
                    && self.is_name(object, references)
                    && self.methods.contains(&field.name.as_str())
            }
            Expr::Cascade {
                object, sections, ..
            } => sections.iter().any(|section| {
                matches!(section.ops.first(), Some(CascadeOp::Call(method, _, _))
                    if (self.kind == VariableKind::Field && self.is_name(object, references))
                        || (self.is_reference(object, references)
                            && self.methods.contains(&method.name.as_str()))
                        || (contains(&self.declarator, node.span())
                            && self.methods.contains(&method.name.as_str())))
                    || section.ops.iter().any(|op|
                        matches!(op, CascadeOp::Call(_, _, args) | CascadeOp::Invoke(_, args)
                            if self.passed_in(args, references)))
                    || section.ops.iter().any(|op| {
                        matches!(op, CascadeOp::Assign(target, _, value)
                            if matches!(value.as_ref(), Expr::Ident(_))
                                && (references.cascade_properties.contains(&(target.span().start, target.span().end))
                                    && matches!(target.as_ref(), Expr::Ident(name) if name.name == self.name)
                                    || references.dynamic_writes.contains(&(target.span().start, target.span().end))
                                        && self.is_name(value, references)))
                            || matches!(op, CascadeOp::IndexAssign(target, _, value)
                                if matches!(value.as_ref(), Expr::Ident(_))
                                    && references.dynamic_writes.contains(&(target.span().start, target.span().end))
                                    && self.is_name(value, references))
                    })
            }),
            _ => false,
        }
    }
}

struct LocalCandidates<'a> {
    rule: LeakRule,
    program: &'a Program,
    ctx: &'a AnalyzeContext<'a>,
    bodies: FunctionBodies,
    candidates: Vec<Candidate>,
    references: References,
    excluded_calls: Vec<(usize, usize)>,
}

impl SemanticRuleVisitor for LocalCandidates<'_> {
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
            .map(|candidate| {
                self.ctx
                    .diagnostic(self.rule.name, self.rule.message, candidate.declarator)
            })
            .collect()
    }

    fn visit_program_end(&mut self, state: &SemanticState<'_>) {
        let mut fields = FieldCandidates {
            rule: &self.rule,
            state,
            file: Span::new(0, self.ctx.source.len()),
            candidates: std::mem::take(&mut self.candidates),
        };
        walk_program(&mut fields, self.program);
        self.candidates = fields.candidates;
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let enclosing = self
            .references
            .extension_receivers
            .iter()
            .find(|(scope, _)| contains(scope, node.span()))
            .map(|(_, receiver)| receiver)
            .or(state.enclosing_type.as_ref());
        let static_owner = if let Expr::Field { object, field, .. } = node {
            falcon_analyze::expression_segments(object, false).and_then(|segments| {
                let head = segments.first()?;
                if state.environment.is_bound(head) {
                    return None;
                }
                let identity = state.model.resolve_dotted(&segments)?;
                state
                    .signatures
                    .declared_member_facts(&identity, &field.name)
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
                    binding.kind == BindingKind::Field && binding.name == identifier.name
                }) && self
                    .references
                    .resolve(&identifier.name, &identifier.span, false)
                    .is_none_or(|declaration| {
                        self.references.bindings.iter().any(|binding| {
                            binding.declaration == declaration && binding.kind == BindingKind::Field
                        })
                    }) =>
            {
                enclosing.map(|receiver| (identifier, receiver.clone()))
            }
            Expr::Field { object, field, .. }
                if self.references.bindings.iter().any(|binding| {
                    binding.kind == BindingKind::Field && binding.name == field.name
                }) =>
            {
                if matches!(object.as_ref(), Expr::This { .. }) {
                    enclosing.map(|receiver| (field, receiver.clone()))
                } else {
                    Some((field, state.infer_receiver(object)))
                }
            }
            _ => None,
        };
        if let Some((identifier, receiver)) = reference
            && let Some(owner) = static_owner
                .or_else(|| {
                    state.signatures.resolved_member_owner(
                        &receiver,
                        &identifier.name,
                        &state.model,
                    )
                })
                .or_else(|| {
                    self.references
                        .mixin_constraints
                        .iter()
                        .filter(|(scope, _)| contains(scope, node.span()))
                        .find_map(|(_, constraint)| {
                            state.signatures.resolved_member_owner(
                                constraint,
                                &identifier.name,
                                &state.model,
                            )
                        })
                })
            && let Some((scope, _)) =
                self.references.owners.iter().find(|(_, name)| {
                    state.model.resolve_name(name.as_str()).as_ref() == Some(&owner)
                })
            && let Some(binding) = self.references.bindings.iter().find(|binding| {
                binding.kind == BindingKind::Field
                    && binding.scope == *scope
                    && binding.name == identifier.name
            })
        {
            self.references
                .resolved_fields
                .push((identifier.span, binding.declaration));
        }
        if let Expr::Assign { target, value, .. } = node
            && let Expr::Ident(identifier) = value.as_ref()
            && self
                .references
                .resolve(&identifier.name, &identifier.span, false)
                .is_some_and(|declaration| {
                    self.candidates
                        .iter()
                        .any(|candidate| candidate.declarator == declaration)
                        || self.references.bindings.iter().any(|binding| {
                            binding.declaration == declaration && binding.kind == BindingKind::Field
                        })
                })
            && let Expr::Field { object, .. } | Expr::Index { object, .. } = target.as_ref()
            && matches!(state.infer_receiver(object), ResolvedType::Dynamic)
        {
            self.references
                .dynamic_writes
                .insert((target.span().start, target.span().end));
        }
        let Expr::Call { callee, span, .. } = node else {
            return;
        };
        let callable = match callee.as_ref() {
            Expr::GenericInstantiation { target, .. } => target.as_ref(),
            target => target,
        };
        let callable_value = match callable {
            Expr::Ident(name) => {
                let binding = self
                    .references
                    .resolve(&name.name, &name.span, false)
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
                            .resolved_field(receiver, &name.name, &state.model)
                            .or_else(|| {
                                state.signatures.resolved_extension_getter(
                                    receiver,
                                    &name.name,
                                    &state.model,
                                )
                            })
                            .is_some()
                    }),
                    Some(BindingKind::Method | BindingKind::LocalFunction) => false,
                }
            }
            Expr::Field { object, field, .. } => {
                let receiver = state.infer_receiver(object);
                state
                    .signatures
                    .resolved_field(&receiver, &field.name, &state.model)
                    .or_else(|| {
                        state.signatures.resolved_extension_getter(
                            &receiver,
                            &field.name,
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
                .resolve(identity.name(), callee.span(), false)
                .is_none()
                && (name == "new"
                    || state.signatures.constructor(&identity, name).is_some()
                    || self
                        .references
                        .constructors
                        .iter()
                        .any(|(span, constructor)| {
                            constructor == name
                                && self.references.owners.iter().any(|(owner, class)| {
                                    contains(owner, span)
                                        && state.model.resolve_name(class.as_str()).as_ref()
                                            == Some(&identity)
                                })
                        }))
        }) {
            self.excluded_calls.push((span.start, span.end));
        }
    }

    fn visit_cascade_assignment(
        &mut self,
        assignment: CascadeAssignment<'_>,
        receiver: &ResolvedType,
        _section: Span,
        _state: &SemanticState<'_>,
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

    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        let Stmt::LocalVar(declaration) = node else {
            return;
        };
        let Some(container) = self
            .bodies
            .enclosing(&declaration.span)
            .map(|body| body.span)
        else {
            return;
        };
        for declarator in &declaration.declarators {
            self.candidates.extend(self.rule.candidate(
                declarator,
                declaration.var_type.as_ref(),
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
    candidates: Vec<Candidate>,
}

impl Visitor for FieldCandidates<'_, '_, '_> {
    fn visit_field_decl(&mut self, node: &FieldDecl) {
        for declarator in &node.declarators {
            self.candidates.extend(self.rule.candidate(
                declarator,
                node.field_type.as_ref(),
                VariableKind::Field,
                self.file,
                self.state,
            ));
        }
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        let elements = match node {
            TopLevelDecl::Class(class) => Some(class.element_members()),
            TopLevelDecl::MixinClass(class) => Some(class.element_members()),
            TopLevelDecl::Enum(class) => Some(class.element_members()),
            _ => None,
        };
        if let Some(elements) = elements {
            for param in elements.declaring_params() {
                let Some(ty) = &param.param_type else {
                    continue;
                };
                let ty = self.state.model.resolve_type(ty);
                let methods = self.rule.methods(&ty, self.state);
                if !methods.is_empty() {
                    self.candidates.push(Candidate {
                        name: param.name.name.clone(),
                        declarator: param.span,
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

struct ValidUses {
    candidates: Vec<Candidate>,
    references: References,
    excluded_calls: Vec<(usize, usize)>,
}

impl ValidUses {
    fn mark(&mut self, span: &Span, valid: impl Fn(&Candidate) -> bool) {
        for candidate in &mut self.candidates {
            if !candidate.valid && contains(&candidate.container, span) && valid(candidate) {
                candidate.valid = true;
            }
        }
    }
}

impl Visitor for ValidUses {
    fn visit_expr(&mut self, node: &Expr) {
        let references = &self.references;
        let excluded_calls = &self.excluded_calls;
        for candidate in &mut self.candidates {
            if !candidate.valid
                && contains(&candidate.container, node.span())
                && candidate.is_valid_use(node, references, excluded_calls)
            {
                candidate.valid = true;
            }
        }
        walk_expr(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::Return(statement) = node
            && let Some(value) = &statement.value
        {
            let references = &self.references;
            for candidate in &mut self.candidates {
                if contains(&candidate.container, &statement.span)
                    && candidate.kind == VariableKind::Local
                    && candidate.is_name(value, references)
                {
                    candidate.valid = true;
                }
            }
        }
        walk_stmt(self, node);
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'_>) {
        for initializer in node.initializers {
            if let ConstructorInitializer::FieldInit { field, span, .. } = initializer {
                let binding = self.references.resolve(&field.name, span, true);
                self.mark(span, |candidate| {
                    candidate.kind == VariableKind::Field && binding == Some(candidate.declarator)
                });
            }
        }
    }

    fn visit_primary_constructor_body(&mut self, node: &PrimaryConstructorBody) {
        for initializer in &node.initializers {
            if let ConstructorInitializer::FieldInit { field, span, .. } = initializer {
                let binding = self.references.resolve(&field.name, span, true);
                self.mark(span, |candidate| {
                    candidate.kind == VariableKind::Field && binding == Some(candidate.declarator)
                });
            }
        }
        visitor::walk_primary_constructor_body(self, node);
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        if matches!(node.initializing, Some(InitializingFormal::This)) {
            let binding = self.references.resolve(&node.name.name, &node.span, true);
            self.mark(&node.span, |candidate| {
                candidate.kind == VariableKind::Field && binding == Some(candidate.declarator)
            });
        }
        walk_formal_param(self, node);
    }
}

/// A lexical identity index built exclusively through the shared AST visitor.
/// Parameters use their callable's scope, fields their class's scope, and
/// locals the innermost block/loop/callable containing their declaration.
struct References {
    scopes: Vec<Span>,
    classes: Vec<Span>,
    callables: Vec<Span>,
    bindings: Vec<Binding>,
    owners: Vec<(Span, String)>,
    resolved_fields: Vec<(Span, Span)>,
    extension_receivers: Vec<(Span, ResolvedType)>,
    mixin_constraints: Vec<(Span, ResolvedType)>,
    constructors: Vec<(Span, String)>,
    primary_parameters: Vec<(Span, Span)>,
    in_primary: bool,
    /// Writes whose receiver is semantically dynamic, including cascades.
    dynamic_writes: HashSet<(usize, usize)>,
    /// Cascade selectors represented as identifiers, not index expressions.
    cascade_properties: HashSet<(usize, usize)>,
}

struct Binding {
    name: String,
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

impl References {
    fn new(program: &Program, len: usize, model: &SemanticModel<'_>) -> Self {
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
        struct Contexts<'r, 's, 'a> {
            references: &'r mut References,
            model: &'s SemanticModel<'a>,
        }
        impl Visitor for Contexts<'_, '_, '_> {
            fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
                if let TopLevelDecl::Extension(extension) = node {
                    self.references
                        .extension_receivers
                        .push((extension.span, self.model.resolve_type(&extension.on_type)));
                }
                if let TopLevelDecl::Mixin(mixin) = node {
                    self.references.mixin_constraints.extend(
                        mixin
                            .on_clause
                            .iter()
                            .map(|constraint| (mixin.span, self.model.resolve_type(constraint))),
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
                    contains(scope, &binding.declaration)
                        && !(binding.kind == BindingKind::LocalFunction
                            && *scope == binding.declaration)
                })
                .min_by_key(|scope| scope.end - scope.start)
                .unwrap_or(file);
        }
        references
    }

    fn bind(&mut self, name: &Identifier, declaration: Span, kind: BindingKind) {
        self.bindings.push(Binding {
            name: name.name.clone(),
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
                    && contains(&binding.scope, span)
            })
            .min_by_key(|binding| binding.scope.end - binding.scope.start)
            .map(|binding| binding.declaration)
    }
}

impl Visitor for References {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        let owner = match node {
            TopLevelDecl::Class(class) => Some(&class.name),
            TopLevelDecl::Mixin(class) => Some(&class.name),
            TopLevelDecl::MixinClass(class) => Some(&class.name),
            TopLevelDecl::Enum(class) => Some(&class.name),
            _ => None,
        };
        if let Some(owner) = owner {
            self.owners.push((*node.span(), owner.name.clone()));
        }
        match node {
            TopLevelDecl::Class(_)
            | TopLevelDecl::Mixin(_)
            | TopLevelDecl::MixinClass(_)
            | TopLevelDecl::Enum(_)
            | TopLevelDecl::Extension(_)
            | TopLevelDecl::ExtensionType(_) => {
                self.classes.push(*node.span());
                self.scopes.push(*node.span());
            }
            TopLevelDecl::Variable(declaration) => {
                for variable in &declaration.declarators {
                    self.bind(&variable.name, variable.span, BindingKind::Local);
                }
            }
            _ => {}
        }
        let (primary, members, representation): (_, &[ClassMember], _) = match node {
            TopLevelDecl::Class(class) => {
                (class.primary_constructor.as_ref(), &class.members, false)
            }
            TopLevelDecl::MixinClass(class) => {
                (class.primary_constructor.as_ref(), &class.members, false)
            }
            TopLevelDecl::Enum(class) => {
                (class.primary_constructor.as_ref(), &class.members, false)
            }
            TopLevelDecl::ExtensionType(class) => {
                (Some(&class.primary_constructor), &class.members, true)
            }
            _ => (None, &[], false),
        };
        if let Some(primary) = primary {
            let body_scope = members
                .iter()
                .find_map(|member| match member {
                    ClassMember::PrimaryConstructorBody(body) => Some(body.span),
                    _ => None,
                })
                .unwrap_or(primary.span);
            self.primary_parameters.reserve(primary.params.params.len());
            for param in &primary.params.params {
                if param.initializing == Some(InitializingFormal::This) {
                    continue;
                }
                let field = representation || param.is_var || param.is_final;
                self.bind(
                    &param.name,
                    param.span,
                    if field {
                        BindingKind::Field
                    } else {
                        BindingKind::Parameter
                    },
                );
                if !field {
                    self.primary_parameters.push((param.span, body_scope));
                }
            }
        }
        visitor::walk_top_level_decl(self, node);
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.callable(node.span);
        visitor::walk_function_decl(self, node);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Method(method) = node {
            self.bind(&method.name, method.span, BindingKind::Method);
        }
        if !matches!(node, ClassMember::Field(_)) {
            self.callable(*node.span());
        }
        visitor::walk_class_member(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        for variable in &node.declarators {
            self.bind(&variable.name, variable.span, BindingKind::Field);
        }
        visitor::walk_field_decl(self, node);
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'_>) {
        self.constructors.push((
            node.span,
            node.constructor_name
                .as_ref()
                .map_or_else(|| "new".to_string(), |name| name.name.clone()),
        ));
    }

    fn visit_primary_constructor(&mut self, node: &PrimaryConstructor) {
        self.in_primary = true;
        visitor::walk_primary_constructor(self, node);
        self.in_primary = false;
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        for variable in &node.declarators {
            self.bind(&variable.name, variable.span, BindingKind::Local);
        }
        visitor::walk_local_var(self, node);
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        // `this.field` parameters are field references, not shadowing locals.
        if !self.in_primary && node.initializing != Some(InitializingFormal::This) {
            self.bind(&node.name, node.span, BindingKind::Parameter);
        }
        if let Some(parameters) = &node.function_params {
            self.callables.push(parameters.span);
        }
        walk_formal_param(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::Block(block) => self.scopes.push(block.span),
            Stmt::LocalFunc(function) => {
                self.bind(&function.name, function.span, BindingKind::LocalFunction);
                self.callable(function.span);
            }
            Stmt::For(statement) => {
                self.scopes.push(statement.span);
                if let Some(ForInit::ForIn {
                    declares_variable: true,
                    name,
                    ..
                }) = &statement.header.init
                {
                    self.bind(name, name.span, BindingKind::Local);
                }
            }
            Stmt::TryCatch(statement) => {
                self.scopes.push(statement.body.span);
                for catch in &statement.catches {
                    self.scopes.push(catch.span);
                    self.scopes.push(catch.body.span);
                    for name in [&catch.exception_var, &catch.stack_trace_var]
                        .into_iter()
                        .flatten()
                    {
                        self.bind(name, name.span, BindingKind::Local);
                    }
                }
                if let Some(block) = &statement.finally {
                    self.scopes.push(block.span);
                }
            }
            _ => {}
        }
        walk_stmt(self, node);
    }

    fn visit_block(&mut self, node: &Block) {
        self.scopes.push(node.span);
        visitor::walk_block(self, node);
    }

    fn visit_switch_case(&mut self, node: &SwitchCase) {
        self.scopes.push(node.span);
        visitor::walk_switch_case(self, node);
    }

    fn visit_collection_element(&mut self, node: &CollectionElement) {
        if let CollectionElement::For { header, span, .. } = node {
            self.scopes.push(*span);
            if let Some(ForInit::ForIn {
                declares_variable: true,
                name,
                ..
            }) = &header.init
            {
                self.bind(name, name.span, BindingKind::Local);
            }
        }
        visitor::walk_collection_element(self, node);
    }

    fn visit_map_element(&mut self, node: &MapElement) {
        if let MapElement::For { header, span, .. } = node {
            self.scopes.push(*span);
            if let Some(ForInit::ForIn {
                declares_variable: true,
                name,
                ..
            }) = &header.init
            {
                self.bind(name, name.span, BindingKind::Local);
            }
        }
        visitor::walk_map_element(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if matches!(node, Expr::FuncExpr { .. }) {
            self.callable(*node.span());
        }
        walk_expr(self, node);
    }
}
