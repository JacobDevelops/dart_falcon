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
    AnalyzeContext, ConstantValue, DeclarationIdentity, ResolvedType, Truth, evaluate_constant,
    expression_segments, resolve_call_constructor,
};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

use crate::lint::semantic_scope::{self, SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub(super) struct Semantics {
    pub uses: HashMap<usize, bool>,
    pub mounted: HashMap<usize, bool>,
    pub protected_callbacks: HashSet<usize>,
    pub constants: HashMap<usize, bool>,
    pub never: HashSet<usize>,
}

pub(super) fn collect(program: &Program, ctx: &AnalyzeContext<'_>) -> Semantics {
    let Some(state) = SemanticState::new(program, ctx) else {
        return Semantics::default();
    };
    let global_count = program
        .declarations
        .iter()
        .map(|declaration| match declaration {
            TopLevelDecl::Variable(variable) => variable.declarators.len(),
            TopLevelDecl::Function(function) if matches!(function.kind, FunctionKind::Getter) => 1,
            _ => 0,
        })
        .sum();
    let mut collector = Collector {
        semantics: Semantics::default(),
        globals: HashMap::with_capacity(global_count),
    };
    for declaration in &program.declarations {
        match declaration {
            TopLevelDecl::Variable(variable) => {
                let ty = variable
                    .var_type
                    .as_ref()
                    .map(|ty| state.model.resolve_type(ty));
                for declarator in &variable.declarators {
                    if let Some(ty) = ty.as_ref().cloned().or_else(|| {
                        declarator
                            .initializer
                            .as_ref()
                            .map(|value| state.infer_invocation(value))
                    }) {
                        collector.globals.insert(declarator.name.name.as_str(), ty);
                    }
                }
            }
            TopLevelDecl::Function(function) if matches!(function.kind, FunctionKind::Getter) => {
                if let Some(ty) = &function.return_type {
                    collector
                        .globals
                        .insert(function.name.name.as_str(), state.model.resolve_type(ty));
                }
            }
            _ => {}
        }
    }
    semantic_scope::visit_program(&mut collector, program, state);
    collector.semantics
}

struct Collector<'a> {
    semantics: Semantics,
    globals: HashMap<&'a str, ResolvedType>,
}

fn key(expression: &Expr) -> usize {
    expression as *const Expr as usize
}

fn mounted_kind(receiver: &ResolvedType, state: &SemanticState<'_>) -> Option<bool> {
    if let Some(owner) = state
        .signatures
        .resolved_member_owner(receiver, "mounted", &state.model)
    {
        return if owner == DeclarationIdentity::flutter("State") {
            Some(true)
        } else if owner == DeclarationIdentity::flutter("BuildContext") {
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
fn context_kind(
    expression: &Expr,
    state: &SemanticState<'_>,
    globals: &HashMap<&str, ResolvedType>,
) -> Option<bool> {
    match expression {
        Expr::Paren { expr, .. } => context_kind(expr.unparenthesized(), state, globals),
        Expr::NullAssert { operand, .. } => context_kind(operand, state, globals),
        Expr::Ident(identifier) => {
            if state.is_own_field(&identifier.name) {
                context_member_kind(state.enclosing_type.as_ref()?, &identifier.name, state)
            } else if state.environment.is_bound(&identifier.name) {
                context_type_kind(
                    &state.environment.declared_type(&identifier.name),
                    false,
                    state,
                )
            } else {
                if let Some(ty) = globals.get(identifier.name.as_str()) {
                    return context_type_kind(ty, true, state);
                }
                if let Some(receiver) = &state.enclosing_type {
                    let kind = context_member_kind(receiver, &identifier.name, state);
                    if kind.is_some()
                        || state
                            .signatures
                            .resolved_field(receiver, &identifier.name, &state.model)
                            .is_some()
                    {
                        return kind;
                    }
                }
                globals
                    .get(identifier.name.as_str())
                    .and_then(|ty| context_type_kind(ty, true, state))
                    .or_else(|| {
                        // Inference reports a promoted implicit-`this` field
                        // read; its declaration was classified above.
                        if state
                            .environment
                            .promoted_this_field(&identifier.name)
                            .is_some()
                        {
                            return None;
                        }
                        context_type_kind(&state.infer_invocation(expression), true, state)
                    })
            }
        }
        Expr::Field { object, field, .. } => {
            let receiver = receiver_type(object, state, globals);
            // Only the getter declaration establishes a context use. An
            // inferred access type may substitute T with BuildContext.
            context_member_kind(&receiver, &field.name, state)
        }
        Expr::IntLit { .. }
        | Expr::DoubleLit { .. }
        | Expr::StringLit(_)
        | Expr::BoolLit { .. }
        | Expr::NullLit { .. }
        | Expr::This { .. }
        | Expr::Super { .. }
        | Expr::Unary { .. }
        | Expr::PostfixIncDec { .. }
        | Expr::Binary { .. }
        | Expr::Assign { .. }
        | Expr::Conditional { .. }
        | Expr::Is { .. }
        | Expr::As { .. }
        | Expr::Index { .. }
        | Expr::Call { .. }
        | Expr::Cascade { .. }
        | Expr::List { .. }
        | Expr::Map { .. }
        | Expr::Set { .. }
        | Expr::Record { .. }
        | Expr::FuncExpr { .. }
        | Expr::New { .. }
        | Expr::DotShorthand { .. }
        | Expr::Await { .. }
        | Expr::Throw { .. }
        | Expr::Switch { .. }
        | Expr::SymbolLit { .. }
        | Expr::GenericInstantiation { .. }
        | Expr::Error { .. } => None,
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
        if *identity == DeclarationIdentity::flutter("BuildContext")
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

fn constant_bool(expression: &Expr, state: &SemanticState<'_>) -> Option<bool> {
    if let Expr::BoolLit { value, .. } = expression.unparenthesized() {
        return Some(*value);
    }
    if !matches!(
        expression.unparenthesized(),
        Expr::Ident(_)
            | Expr::Field { .. }
            | Expr::Unary { .. }
            | Expr::Binary { .. }
            | Expr::Conditional { .. }
    ) {
        return None;
    }
    // The shared evaluator resolves library constants, not lexical bindings.
    // Do not accidentally fold a shadowed library constant as a local value.
    struct BoundNames<'a, 'model> {
        state: &'a SemanticState<'model>,
        found: bool,
    }
    impl Visitor for BoundNames<'_, '_> {
        fn visit_expr(&mut self, expression: &Expr) {
            if let Expr::Ident(identifier) = expression {
                self.found |= self.state.environment.is_bound(&identifier.name);
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
    match evaluate_constant(
        expression,
        None,
        &HashMap::new(),
        &state.model,
        &state.signatures,
    ) {
        Some(ConstantValue::Bool(value)) => Some(value),
        _ => None,
    }
}

fn receiver_type(
    expression: &Expr,
    state: &SemanticState<'_>,
    globals: &HashMap<&str, ResolvedType>,
) -> ResolvedType {
    if let Expr::Ident(identifier) = expression.unparenthesized()
        && !state.environment.is_bound(&identifier.name)
        && let Some(ty) = globals.get(identifier.name.as_str())
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

impl Collector<'_> {
    fn use_context(&mut self, expression: &Expr, state: &SemanticState<'_>) {
        if let Some(kind) = context_kind(expression, state, &self.globals) {
            self.semantics.uses.insert(key(expression), kind);
        }
    }

    fn arguments(&mut self, args: &ArgList, state: &SemanticState<'_>) {
        for expression in &args.positional {
            self.use_context(expression, state);
        }
        for argument in &args.named {
            self.use_context(&argument.value, state);
        }
    }

    fn callbacks(&mut self, args: &ArgList, positional: &[usize], named: &[&str]) {
        for &position in positional {
            if let Some(expression @ Expr::FuncExpr { .. }) =
                args.positional.get(position).map(Expr::unparenthesized)
            {
                self.semantics.protected_callbacks.insert(key(expression));
            }
        }
        for argument in &args.named {
            if named.contains(&argument.name.name.as_str())
                && matches!(argument.value.unparenthesized(), Expr::FuncExpr { .. })
            {
                self.semantics
                    .protected_callbacks
                    .insert(key(&argument.value));
            }
        }
    }

    fn constructor_callbacks(
        &mut self,
        identity: &DeclarationIdentity,
        constructor: &str,
        args: &ArgList,
    ) {
        if !identity.is_sdk("dart:async", identity.name()) {
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

    fn method_callbacks(&mut self, callee: &Expr, args: &ArgList, state: &SemanticState<'_>) {
        let Expr::Field { object, field, .. } = callee.unparenthesized() else {
            return;
        };
        // The SDK's method catalog deliberately matches the class name, unlike
        // its constructor catalog, which also requires dart:async identity.
        let static_class = expression_segments(object, false).and_then(|segments| {
            (!state.environment.is_bound(segments[0]) && !self.globals.contains_key(segments[0]))
                .then(|| state.model.resolve_dotted(&segments))
                .flatten()
        });
        if let Some(identity) = static_class {
            match (identity.name(), field.name.as_str()) {
                ("Future", "doWhile") => self.callbacks(args, &[0], &[]),
                ("Future", "forEach") => self.callbacks(args, &[1], &[]),
                ("Future", "wait") => self.callbacks(args, &[], &["cleanUp"]),
                _ => {}
            }
            return;
        }
        let receiver = receiver_type(object, state, &self.globals);
        self.instance_callbacks(&receiver, &field.name, args);
    }

    fn instance_callbacks(&mut self, receiver: &ResolvedType, method: &str, args: &ArgList) {
        let ResolvedType::Interface { identity, .. } = receiver else {
            return;
        };
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

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, expression: &Expr, state: &SemanticState<'_>) {
        if matches!(expression, Expr::Paren { .. }) {
            return;
        }
        if matches!(
            expression,
            Expr::Ident(_) | Expr::Field { .. } | Expr::Call { .. }
        ) && state.infer_invocation(expression) == ResolvedType::Never
        {
            self.semantics.never.insert(key(expression));
        }
        if let Some(value) = constant_bool(expression, state) {
            self.semantics.constants.insert(key(expression), value);
        }
        match expression {
            Expr::Ident(identifier)
                if identifier.name == "mounted"
                    && !state.environment.is_bound(&identifier.name) =>
            {
                if let Some(kind) = state
                    .enclosing_type
                    .as_ref()
                    .and_then(|receiver| mounted_kind(receiver, state))
                {
                    self.semantics.mounted.insert(key(expression), kind);
                }
            }
            Expr::Field {
                object,
                field,
                is_null_safe,
                ..
            } => {
                if field.name == "mounted" {
                    if let Some(kind) =
                        mounted_kind(&receiver_type(object, state, &self.globals), state)
                    {
                        self.semantics.mounted.insert(key(expression), kind);
                    }
                } else if !is_null_safe && matches!(object.unparenthesized(), Expr::Ident(_)) {
                    self.use_context(object, state);
                }
            }
            Expr::Call { callee, args, .. } => {
                self.arguments(args, state);
                if let Expr::Field { object, .. } = callee.unparenthesized()
                    && (is_context(&receiver_type(object, state, &self.globals), true)
                        || context_kind(object, state, &self.globals) == Some(true))
                {
                    self.use_context(object, state);
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
            Expr::Cascade {
                object, sections, ..
            } => {
                let base = receiver_type(object, state, &self.globals);
                for section in sections {
                    let mut receiver = Cow::Borrowed(&base);
                    for (index, operation) in section.ops.iter().enumerate() {
                        match operation {
                            CascadeOp::Call(name, _, args) => {
                                self.arguments(args, state);
                                self.instance_callbacks(&receiver, &name.name, args);
                                if index + 1 < section.ops.len() {
                                    receiver = Cow::Owned(
                                        state
                                            .signatures
                                            .resolved_member(&receiver, &name.name, &state.model)
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
                            CascadeOp::Invoke(type_args, args) => {
                                self.arguments(args, state);
                                if index + 1 < section.ops.len() {
                                    receiver = Cow::Owned(
                                        state
                                            .cascade_invoke_return_type(&receiver, type_args, args),
                                    );
                                }
                            }
                            CascadeOp::Field { name, .. } => {
                                if index + 1 < section.ops.len() {
                                    receiver = Cow::Owned(
                                        state
                                            .signatures
                                            .resolved_field(&receiver, &name.name, &state.model)
                                            .or_else(|| {
                                                state.signatures.resolved_extension_getter(
                                                    &receiver,
                                                    &name.name,
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
                            CascadeOp::Index { .. } => {
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
            Expr::New {
                dart_type,
                constructor_name,
                args,
                ..
            } => {
                self.arguments(args, state);
                if let ResolvedType::Interface { identity, .. } = state
                    .model
                    .resolve_type_in(dart_type, &state.type_parameters)
                {
                    self.constructor_callbacks(
                        &identity,
                        constructor_name.as_ref().map_or("new", |name| &name.name),
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
    use falcon_analyze::Rule;
    use falcon_config::FalconConfig;
    use falcon_diagnostics::Diagnostic;
    use std::path::Path;

    struct CheckProtectedCallbacks;

    impl Rule for CheckProtectedCallbacks {
        fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
            assert_eq!(collect(program, ctx).protected_callbacks.len(), 3);
            Vec::new()
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
            &CheckProtectedCallbacks,
            Path::new("generic_future_callbacks.dart"),
            source,
            &FalconConfig::default(),
        );
    }

    struct CheckCascadeCallbacks;

    impl Rule for CheckCascadeCallbacks {
        fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
            let semantics = collect(program, ctx);
            assert_eq!(semantics.protected_callbacks.len(), 2);
            assert_eq!(semantics.uses.len(), 2);
            Vec::new()
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
            &CheckCascadeCallbacks,
            Path::new("cascade_callbacks.dart"),
            source,
            &FalconConfig::default(),
        );
    }
}
