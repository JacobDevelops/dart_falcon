//! Flags a closure that only forwards its parameters to a function a tear-off could name.
//!
//! `names.forEach((name) { print(name); })` allocates a closure whose whole job
//! is to call `print`; `names.forEach(print)` says the same thing. The rule
//! fires on an anonymous, synchronous closure whose body is a single
//! invocation — `=> f(a, b)`, `{ f(a, b); }`, or `{ return f(a, b); }` —
//! passing exactly the closure's positional parameters in order and named
//! parameters under the same names, and on `(a) => C(a)` constructor calls a
//! `C.new` tear-off could replace. Like the analyzer, this does not compare
//! optional parameter defaults. The invoked function must stay the same object
//! between calls:
//!
//! - a top-level, local, static, or instance method, or a `final`, non-`late`
//!   variable or field holding a function (the analyzer does not count a
//!   `const` variable, field, or enum value as final);
//! - reached through a receiver that is a class name, an import prefix, or a
//!   `final` variable or field, never through `this`, a parameter of the
//!   closure, or a null-aware access (`a?.b`). A member falcon cannot resolve
//!   on a stable receiver is taken to be a method, as the analyzer treats any
//!   member that is not a getter.
//!
//! A function or method is itself a stable receiver, including `p.call(x)`.
//! Optional constructor parameters do not prevent a tear-off from replacing
//! a required-parameter closure when the constructor accepts those arguments.
//! An optional closure parameter must stay optional on the target; legacy
//! `@required` annotations do not change that shape. Constructor invocations
//! with named arguments are excluded, matching the analyzer.
//! Optional positional super formals retain their optional shape even when
//! their inherited default value is unavailable.
//!
//! Generic method invocations with explicit type arguments, calls through
//! deferred imports, and `const` constructor calls are skipped. A constructor
//! call only counts in an arrow body, and the constructor tear-off must fit the
//! closure's type, so every closure parameter's type must be known (written or
//! supplied by the context). When the closure is a named argument or a variable
//! initializer, a method tear-off must also fit the closure's type: the
//! parameter types must be known, and a block body that discards the call must
//! be in a `void`-returning context (otherwise the closure returns `Null`).
//!
//! falcon stays silent when it cannot resolve the invoked function or receiver.
//! Constructor calls are only checked when constructor tear-offs are enabled
//! (language version 2.15 or later); before that, `(a) => C(a)` is silent.
//!
//! Adopted from the official `unnecessary_lambdas` lint.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, Feature, MemberFacts, MemberKind, QualifiedName,
    ResolvedType, Rule, SignatureIndex, expression_segments, resolve_call_constructor,
    resolve_new_constructor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_local_var, walk_program, walk_stmt};

use crate::lint::declarations::members_of;
use crate::lint::lexical_usage::{collection_for_inits, local_declarations};
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct UnnecessaryLambdas;

impl Rule for UnnecessaryLambdas {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut stable = StableLocals::default();
        stable.visit_program(program);
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            program,
            locals: local_declarations(program),
            stable_locals: stable.0,
            optional_locals: stable.1,
            late_fields: late_fields(program),
            typed_contexts: declaration_initializers(program),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

/// How invoking a name behaves: a function or method is invoked directly
/// (the analyzer's `MethodInvocation`); a variable or field is read, then its
/// value invoked (`FunctionExpressionInvocation`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Callee {
    Function,
    Variable,
}

/// Declarations (declaring name's span start) of the `final`, non-`late`
/// local variables and parameters and of the local functions in a file. The
/// analyzer does not count a `const` variable as final.
#[derive(Default)]
struct StableLocals(
    HashMap<usize, Callee>,
    HashMap<usize, Vec<(Span, ParamKind)>>,
);

impl StableLocals {
    fn record_optional(&mut self, declaration: Span, params: &FormalParamList) {
        if params.params.iter().any(|param| {
            matches!(
                param.kind,
                ParamKind::OptionalPositional | ParamKind::Named { required: false }
            )
        }) {
            self.1.insert(
                declaration.start,
                params
                    .params
                    .iter()
                    .map(|param| (param.name.span, param.kind))
                    .collect(),
            );
        }
    }

    fn record_for_init(&mut self, init: &ForInit) {
        if let ForInit::ForIn { keyword, name, .. } = init
            && *keyword == VarKeyword::Final
        {
            self.0.insert(name.span.start, Callee::Variable);
        }
    }
}

impl Visitor for StableLocals {
    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        if node.keyword == VarKeyword::Final && !node.is_late {
            for declarator in &node.declarators {
                self.0.insert(declarator.name.span.start, Callee::Variable);
                if let Some(Expr::FuncExpr { params, .. }) =
                    declarator.initializer.as_ref().map(Expr::unparenthesized)
                {
                    self.record_optional(declarator.name.span, params);
                }
            }
        }
        walk_local_var(self, node);
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        if node.is_final {
            self.0.insert(node.name.span.start, Callee::Variable);
        }
        falcon_syntax::visitor::walk_formal_param(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::LocalFunc(function) => {
                self.0.insert(function.name.span.start, Callee::Function);
                self.record_optional(function.name.span, &function.params);
            }
            Stmt::For(statement) => {
                if let Some(init) = &statement.header.init {
                    self.record_for_init(init);
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

fn late_fields(program: &Program) -> HashSet<String> {
    program
        .declarations
        .iter()
        .flat_map(|declaration| members_of(declaration).unwrap_or_default())
        .filter_map(|member| match member {
            ClassMember::Field(field) if field.is_late => Some(&field.declarators),
            _ => None,
        })
        .flatten()
        .map(|declarator| declarator.name.name.clone())
        .collect()
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
    /// The local declaration each identifier expression resolves to.
    locals: HashMap<usize, usize>,
    stable_locals: HashMap<usize, Callee>,
    /// Parameter shape of local callables, before function-type inference erases it.
    optional_locals: HashMap<usize, Vec<(Span, ParamKind)>>,
    /// Names of the `late` fields declared in this file.
    late_fields: HashSet<String>,
    /// Closures that are named arguments or variable initializers; a method
    /// tear-off must be a subtype of their type.
    typed_contexts: HashSet<(usize, usize)>,
}

/// The single invocation a closure body consists of.
fn forwarded_invocation(body: &FunctionBody) -> Option<&Expr> {
    match body {
        FunctionBody::Arrow(expression, _) => Some(expression),
        FunctionBody::Block(block) => match block.stmts.as_slice() {
            [Stmt::Expr(statement)]
                if matches!(statement.expr.unparenthesized(), Expr::Call { .. }) =>
            {
                Some(&statement.expr)
            }
            [
                Stmt::Return(ReturnStmt {
                    value: Some(value), ..
                }),
            ] if matches!(value.unparenthesized(), Expr::Call { .. }) => Some(value),
            _ => None,
        },
        FunctionBody::Native(..) => None,
    }
}

/// Whether `args` forward every parameter once, preserving positional order
/// and named parameter names.
fn forwards_parameters(args: &ArgList, params: &[FormalParam]) -> bool {
    let positional = params
        .iter()
        .filter(|param| !matches!(param.kind, ParamKind::Named { .. }));
    let mut named = params
        .iter()
        .filter(|param| matches!(param.kind, ParamKind::Named { .. }));
    args.positional.len() == positional.clone().count()
        && args.named.len() == named.clone().count()
        && args.positional.iter().zip(positional).all(|(argument, param)| {
            matches!(argument.unparenthesized(), Expr::Ident(name) if name.name == param.name.name)
        })
        && named.all(|param| {
            args.named.iter().any(|argument| {
                argument.name.name == param.name.name
                    && matches!(argument.value.unparenthesized(), Expr::Ident(name)
                        if name.name == param.name.name)
            })
        })
}

/// Whether a closure call `callee(...)` creates an instance.
#[derive(PartialEq, Eq)]
enum Creation {
    Yes,
    No,
    /// A `Type.name(...)` call on a type falcon does not index: a named
    /// constructor or a static method.
    Unknown,
}

impl Collector<'_> {
    /// How a member named `name` with these facts is invoked: a method directly,
    /// a `final` field through its value.
    fn member_callee(
        &self,
        facts: &[MemberFacts],
        name: &str,
        is_static: bool,
        signatures: &SignatureIndex,
    ) -> Option<Callee> {
        if facts
            .iter()
            .any(|fact| fact.kind == MemberKind::Method && fact.is_static == is_static)
        {
            return Some(Callee::Function);
        }
        self.final_field(facts, name, signatures)
            .then_some(Callee::Variable)
    }

    /// Whether the member `name` is a `final`, non-`late` field. The analyzer does
    /// not count a `const` field as final. falcon sees `late` only on fields
    /// declared in this file, and treats every field of that name as late.
    fn final_field(&self, facts: &[MemberFacts], name: &str, signatures: &SignatureIndex) -> bool {
        !facts.is_empty()
            && !self.late_fields.contains(name)
            && facts.iter().all(|fact| {
                matches!(
                    fact.kind,
                    MemberKind::Field {
                        writable: false,
                        ..
                    }
                ) && !(fact.is_static
                    && signatures
                        .declaration(&fact.declaring_type)
                        .is_some_and(|declaration| {
                            declaration
                                .static_consts
                                .iter()
                                .any(|constant| constant.name == name)
                        }))
            })
    }

    /// For an identifier naming a local, whether that local is a stable
    /// `final` variable or local function; `None` when it is not a local.
    fn local(&self, name: &Identifier) -> Option<Option<Callee>> {
        self.locals
            .get(&name.span.start)
            .map(|declaration| self.stable_locals.get(declaration).copied())
    }

    fn top_level_function(&self, name: &str) -> bool {
        self.program.declarations.iter().any(|declaration| {
            matches!(declaration, TopLevelDecl::Function(function)
                if function.name.name == name && matches!(function.kind, FunctionKind::Function(_)))
        })
    }

    /// A `final`, non-`late` top-level variable; the analyzer does not count a
    /// `const` one as final.
    fn final_top_level_variable(&self, name: &str) -> bool {
        self.program.declarations.iter().any(|declaration| {
            matches!(declaration, TopLevelDecl::Variable(variable)
                if variable.keyword == VarKeyword::Final
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

    /// How the unqualified `name` invoked as `name(...)` is called, when it is
    /// a function or method, or a `final` variable or field holding one.
    fn stable_function(&self, name: &Identifier, state: &SemanticState<'_>) -> Option<Callee> {
        if let Some(local) = self.local(name) {
            return local;
        }
        let name = name.name.as_str();
        let signatures = state.signatures.as_ref();
        if let Some(facts) = self.enclosing_member(name, state) {
            return self
                .member_callee(facts, name, false, signatures)
                .or_else(|| self.member_callee(facts, name, true, signatures));
        }
        if let Some(ResolvedType::Interface { identity, .. }) = &state.enclosing_type {
            let facts = signatures.declared_member_facts(identity, name);
            if !facts.is_empty() {
                return self.member_callee(facts, name, true, signatures);
            }
        }
        if self.top_level_function(name) || self.imported_function(name.into(), state) {
            return Some(Callee::Function);
        }
        self.final_top_level_variable(name)
            .then_some(Callee::Variable)
    }

    fn imported_function(&self, name: QualifiedName<'_>, state: &SemanticState<'_>) -> bool {
        state.model.resolve_sdk_member(name).is_some()
            || state
                .model
                .resolve_value(name)
                .is_some_and(|identity| state.signatures.function(&identity).is_some())
    }

    /// A callable declaration, not merely an imported value of callable type.
    fn function_or_method(&self, name: &Identifier, state: &SemanticState<'_>) -> bool {
        self.top_level_function(&name.name)
            || self
                .enclosing_member(&name.name, state)
                .is_some_and(|facts| facts.iter().any(|fact| fact.kind == MemberKind::Method))
            || state
                .model
                .resolve_value(name.name.as_str())
                .or_else(|| state.model.resolve_sdk_member(name.name.as_str()))
                .is_some_and(|identity| state.signatures.function(&identity).is_some())
    }

    /// Whether `receiver` denotes the same object on every evaluation: a class
    /// name, a non-deferred import prefix, or a chain of `final` variables and
    /// fields.
    fn stable_receiver(&self, receiver: &Expr, state: &SemanticState<'_>) -> bool {
        let mut fields = Vec::new();
        let mut root = receiver;
        while let Expr::Field {
            object,
            field,
            is_null_safe: false,
            ..
        } = root.unparenthesized()
        {
            fields.push((object.as_ref(), field));
            root = object;
        }
        let Expr::Ident(name) = root.unparenthesized() else {
            return false;
        };
        let signatures = state.signatures.as_ref();
        if let Some(local) = self.local(name) {
            if local.is_none() {
                return false;
            }
        } else if self.import_prefix(&name.name).is_some() {
            // Members of other libraries reached through a prefix are not
            // resolved here.
            return fields.is_empty();
        } else if !(state.model.resolve_name(name.name.as_str()).is_some()
            || self
                .enclosing_member(&name.name, state)
                .is_some_and(|facts| self.final_field(facts, &name.name, signatures))
            || self.final_top_level_variable(&name.name)
            || self.function_or_method(name, state))
        {
            return false;
        }
        fields.iter().rev().all(|(object, field)| {
            let facts = match self.type_name(object, state) {
                Some(identity) => signatures.declared_member_facts(&identity, &field.name),
                None => signatures
                    .resolved_member_facts(&state.infer(object), &field.name, &state.model)
                    .unwrap_or_default(),
            };
            self.final_field(facts, &field.name, signatures)
        })
    }

    /// The type an identifier expression names, when it is not a local.
    fn type_name(
        &self,
        expression: &Expr,
        state: &SemanticState<'_>,
    ) -> Option<DeclarationIdentity> {
        match expression.unparenthesized() {
            Expr::Ident(name) if self.local(name).is_none() => {
                state.model.resolve_name(name.name.as_str())
            }
            _ => None,
        }
    }

    /// How `name` invoked on the stable `receiver` is called. A member falcon
    /// does not index is taken to be a method.
    fn stable_member(
        &self,
        receiver: &Expr,
        name: &str,
        state: &SemanticState<'_>,
    ) -> Option<Callee> {
        if let Expr::Ident(prefix) = receiver.unparenthesized()
            && self.local(prefix).is_none()
            && self.import_prefix(&prefix.name).is_some()
        {
            return Some(Callee::Function);
        }
        let signatures = state.signatures.as_ref();
        if let Some(identity) = self.type_name(receiver, state) {
            let facts = signatures.declared_member_facts(&identity, name);
            return if facts.is_empty() {
                Some(Callee::Function)
            } else {
                self.member_callee(facts, name, true, signatures)
            };
        }
        match signatures.resolved_member_facts(&state.infer(receiver), name, &state.model) {
            Some(facts) => self.member_callee(facts, name, false, signatures),
            None => Some(Callee::Function),
        }
    }

    fn deferred(&self, receiver: &Expr) -> bool {
        matches!(receiver.unparenthesized(), Expr::Ident(name)
            if self.import_prefix(&name.name).is_some_and(|import| import.is_deferred))
    }

    /// Whether every closure parameter's type is known: written, or supplied
    /// by the context.
    fn parameter_types_known(&self, params: &[FormalParam], state: &SemanticState<'_>) -> bool {
        let (contextual, named) = match state.expected() {
            Some(ResolvedType::Function {
                positional, named, ..
            }) => (positional.as_ref(), Some(named)),
            _ => (&[][..], None),
        };
        let mut positional_index = 0;
        params.iter().all(|param| {
            let contextual_type = if matches!(param.kind, ParamKind::Named { .. }) {
                named.and_then(|named| {
                    named
                        .iter()
                        .find(|(name, _)| name == &param.name.name)
                        .map(|(_, ty)| ty)
                })
            } else {
                let ty = contextual.get(positional_index);
                positional_index += 1;
                ty
            };
            param
                .param_type
                .as_ref()
                .is_some_and(|ty| !matches!(ty, DartType::Dynamic { .. }))
                || contextual_type
                    .is_some_and(|ty| !matches!(ty, ResolvedType::Unknown | ResolvedType::Dynamic))
        })
    }

    /// Whether a method tear-off fits the closure `node`'s own type when the
    /// closure is a named argument or a variable initializer.
    fn fits_typed_context(
        &self,
        node: &Expr,
        params: &[FormalParam],
        body: &FunctionBody,
        state: &SemanticState<'_>,
    ) -> bool {
        if !self
            .typed_contexts
            .contains(&(node.span().start, node.span().end))
        {
            return true;
        }
        if !self.parameter_types_known(params, state) {
            return false;
        }
        // A block body that discards the call returns `void` only under a
        // `void`-returning context; otherwise it returns `Null`, which no
        // forwarded method's return type is a subtype of.
        let discards = matches!(body, FunctionBody::Block(block)
            if matches!(block.stmts.as_slice(), [Stmt::Expr(_)]));
        !discards
            || matches!(state.expected(), Some(ResolvedType::Function { return_type, .. })
                if **return_type == ResolvedType::Void)
    }

    /// Whether `callee(...)` invokes a constructor.
    fn creation(&self, callee: &Expr, state: &SemanticState<'_>) -> Creation {
        if let Expr::Ident(name) = callee_root(callee).unparenthesized()
            && self.local(name).is_some()
        {
            return Creation::No;
        }
        let Some((identity, constructor)) = resolve_call_constructor(callee, &state.model, true)
        else {
            return Creation::No;
        };
        // `C(...)`, `p.C(...)`, and `C<T>.name(...)` name a constructor.
        if constructor == "new"
            || expression_segments(callee, false).is_none()
            || state
                .signatures
                .constructor(&identity, constructor)
                .is_some()
        {
            return Creation::Yes;
        }
        if !state
            .signatures
            .declared_member_facts(&identity, constructor)
            .is_empty()
        {
            return Creation::No;
        }
        match state.signatures.declaration(&identity) {
            Some(facts) if facts.constructors.iter().any(|c| c.name == constructor) => {
                Creation::Yes
            }
            Some(_) => Creation::No,
            None => Creation::Unknown,
        }
    }

    /// A tear-off cannot make one of the wrapper's optional parameters required.
    fn optional_parameters_fit(
        &self,
        invocation: &Expr,
        params: &[FormalParam],
        state: &SemanticState<'_>,
    ) -> bool {
        if params.iter().all(|param| {
            matches!(
                param.kind,
                ParamKind::Positional | ParamKind::Named { required: true }
            )
        }) {
            return true;
        }
        if let Expr::Call { callee, .. } = invocation.unparenthesized()
            && let Expr::Ident(name) = callee.unparenthesized()
            && let Some(shape) = self
                .locals
                .get(&name.span.start)
                .and_then(|declaration| self.optional_locals.get(declaration))
        {
            let mut positional = shape
                .iter()
                .filter(|(_, kind)| !matches!(kind, ParamKind::Named { .. }));
            return params.iter().all(|param| match param.kind {
                ParamKind::Named { required: true } => true,
                ParamKind::Named { required: false } => shape.iter().any(|(span, kind)| {
                    matches!(kind, ParamKind::Named { required: false })
                        && self.ctx.source.get(span.start..span.end)
                            == Some(param.name.name.as_str())
                }),
                kind => {
                    let target = positional.next();
                    kind == ParamKind::Positional
                        || target.is_some_and(|(_, kind)| *kind == ParamKind::OptionalPositional)
                }
            });
        }
        let signature = match invocation.unparenthesized() {
            Expr::Call {
                callee,
                type_args,
                args,
                ..
            } => state
                .call_signature(callee, type_args, args, None)
                .map(Cow::Owned),
            Expr::New {
                dart_type,
                constructor_name,
                ..
            } => {
                let Some((identity, constructor)) =
                    resolve_new_constructor(dart_type, constructor_name.as_ref(), &state.model)
                else {
                    return false;
                };
                state
                    .signatures
                    .constructor(&identity, constructor)
                    .map(Cow::Borrowed)
            }
            _ => None,
        };
        let Some(signature) = signature else {
            return false;
        };
        let mut positional = 0;
        params.iter().all(|param| {
            let name = match param.kind {
                ParamKind::Named { required: true } => return true,
                ParamKind::Named { required: false } => Some(param.name.name.as_str()),
                ParamKind::Positional => {
                    positional += 1;
                    return true;
                }
                ParamKind::OptionalPositional => {
                    let name = signature.positional_parameter_names.get(positional);
                    positional += 1;
                    // Optional super formals can lack a resolved inherited
                    // default, but their binding slot still records the shape.
                    return name.is_some_and(|name| !name.is_empty());
                }
            };
            // The signature records implicit null defaults for optional slots.
            // `non_optional_defaults` also includes legacy `@required`, which
            // does not make a named parameter required in its function type.
            name.is_some_and(|name| signature.defaults.contains_key(name))
        })
    }

    fn is_unnecessary(&self, node: &Expr, state: &SemanticState<'_>) -> bool {
        let Expr::FuncExpr {
            type_params,
            params,
            async_marker: AsyncMarker::Sync,
            body,
            ..
        } = node.unparenthesized()
        else {
            return false;
        };
        let params = params.params.as_slice();
        if !type_params.is_empty()
            || params
                .iter()
                .any(|param| param.initializing.is_some() || param.function_params.is_some())
        {
            return false;
        }
        let Some(invocation) = forwarded_invocation(body) else {
            return false;
        };
        if !self.optional_parameters_fit(invocation, params, state) {
            return false;
        }
        let arrow = matches!(body.as_ref(), FunctionBody::Arrow(..));
        // A constructor tear-off must exist in the file's language version and
        // fit the closure's type.
        let creation_fits = || {
            arrow
                && self
                    .ctx
                    .language_version()
                    .supports(Feature::ConstructorTearoffs)
                && self.parameter_types_known(params, state)
        };
        match invocation.unparenthesized() {
            Expr::New {
                is_const: false,
                dart_type,
                constructor_name,
                args,
                ..
            } => {
                forwards_parameters(args, params)
                    && args.named.is_empty()
                    && !matches!(dart_type, DartType::Named(named) if named.segments.len() > 1
                    && named.segments.first().is_some_and(|prefix| {
                        self.import_prefix(&prefix.name).is_some_and(|import| import.is_deferred)
                    }))
                    && resolve_new_constructor(dart_type, constructor_name.as_ref(), &state.model)
                        .is_some()
                    && creation_fits()
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
                match self.creation(callee, state) {
                    Creation::Yes | Creation::Unknown => {
                        return args.named.is_empty()
                            && !self.deferred(callee_root(callee))
                            && creation_fits();
                    }
                    Creation::No => {}
                }
                let callee_kind = match callee.unparenthesized() {
                    Expr::Ident(name) => self.stable_function(name, state),
                    Expr::FuncExpr { .. } => Some(Callee::Function),
                    Expr::Field {
                        object,
                        field,
                        is_null_safe: false,
                        ..
                    } if !self.deferred(object) && self.stable_receiver(object, state) => {
                        self.stable_member(object, &field.name, state)
                    }
                    _ => None,
                };
                match callee_kind {
                    Some(Callee::Variable) => true,
                    Some(Callee::Function) => {
                        type_args.is_empty() && self.fits_typed_context(node, params, body, state)
                    }
                    None => false,
                }
            }
            _ => false,
        }
    }
}

/// The leftmost identifier of a dotted callee.
fn callee_root(callee: &Expr) -> &Expr {
    let mut root = callee;
    while let Expr::Field { object, .. } | Expr::GenericInstantiation { target: object, .. } =
        root.unparenthesized()
    {
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
        if matches!(node, Expr::Paren { .. }) {
            return;
        }
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
