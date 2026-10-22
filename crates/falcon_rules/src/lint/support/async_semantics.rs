//! Semantic facts for the structural async-state visitor.
//!
//! Mirrors SDK 3.13.5's declaration-based BuildContext classification through
//! the shared semantic visitor. Assignments do not change a binding's declared
//! context type. Getter-backed fields exclude nullable declarations; local
//! variables and parameters do not. Boolean getter identities distinguish
//! Flutter State from BuildContext, but cannot represent custom State getter
//! overrides separately.
//! Generic constructor type arguments do not change protected callback identity.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, ConstantCache, ConstantValue, DeclarationIdentity, ResolvedType, Truth,
    evaluate_constant_cached, expression_segments, resolve_call_constructor,
};
use falcon_syntax::cst::visit::{Visitor, walk_expr};
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub(crate) struct Semantics {
    pub uses: HashMap<NodeId, bool>,
    pub mounted: HashMap<NodeId, bool>,
    pub protected_callbacks: HashSet<NodeId>,
    pub constants: HashMap<NodeId, bool>,
    pub never: HashSet<NodeId>,
    pub message: &'static str,
}

pub(crate) fn prepare<'a>(
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
) -> Option<Collector<'a>> {
    let state = SemanticState::new(program, ctx)?;
    let global_count = program
        .declarations()
        .map(|declaration| match declaration {
            TopLevelDecl::Variable(variable) => variable.declarators().len(),
            TopLevelDecl::Function(function) if matches!(function.kind(), FunctionKind::Getter) => {
                1
            }
            _ => 0,
        })
        .sum();
    let mut collector = Collector {
        program,
        ctx,
        setstate: false,
        semantics: Semantics::default(),
        globals: HashMap::with_capacity(global_count),
    };
    collector.semantics.message =
        "Do not use a `BuildContext` across an `async` gap without a matching `mounted` check.";
    for declaration in program.declarations() {
        match declaration {
            TopLevelDecl::Variable(variable) => {
                let ty = variable.var_type().map(|ty| state.model.resolve_type(ty));
                for declarator in variable.declarators() {
                    if let Some(ty) = ty
                        .as_ref()
                        .cloned()
                        .or_else(|| declarator.initializer().map(|value| state.infer(value)))
                    {
                        collector.globals.insert(declarator.name().name(), ty);
                    }
                }
            }
            TopLevelDecl::Function(function) if matches!(function.kind(), FunctionKind::Getter) => {
                if let Some(ty) = function.return_type() {
                    collector
                        .globals
                        .insert(function.name().name(), state.model.resolve_type(ty));
                }
            }
            _ => {}
        }
    }
    Some(collector)
}

pub(super) fn prepare_setstate<'a>(
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
) -> Option<Collector<'a>> {
    let mut collector = prepare(program, ctx)?;
    collector.setstate = true;
    collector.semantics.message =
        "Do not call `setState` across an `async` gap without checking `mounted`.";
    Some(collector)
}

pub(crate) struct Collector<'a> {
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    semantics: Semantics,
    globals: HashMap<&'a str, ResolvedType>,
    setstate: bool,
}

#[cfg(test)]
fn collect<'a>(program: Program<'a>, ctx: &'a AnalyzeContext<'a>) -> Semantics {
    let Some(mut collector) = prepare(program, ctx) else {
        return Semantics::default();
    };
    let Some(state) = SemanticState::new(program, ctx) else {
        return Semantics::default();
    };
    falcon_analyze::semantic_scope::visit_program(&mut collector, program, state);
    collector.semantics
}

fn key(expression: Expr<'_>) -> NodeId {
    expression.syntax().id()
}

pub(crate) fn mounted_kind(receiver: &ResolvedType, state: &SemanticState<'_>) -> Option<bool> {
    if let Some(owner) = state
        .signatures
        .resolved_member_owner(receiver, "mounted", &state.model)
    {
        return if owner.is_package("flutter", "State") {
            Some(true)
        } else if owner.is_package("flutter", "BuildContext") {
            Some(false)
        } else {
            None
        };
    }
    // External Flutter classes have canonical identities even when their
    // member signatures are not indexed. Resolved overrides never use this.
    if is_context(receiver, false) {
        Some(false)
    } else if state.signatures.is_subtype_of(
        receiver,
        &DeclarationIdentity::flutter("State"),
        &state.model,
    ) == Truth::Yes
    {
        Some(true)
    } else {
        None
    }
}

/// Use declaration types, not assignment values or flow promotions. The SDK
/// recognizes identifiers/property reads (plus parentheses and `!`), not calls
/// which happen to return a context.
fn context_kind<'a>(
    expression: Expr<'a>,
    state: &SemanticState<'a>,
    globals: &HashMap<&str, ResolvedType>,
) -> Option<bool> {
    match expression {
        Expr::Paren(paren) => context_kind(paren.expr().unparenthesized(), state, globals),
        Expr::NullAssert(assertion) => context_kind(assertion.operand(), state, globals),
        Expr::Ident(identifier) => {
            if state.is_own_field(identifier.name()) {
                context_member_kind(state.enclosing_type.as_ref()?, identifier.name(), state)
            } else if state.environment.is_bound(identifier.name()) {
                context_type_kind(
                    &state.environment.declared_type(identifier.name()),
                    false,
                    state,
                )
            } else {
                if let Some(ty) = globals.get(identifier.name()) {
                    return context_type_kind(ty, true, state);
                }
                if let Some(receiver) = &state.enclosing_type {
                    let kind = context_member_kind(receiver, identifier.name(), state);
                    if kind.is_some()
                        || state
                            .signatures
                            .resolved_field(receiver, identifier.name(), &state.model)
                            .is_some()
                    {
                        return kind;
                    }
                }
                globals
                    .get(identifier.name())
                    .and_then(|ty| context_type_kind(ty, true, state))
                    .or_else(|| {
                        // Inference reports a promoted implicit-`this` field
                        // read; its declaration was classified above.
                        if state
                            .environment
                            .promoted_this_field(identifier.name())
                            .is_some()
                        {
                            return None;
                        }
                        context_type_kind(&state.infer(expression), true, state)
                    })
            }
        }
        Expr::Field(field) => {
            let receiver = receiver_type(field.object(), state, globals);
            // Only the getter declaration establishes a context use. An
            // inferred access type may substitute T with BuildContext.
            context_member_kind(&receiver, field.field().name(), state)
        }
        Expr::IntLit(_)
        | Expr::DoubleLit(_)
        | Expr::StringLit(_)
        | Expr::BoolLit(_)
        | Expr::NullLit(_)
        | Expr::This(_)
        | Expr::Super(_)
        | Expr::Unary(_)
        | Expr::PostfixIncDec(_)
        | Expr::Binary(_)
        | Expr::Assign(_)
        | Expr::Conditional(_)
        | Expr::Is(_)
        | Expr::As(_)
        | Expr::Index(_)
        | Expr::Call(_)
        | Expr::Cascade(_)
        | Expr::List(_)
        | Expr::Map(_)
        | Expr::Set(_)
        | Expr::Record(_)
        | Expr::FuncExpr(_)
        | Expr::New(_)
        | Expr::DotShorthand(_)
        | Expr::Await(_)
        | Expr::Throw(_)
        | Expr::Switch(_)
        | Expr::SymbolLit(_)
        | Expr::GenericInstantiation(_)
        | Expr::Error(_) => None,
    }
}

fn context_member_kind(
    receiver: &ResolvedType,
    name: &str,
    state: &SemanticState<'_>,
) -> Option<bool> {
    let owner = state
        .signatures
        .resolved_member_owner(receiver, name, &state.model);
    if owner.is_none()
        && name == "context"
        && state.signatures.is_subtype_of(
            receiver,
            &DeclarationIdentity::flutter("State"),
            &state.model,
        ) == Truth::Yes
    {
        return Some(true);
    }
    // The official visitor inspects the getter declaration, not the member's
    // substituted access type. A getter declared as T is not BuildContext.
    let (ty, _) = state
        .signatures
        .resolved_field(receiver, name, &state.model)
        .or_else(|| {
            state
                .signatures
                .resolved_extension_getter(receiver, name, &state.model)
        })?;
    if !is_context(&ty, true) {
        return None;
    }
    if let Some(owner) = owner {
        let declaring_type = ResolvedType::interface_identity(owner);
        if state.signatures.is_subtype_of(
            &declaring_type,
            &DeclarationIdentity::flutter("State"),
            &state.model,
        ) == Truth::Yes
        {
            // State fields use their declaring class's mounted getter, not
            // the getter on the BuildContext value or its accessing receiver.
            return Some(true);
        }
    }
    context_type_kind(&ty, true, state)
}

fn is_context(ty: &ResolvedType, skip_nullable: bool) -> bool {
    matches!(ty, ResolvedType::Interface { identity, nullable, .. }
        if identity.is_package("flutter", "BuildContext")
            && (!skip_nullable || !nullable))
}

fn context_type_kind(
    ty: &ResolvedType,
    skip_nullable: bool,
    state: &SemanticState<'_>,
) -> Option<bool> {
    is_context(ty, skip_nullable)
        .then(|| mounted_kind(ty, state))
        .flatten()
}

/// Folds `expression` to a boolean through the file's shared constant cache,
/// so repeated references to one library constant evaluate it once.
pub(crate) fn constant_bool<'a>(
    expression: Expr<'a>,
    state: &SemanticState<'a>,
    cache: &ConstantCache,
) -> Option<bool> {
    if let Expr::BoolLit(literal) = expression.unparenthesized() {
        return Some(literal.value());
    }
    if !matches!(
        expression.unparenthesized(),
        Expr::Ident(_) | Expr::Field(_) | Expr::Unary(_) | Expr::Binary(_) | Expr::Conditional(_)
    ) {
        return None;
    }
    // The shared evaluator resolves library constants, not lexical bindings.
    // Do not accidentally fold a shadowed library constant as a local value.
    struct BoundNames<'a, 'model> {
        state: &'a SemanticState<'model>,
        found: bool,
    }
    impl<'a> Visitor<'a> for BoundNames<'_, '_> {
        fn visit_expr(&mut self, expression: Expr<'a>) {
            if let Expr::Ident(identifier) = expression {
                self.found |= self.state.environment.is_bound(identifier.name());
            } else if !self.found {
                walk_expr(self, expression);
            }
        }
    }
    let mut names = BoundNames {
        state,
        found: false,
    };
    names.visit_expr(expression);
    if names.found {
        return None;
    }
    match evaluate_constant_cached(expression, &state.model, &state.signatures, cache) {
        Some(ConstantValue::Bool(value)) => Some(value),
        _ => None,
    }
}

/// The operands of an `==` or `!=` comparison, the only expressions whose
/// constant value async-gap analysis consults.
///
/// Collectors fold just these operands. Folding every visited expression
/// re-evaluates a library constant's initializer chain at each reference,
/// which is quadratic in the length of `const` alias chains.
pub(crate) fn equality_operands(expression: Expr<'_>) -> Option<[Expr<'_>; 2]> {
    match expression {
        Expr::Binary(binary) if matches!(binary.op(), BinaryOp::EqEq | BinaryOp::NotEq) => {
            Some([binary.left(), binary.right()])
        }
        _ => None,
    }
}

fn receiver_type<'a>(
    expression: Expr<'a>,
    state: &SemanticState<'a>,
    globals: &HashMap<&str, ResolvedType>,
) -> ResolvedType {
    if let Expr::Ident(identifier) = expression.unparenthesized()
        && !state.environment.is_bound(identifier.name())
        && let Some(ty) = globals.get(identifier.name())
    {
        return ty.clone();
    }
    let inferred = state.infer_receiver(expression);
    if inferred == ResolvedType::Unknown && context_kind(expression, state, globals).is_some() {
        // A State.context declaration can be known even when external field
        // signatures are absent. Reading its mounted member still resolves
        // BuildContext.mounted, not the State getter associated with its use.
        ResolvedType::interface_identity(DeclarationIdentity::flutter("BuildContext"))
    } else {
        inferred
    }
}

pub(crate) fn mounted_receiver_kind<'a>(
    expression: Expr<'a>,
    state: &SemanticState<'a>,
) -> Option<bool> {
    mounted_kind(&receiver_type(expression, state, &HashMap::new()), state)
}

impl<'a> Collector<'a> {
    fn use_context(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        if !self.setstate
            && let Some(kind) = context_kind(expression, state, &self.globals)
        {
            self.semantics.uses.insert(key(expression), kind);
        }
    }

    fn arguments(&mut self, args: ArgList<'a>, state: &SemanticState<'a>) {
        for expression in args.positional() {
            self.use_context(expression, state);
        }
        for argument in args.named() {
            self.use_context(argument.value(), state);
        }
    }

    fn callbacks(&mut self, args: ArgList<'a>, positional: &[usize], named: &[&str]) {
        for &position in positional {
            if let Some(expression @ Expr::FuncExpr(_)) =
                args.positional().nth(position).map(Expr::unparenthesized)
            {
                self.semantics.protected_callbacks.insert(key(expression));
            }
        }
        for argument in args.named() {
            if named.contains(&argument.name().name())
                && matches!(argument.value().unparenthesized(), Expr::FuncExpr(_))
            {
                self.semantics
                    .protected_callbacks
                    .insert(key(argument.value()));
            }
        }
    }

    fn constructor_callbacks(
        &mut self,
        identity: &DeclarationIdentity,
        constructor: &str,
        args: ArgList<'a>,
    ) {
        if !identity.is_sdk("dart:async", identity.name()) {
            return;
        }
        if self.setstate && identity.name() != "Future" {
            return;
        }
        match (identity.name(), constructor) {
            ("Future", "new" | "" | "microtask") => self.callbacks(args, &[0], &[]),
            ("Future", "delayed") => self.callbacks(args, &[1], &[]),
            ("Stream", "eventTransformed" | "periodic") => self.callbacks(args, &[1], &[]),
            ("Stream", "multi") => self.callbacks(args, &[0], &[]),
            ("StreamController", "new" | "") => {
                self.callbacks(args, &[], &["onListen", "onPause", "onResume", "onCancel"]);
            }
            ("StreamController", "broadcast") => {
                self.callbacks(args, &[], &["onListen", "onCancel"]);
            }
            _ => {}
        }
    }

    fn method_callbacks(&mut self, callee: Expr<'a>, args: ArgList<'a>, state: &SemanticState<'a>) {
        let Expr::Field(field) = callee.unparenthesized() else {
            return;
        };
        let object = field.object();
        // The SDK's method catalog deliberately matches the class name, unlike
        // its constructor catalog, which also requires dart:async identity.
        let static_class = expression_segments(object, false).and_then(|segments| {
            (!state.environment.is_bound(segments[0]) && !self.globals.contains_key(segments[0]))
                .then(|| state.model.resolve_dotted(&segments))
                .flatten()
        });
        if let Some(identity) = static_class {
            if self.setstate && !identity.is_sdk("dart:async", "Future") {
                return;
            }
            match (identity.name(), field.field().name()) {
                ("Future", "doWhile") => self.callbacks(args, &[0], &[]),
                ("Future", "forEach") => self.callbacks(args, &[1], &[]),
                ("Future", "wait") => self.callbacks(args, &[], &["cleanUp"]),
                _ => {}
            }
            return;
        }
        let receiver = receiver_type(object, state, &self.globals);
        self.instance_callbacks(&receiver, field.field().name(), args);
    }

    fn instance_callbacks(&mut self, receiver: &ResolvedType, method: &str, args: ArgList<'a>) {
        let ResolvedType::Interface { identity, .. } = receiver else {
            return;
        };
        if self.setstate && !identity.is_sdk("dart:async", "Future") {
            return;
        }
        match (identity.name(), method) {
            ("Future", "catchError" | "onError") => self.callbacks(args, &[0], &["test"]),
            ("Future", "then") => self.callbacks(args, &[0], &["onError"]),
            ("Future", "timeout") | ("Stream", "timeout") => {
                self.callbacks(args, &[], &["onTimeout"]);
            }
            ("Future", "whenComplete") => self.callbacks(args, &[0], &[]),
            ("Stream", "asBroadcastStream") => {
                self.callbacks(args, &[], &["onListen", "onCancel"]);
            }
            ("Stream", "firstWhere" | "lastWhere" | "singleWhere") => {
                self.callbacks(args, &[0], &["orElse"]);
            }
            ("Stream", "handleError") => self.callbacks(args, &[0], &["test"]),
            ("Stream", "listen") => self.callbacks(args, &[0], &["onError", "onDone"]),
            ("Stream", "fold") => self.callbacks(args, &[1], &[]),
            (
                "Stream",
                "any" | "asyncExpand" | "asyncMap" | "distinct" | "expand" | "forEach" | "map"
                | "reduce" | "skipWhile" | "takeWhile" | "where",
            )
            | ("StreamSubscription", "onData" | "onDone" | "onError") => {
                self.callbacks(args, &[0], &[]);
            }
            _ => {}
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<falcon_diagnostics::Diagnostic> {
        super::async_state::analyze_semantics(self.program, self.ctx, self.semantics)
    }

    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        if matches!(expression, Expr::Paren(_)) {
            return;
        }
        if matches!(expression, Expr::Ident(_) | Expr::Field(_) | Expr::Call(_))
            && state.infer(expression) == ResolvedType::Never
        {
            self.semantics.never.insert(key(expression));
        }
        for operand in equality_operands(expression).into_iter().flatten() {
            let operand = operand.unparenthesized();
            if let Some(value) = constant_bool(operand, state, self.ctx.constant_cache()) {
                self.semantics.constants.insert(key(operand), value);
            }
        }
        match expression {
            Expr::Ident(identifier)
                if identifier.name() == "mounted"
                    && !state.environment.is_bound(identifier.name()) =>
            {
                if let Some(kind) = state
                    .enclosing_type
                    .as_ref()
                    .and_then(|receiver| mounted_kind(receiver, state))
                {
                    self.semantics.mounted.insert(key(expression), kind);
                }
            }
            Expr::Field(field) => {
                let object = field.object();
                if field.field().name() == "mounted" {
                    if let Some(kind) =
                        mounted_kind(&receiver_type(object, state, &self.globals), state)
                    {
                        let kind = kind
                            || (self.setstate
                                && context_kind(object, state, &self.globals) == Some(true));
                        self.semantics.mounted.insert(key(expression), kind);
                    }
                } else if !field.is_null_safe()
                    && matches!(object.unparenthesized(), Expr::Ident(_))
                {
                    self.use_context(object, state);
                }
            }
            Expr::Call(call) => {
                let callee = call.callee();
                let args = call.args();
                if self.setstate && super::flutter_state::is_setstate(call, state) {
                    self.semantics.uses.insert(key(expression), true);
                }
                self.arguments(args, state);
                if let Expr::Field(field) = callee.unparenthesized()
                    && (is_context(&receiver_type(field.object(), state, &self.globals), true)
                        || context_kind(field.object(), state, &self.globals) == Some(true))
                {
                    self.use_context(field.object(), state);
                }
                let constructor = expression_segments(callee, true)
                    .filter(|segments| {
                        !state.environment.is_bound(segments[0])
                            && !self.globals.contains_key(segments[0])
                    })
                    .and_then(|_| resolve_call_constructor(callee, &state.model, true));
                if let Some((identity, name)) = constructor {
                    self.constructor_callbacks(&identity, name, args);
                }
                self.method_callbacks(callee, args, state);
            }
            Expr::Cascade(cascade) => {
                let base = receiver_type(cascade.object(), state, &self.globals);
                for section in cascade.sections() {
                    let mut receiver = Cow::Borrowed(&base);
                    let mut operations = section.ops().peekable();
                    while let Some(operation) = operations.next() {
                        match operation {
                            CascadeOp::NullAssert(_) => {
                                receiver = Cow::Owned(receiver.non_null());
                            }
                            CascadeOp::Call(call) => {
                                self.arguments(call.args(), state);
                                self.instance_callbacks(&receiver, call.name().name(), call.args());
                                if operations.peek().is_some() {
                                    receiver = Cow::Owned(
                                        state
                                            .signatures
                                            .resolved_member(
                                                &receiver,
                                                call.name().name(),
                                                &state.model,
                                            )
                                            .map(|(signature, substitutions)| {
                                                state.model.substitute(
                                                    &signature.return_type,
                                                    &substitutions,
                                                )
                                            })
                                            .unwrap_or(ResolvedType::Unknown),
                                    );
                                }
                            }
                            CascadeOp::Invoke(invoke) => {
                                self.arguments(invoke.args(), state);
                                if operations.peek().is_some() {
                                    receiver = Cow::Owned(state.cascade_invoke_return_type(
                                        &receiver,
                                        invoke.type_args(),
                                        invoke.args(),
                                    ));
                                }
                            }
                            CascadeOp::Field(field) => {
                                if operations.peek().is_some() {
                                    receiver = Cow::Owned(
                                        state
                                            .signatures
                                            .resolved_field(
                                                &receiver,
                                                field.name().name(),
                                                &state.model,
                                            )
                                            .or_else(|| {
                                                state.signatures.resolved_extension_getter(
                                                    &receiver,
                                                    field.name().name(),
                                                    &state.model,
                                                )
                                            })
                                            .map(|(ty, substitutions)| {
                                                state.model.substitute(&ty, &substitutions)
                                            })
                                            .unwrap_or(ResolvedType::Unknown),
                                    );
                                }
                            }
                            CascadeOp::Index(_) => {
                                receiver = Cow::Owned(
                                    state
                                        .signatures
                                        .resolved_member(&receiver, "[]", &state.model)
                                        .map(|(signature, substitutions)| {
                                            state
                                                .model
                                                .substitute(&signature.return_type, &substitutions)
                                        })
                                        .unwrap_or(ResolvedType::Unknown),
                                );
                            }
                            CascadeOp::Assign(..) | CascadeOp::IndexAssign(..) => {}
                        }
                    }
                }
            }
            Expr::New(new) => {
                let args = new.args();
                self.arguments(args, state);
                if let ResolvedType::Interface { identity, .. } = state
                    .model
                    .resolve_type_in(new.dart_type(), &state.type_parameters)
                {
                    self.constructor_callbacks(
                        &identity,
                        new.constructor_name().map_or("new", |name| name.name()),
                        args,
                    );
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
use crate::inference;

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::{Rule, RulePreparation};
    use falcon_config::FalconConfig;
    use std::path::Path;

    struct CheckProtectedCallbacks;

    impl Rule for CheckProtectedCallbacks {
        fn prepare<'a>(
            &self,
            program: Program<'a>,
            ctx: &'a AnalyzeContext<'a>,
        ) -> RulePreparation<'a> {
            assert_eq!(collect(program, ctx).protected_callbacks.len(), 3);
            RulePreparation::Diagnostics(Vec::new())
        }
    }

    #[test]
    fn generic_future_constructors_protect_callbacks() {
        let source = r"
import 'dart:async' as a;
void callbacks() {
  Future<void>.delayed(Duration.zero, () {});
  Future<void>.microtask(() {});
  a.Future<void>.microtask(() {});
}
";
        inference::analyze(
            "check-protected-callbacks",
            &CheckProtectedCallbacks,
            Path::new("generic_future_callbacks.dart"),
            source,
            &FalconConfig::default(),
        );
    }

    struct CheckCascadeCallbacks;

    impl Rule for CheckCascadeCallbacks {
        fn prepare<'a>(
            &self,
            program: Program<'a>,
            ctx: &'a AnalyzeContext<'a>,
        ) -> RulePreparation<'a> {
            let semantics = collect(program, ctx);
            assert_eq!(semantics.protected_callbacks.len(), 2);
            assert_eq!(semantics.uses.len(), 2);
            RulePreparation::Diagnostics(Vec::new())
        }
    }

    #[test]
    fn invoked_and_indexed_cascade_results_keep_callback_and_argument_types() {
        let source = r"
import 'dart:async';
import 'package:flutter/widgets.dart';
class Holder {
  Future<void> Function(BuildContext) factory() => (context) async {};
  Future<void> Function(BuildContext) operator [](int index) => (context) async {};
}
void use(Holder holder, BuildContext context) {
  holder..factory()(context).then((value) {});
  holder..[0](context).then((value) {});
}
";
        inference::analyze(
            "check-cascade-callbacks",
            &CheckCascadeCallbacks,
            Path::new("cascade_callbacks.dart"),
            source,
            &FalconConfig::default(),
        );
    }

    struct CheckEqualityConstants;

    impl Rule for CheckEqualityConstants {
        fn prepare<'a>(
            &self,
            program: Program<'a>,
            ctx: &'a AnalyzeContext<'a>,
        ) -> RulePreparation<'a> {
            // Comparison operands fold; alias initializers, the negation, the
            // comparisons themselves and the shadowing parameter do not. The
            // second walk reads the file's warm constant cache.
            for _ in 0..2 {
                let mut values: Vec<_> = collect(program, ctx).constants.into_values().collect();
                values.sort_unstable();
                assert_eq!(
                    values,
                    [false, true, true, true, true, true, true, true, true]
                );
            }
            RulePreparation::Diagnostics(Vec::new())
        }
    }

    #[test]
    fn constants_are_folded_only_for_equality_operands() {
        let source = r"
const base = false;
const negated = !base;
const aliased = negated;
bool compare() => aliased == false;
bool again() => aliased != true;
bool shadowed(bool aliased) => aliased == true;
bool parenthesized() => aliased == (negated);
bool nestedParenthesized() => aliased == ((negated));
";
        inference::analyze(
            "check-equality-constants",
            &CheckEqualityConstants,
            Path::new("equality_constants.dart"),
            source,
            &FalconConfig::default(),
        );
    }
}
