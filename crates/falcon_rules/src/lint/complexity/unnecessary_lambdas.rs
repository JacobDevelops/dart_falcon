//! Flags a closure that only forwards its parameters to a function a tear-off could name.
//!
//! `names.forEach((name) { print(name); })` allocates a closure whose whole job
//! is to call `print`; `names.forEach(print)` says the same thing. The rule
//! fires on an anonymous, synchronous closure whose body is a single
//! invocation — `=> f(a, b)`, `{ f(a, b); }`, or `{ return f(a, b); }` —
//! passing exactly the closure's parameters, in order, as positional
//! arguments, and on `(a) => C(a)` constructor calls a `C.new` tear-off could
//! replace. The invoked function must stay the same object between calls:
//!
//! - a top-level, local, static, or instance method, or a `final`, non-`late`
//!   variable or field holding a function;
//! - reached through a receiver that is a class name, an import prefix, or a
//!   `final` variable or field, never through `this`, a parameter of the
//!   closure, or a null-aware access (`a?.b`).
//!
//! Generic invocations with explicit type arguments, calls through deferred
//! imports, and `const` constructor calls are skipped. When the closure is a
//! named argument or a variable initializer, a method tear-off must also fit
//! the closure's type, so the closure's parameter types must be known.
//!
//! falcon only checks closures whose parameters are all required positional,
//! and stays silent when it cannot resolve the invoked member.
//!
//! Adopted from the official `unnecessary_lambdas` lint.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberFacts, MemberKind, ResolvedType, Rule,
    resolve_call_constructor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_expr, walk_formal_param, walk_local_var, walk_pattern, walk_program, walk_stmt,
};

use crate::lint::declarations::members_of;
use crate::lint::lexical_usage::collection_for_inits;
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct UnnecessaryLambdas;

impl Rule for UnnecessaryLambdas {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut bindings = LocalBindings::default();
        bindings.visit_program(program);
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            program,
            bindings: bindings.0,
            typed_contexts: declaration_initializers(program),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

/// Whether every local binding of a name (variable, parameter, local function,
/// loop or catch variable, pattern variable) in the file is a `final`,
/// non-`late` variable or a local function.
#[derive(Default)]
struct LocalBindings(HashMap<String, bool>);

impl LocalBindings {
    fn record(&mut self, name: &str, is_final: bool) {
        let entry = self.0.entry(name.to_string()).or_insert(true);
        *entry &= is_final;
    }

    fn record_for_init(&mut self, init: &ForInit) {
        if let ForInit::ForIn { keyword, name, .. } = init {
            self.record(&name.name, *keyword == VarKeyword::Final);
        }
    }
}

impl Visitor for LocalBindings {
    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        let is_final =
            matches!(node.keyword, VarKeyword::Final | VarKeyword::Const) && !node.is_late;
        for declarator in &node.declarators {
            self.record(&declarator.name.name, is_final);
        }
        walk_local_var(self, node);
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        self.record(&node.name.name, node.is_final);
        walk_formal_param(self, node);
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        if let Pattern::Variable { name, .. } = node {
            self.record(&name.name, false);
        }
        walk_pattern(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::LocalFunc(function) => self.record(&function.name.name, true),
            Stmt::For(statement) => {
                if let Some(init) = &statement.header.init {
                    self.record_for_init(init);
                }
            }
            Stmt::TryCatch(statement) => {
                for clause in &statement.catches {
                    for name in clause.exception_var.iter().chain(&clause.stack_trace_var) {
                        self.record(&name.name, false);
                    }
                }
            }
            _ => {}
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        for init in collection_for_inits(node) {
            self.record_for_init(init);
        }
        walk_expr(self, node);
    }

    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }
}

/// Initializers of top-level variables and fields.
fn declaration_initializers(program: &Program) -> HashSet<(usize, usize)> {
    let mut spans = HashSet::new();
    for declaration in &program.declarations {
        if let TopLevelDecl::Variable(variable) = declaration {
            spans.extend(initializer_spans(&variable.declarators));
        }
        for member in members_of(declaration).unwrap_or_default() {
            if let ClassMember::Field(field) = member {
                spans.extend(initializer_spans(&field.declarators));
            }
        }
    }
    spans
}

fn initializer_spans(declarators: &[VarDeclarator]) -> impl Iterator<Item = (usize, usize)> + '_ {
    declarators
        .iter()
        .filter_map(|declarator| declarator.initializer.as_ref())
        .map(|initializer| (initializer.span().start, initializer.span().end))
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    program: &'a Program,
    bindings: HashMap<String, bool>,
    /// Closures that are named arguments or variable initializers; a method
    /// tear-off must be a subtype of their type.
    typed_contexts: HashSet<(usize, usize)>,
}

/// The single invocation a closure body consists of.
fn forwarded_invocation(body: &FunctionBody) -> Option<&Expr> {
    match body {
        FunctionBody::Arrow(expression, _) => Some(expression),
        FunctionBody::Block(block) => match block.stmts.as_slice() {
            [Stmt::Expr(statement)] if matches!(statement.expr, Expr::Call { .. }) => {
                Some(&statement.expr)
            }
            [
                Stmt::Return(ReturnStmt {
                    value: Some(value), ..
                }),
            ] if matches!(value, Expr::Call { .. }) => Some(value),
            _ => None,
        },
        FunctionBody::Native(..) => None,
    }
}

/// Whether `args` pass exactly `params`, in order, as positional arguments.
fn forwards_parameters(args: &ArgList, params: &[FormalParam]) -> bool {
    args.named.is_empty()
        && args.positional.len() == params.len()
        && args.positional.iter().zip(params).all(|(argument, param)| {
            matches!(argument, Expr::Ident(name) if name.name == param.name.name)
        })
}

fn has_method(facts: &[MemberFacts], is_static: bool) -> bool {
    facts
        .iter()
        .any(|fact| fact.kind == MemberKind::Method && fact.is_static == is_static)
}

/// A field declared `final` or `const` (falcon does not see `late` here).
fn has_final_field(facts: &[MemberFacts]) -> bool {
    !facts.is_empty()
        && facts.iter().all(|fact| {
            matches!(
                fact.kind,
                MemberKind::Field {
                    writable: false,
                    ..
                }
            )
        })
}

impl Collector<'_> {
    /// Whether a local binding named `name` is always `final` (or a local function).
    fn final_local(&self, name: &str) -> bool {
        self.bindings.get(name).copied().unwrap_or(false)
    }

    fn top_level_function(&self, name: &str) -> bool {
        self.program.declarations.iter().any(|declaration| {
            matches!(declaration, TopLevelDecl::Function(function)
                if function.name.name == name && matches!(function.kind, FunctionKind::Function(_)))
        })
    }

    fn final_top_level_variable(&self, name: &str) -> bool {
        self.program.declarations.iter().any(|declaration| {
            matches!(declaration, TopLevelDecl::Variable(variable)
                if matches!(variable.keyword, VarKeyword::Final | VarKeyword::Const)
                    && !variable.is_late
                    && variable.declarators.iter().any(|declarator| declarator.name.name == name))
        })
    }

    fn import_prefix(&self, name: &str) -> Option<&ImportDirective> {
        self.program.imports.iter().find(|import| {
            import
                .as_name
                .as_ref()
                .is_some_and(|prefix| prefix.name == name)
        })
    }

    /// Facts for `name` as a member of the enclosing type, through `this`.
    fn enclosing_member<'s>(
        &self,
        name: &str,
        state: &'s SemanticState<'_>,
    ) -> Option<&'s [MemberFacts]> {
        let receiver = state.enclosing_type.as_ref()?;
        state
            .signatures
            .resolved_member_facts(receiver, name, &state.model)
    }

    /// Whether the unqualified `name` invoked as `name(...)` is a function or
    /// method, or a `final` variable or field holding one.
    fn stable_function(
        &self,
        name: &str,
        params: &[FormalParam],
        state: &SemanticState<'_>,
    ) -> bool {
        if params.iter().any(|param| param.name.name == name) {
            return false;
        }
        if state.environment.is_bound(name) {
            return self.final_local(name);
        }
        if let Some(facts) = self.enclosing_member(name, state) {
            return has_method(facts, false) || has_method(facts, true) || has_final_field(facts);
        }
        if let Some(identity) = &state.enclosing_type.as_ref().and_then(|ty| match ty {
            ResolvedType::Interface { identity, .. } => Some(identity.clone()),
            _ => None,
        }) {
            let facts = state.signatures.declared_member_facts(identity, name);
            if !facts.is_empty() {
                return has_method(facts, true) || has_final_field(facts);
            }
        }
        self.top_level_function(name)
            || self.final_top_level_variable(name)
            || state.model.resolve_sdk_member(name).is_some()
    }

    /// Whether `receiver` denotes the same object on every evaluation: a class
    /// name, a non-deferred import prefix, or a chain of `final` variables and
    /// fields.
    fn stable_receiver(
        &self,
        receiver: &Expr,
        params: &[FormalParam],
        state: &SemanticState<'_>,
    ) -> bool {
        let mut fields = Vec::new();
        let mut root = receiver;
        while let Expr::Field {
            object,
            field,
            is_null_safe: false,
            ..
        } = root
        {
            fields.push((object.as_ref(), field));
            root = object;
        }
        let Expr::Ident(name) = root else {
            return false;
        };
        if params.iter().any(|param| param.name.name == name.name) {
            return false;
        }
        if state.environment.is_bound(&name.name) {
            if !self.final_local(&name.name) {
                return false;
            }
        } else if self.import_prefix(&name.name).is_some() {
            // Members of other libraries reached through a prefix are not
            // resolved here.
            return fields.is_empty();
        } else if !(state.model.resolve_name(name.name.as_str()).is_some()
            || self
                .enclosing_member(&name.name, state)
                .is_some_and(has_final_field)
            || self.final_top_level_variable(&name.name))
        {
            return false;
        }
        fields.iter().rev().all(|(object, field)| {
            if let Expr::Ident(type_name) = object
                && !state.environment.is_bound(&type_name.name)
                && let Some(identity) = state.model.resolve_name(type_name.name.as_str())
            {
                return has_final_field(
                    state
                        .signatures
                        .declared_member_facts(&identity, &field.name),
                );
            }
            state
                .signatures
                .resolved_member_facts(&state.infer(object), &field.name, &state.model)
                .is_some_and(has_final_field)
        })
    }

    /// Whether `name` invoked on `receiver` is a method or a `final` field.
    fn stable_member(&self, receiver: &Expr, name: &str, state: &SemanticState<'_>) -> bool {
        if let Expr::Ident(type_name) = receiver
            && !state.environment.is_bound(&type_name.name)
        {
            if self.import_prefix(&type_name.name).is_some() {
                return self.prefixed_function(&type_name.name, name, state);
            }
            if let Some(identity) = state.model.resolve_name(type_name.name.as_str()) {
                let facts = state.signatures.declared_member_facts(&identity, name);
                return has_method(facts, true) || has_final_field(facts);
            }
        }
        let receiver_type = state.infer(receiver);
        match state
            .signatures
            .resolved_member_facts(&receiver_type, name, &state.model)
        {
            Some(facts) => has_method(facts, false) || has_final_field(facts),
            // The core collection and async classes expose no function-valued
            // getters, so an invoked member falcon does not index is a method.
            None => matches!(
                receiver_type,
                ResolvedType::Interface {
                    identity: DeclarationIdentity::Sdk {
                        library: "dart:core" | "dart:async" | "dart:collection",
                        ..
                    },
                    ..
                }
            ),
        }
    }

    fn prefixed_function(&self, prefix: &str, name: &str, state: &SemanticState<'_>) -> bool {
        let qualified = falcon_analyze::QualifiedName::prefixed(prefix, name);
        state.model.resolve_sdk_member(qualified).is_some()
            || state
                .model
                .resolve_value(qualified)
                .is_some_and(|identity| state.signatures.function(&identity).is_some())
    }

    fn deferred(&self, receiver: &Expr) -> bool {
        matches!(receiver, Expr::Ident(name)
            if self.import_prefix(&name.name).is_some_and(|import| import.is_deferred))
    }

    /// Whether a method tear-off fits a closure in a typed context: its
    /// parameter types are written or supplied by the context.
    fn parameter_types_known(&self, params: &[FormalParam], state: &SemanticState<'_>) -> bool {
        let contextual = match state.expected() {
            Some(ResolvedType::Function { positional, .. }) => positional.as_slice(),
            _ => &[],
        };
        params.iter().enumerate().all(|(index, param)| {
            param
                .param_type
                .as_ref()
                .is_some_and(|ty| !matches!(ty, DartType::Dynamic { .. }))
                || contextual
                    .get(index)
                    .is_some_and(|ty| !matches!(ty, ResolvedType::Unknown | ResolvedType::Dynamic))
        })
    }

    fn is_unnecessary(&self, node: &Expr, state: &SemanticState<'_>) -> bool {
        let Expr::FuncExpr {
            type_params,
            params,
            async_marker: AsyncMarker::Sync,
            body,
            ..
        } = node
        else {
            return false;
        };
        let params = params.params.as_slice();
        if !type_params.is_empty()
            || params.iter().any(|param| {
                param.kind != ParamKind::Positional
                    || param.initializing.is_some()
                    || param.function_params.is_some()
            })
        {
            return false;
        }
        let Some(invocation) = forwarded_invocation(body) else {
            return false;
        };
        match invocation {
            Expr::New {
                is_const: false,
                dart_type,
                args,
                ..
            } => {
                matches!(body.as_ref(), FunctionBody::Arrow(..))
                    && forwards_parameters(args, params)
                    && !matches!(dart_type, DartType::Named(named) if named.segments.len() > 1
                    && named.segments.first().is_some_and(|prefix| {
                        self.import_prefix(&prefix.name).is_some_and(|import| import.is_deferred)
                    }))
                    && matches!(
                        state.model.resolve_type(dart_type),
                        ResolvedType::Interface { .. }
                    )
            }
            Expr::Call {
                callee,
                type_args,
                args,
                ..
            } => {
                if !forwards_parameters(args, params) {
                    return false;
                }
                if matches!(body.as_ref(), FunctionBody::Arrow(..))
                    && type_args.is_empty()
                    && self.constructor_call(callee, state)
                {
                    return !self.deferred(callee_root(callee));
                }
                if !type_args.is_empty() {
                    return false;
                }
                match callee.as_ref() {
                    Expr::Ident(name) => {
                        if self
                            .typed_contexts
                            .contains(&(node.span().start, node.span().end))
                            && !self.parameter_types_known(params, state)
                        {
                            return false;
                        }
                        self.stable_function(&name.name, params, state)
                    }
                    Expr::Field {
                        object,
                        field,
                        is_null_safe: false,
                        ..
                    } => {
                        !self.deferred(object)
                            && !(self
                                .typed_contexts
                                .contains(&(node.span().start, node.span().end))
                                && !self.parameter_types_known(params, state))
                            && self.stable_receiver(object, params, state)
                            && self.stable_member(object, &field.name, state)
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Whether `callee(...)` invokes a constructor.
    fn constructor_call(&self, callee: &Expr, state: &SemanticState<'_>) -> bool {
        if let Expr::Ident(name) = callee_root(callee)
            && state.environment.is_bound(&name.name)
        {
            return false;
        }
        resolve_call_constructor(callee, &state.model, false).is_some_and(
            |(identity, constructor)| {
                state
                    .signatures
                    .constructor(&identity, constructor)
                    .is_some()
            },
        )
    }
}

/// The leftmost identifier of a dotted callee.
fn callee_root(callee: &Expr) -> &Expr {
    let mut root = callee;
    while let Expr::Field { object, .. } = root {
        root = object;
    }
    root
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, _state: &SemanticState<'_>) {
        if let Stmt::LocalVar(declaration) = node {
            self.typed_contexts
                .extend(initializer_spans(&declaration.declarators));
        }
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if let Expr::Call { args, .. } | Expr::New { args, .. } = node {
            self.typed_contexts.extend(
                args.named
                    .iter()
                    .map(|argument| (argument.value.span().start, argument.value.span().end)),
            );
        }
        if self.is_unnecessary(node, state) {
            self.diags.push(self.ctx.diagnostic(
                "unnecessary-lambdas",
                "Closure should be a tearoff.",
                *node.span(),
            ));
        }
    }
}
