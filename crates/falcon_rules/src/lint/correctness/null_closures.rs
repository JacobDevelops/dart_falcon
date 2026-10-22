//! Reject literal `null` in the closure slots of curated SDK APIs.
//!
//! Mirrors the official `null_closures` lists: the constructors `Future()`,
//! `Future.microtask`, `Future.sync`, `Timer()`, `Timer.periodic`, and
//! `List.generate`; statics on classes named `Future` (`doWhile`, `forEach`,
//! `wait(cleanUp:)`) and `Timer` (`run`), regardless of their library; and
//! instance methods of `Iterable`, `Map`, `List`, `Set`, `Queue`, `String`, and `Future` invoked
//! on a receiver whose static type is, or has as a supertype, that SDK class
//! (project overrides of those methods included). Invocations without an
//! explicit target — the top-level `scheduleMicrotask`, implicit-`this` calls,
//! and cascade sections — are never checked, as in the official lint.
//! Parenthesized nulls do not match. Named arguments are reported as a whole;
//! positional indexes count named arguments too, matching analyzer source order.
//! Static callback APIs match the receiver's class name, even outside the SDK.

use falcon_analyze::resolve::TypeKind;
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation,
    resolve_new_constructor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::expressions::is_parenthesized;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct NullClosures;

impl Rule for NullClosures {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics) }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Collector<'_> {
    fn is_sdk_instance(
        &self,
        receiver: &ResolvedType,
        library: &str,
        name: &str,
        state: &SemanticState<'_>,
    ) -> bool {
        state
            .signatures
            .instantiated_supertype(receiver, library, name, &state.model)
            .is_some()
    }

    fn report(&mut self, span: Span) {
        self.diagnostics.push(self.ctx.diagnostic("Pass a closure instead of `null`.", span));
    }

    fn check_arguments(&mut self, args: ArgList<'_>, positional: &[usize], named: &[&str]) {
        for argument in args.positional() {
            if !matches!(argument, Expr::NullLit(_)) || is_parenthesized(argument) {
                continue;
            }
            let start = argument.span().start;
            let index = args
                .positional()
                .map(|other| other.span().start)
                .chain(args.named().map(|other| other.span().start))
                .filter(|&other| other < start)
                .count();
            if positional.contains(&index) {
                self.report(argument.span());
            }
        }
        for argument in args.named() {
            if named.contains(&argument.name().name())
                && matches!(argument.value(), Expr::NullLit(_))
                && !is_parenthesized(argument.value())
            {
                self.report(argument.span());
            }
        }
    }

    fn sdk_type(
        &self,
        expression: Expr<'_>,
        state: &SemanticState<'_>,
    ) -> Option<DeclarationIdentity> {
        let segments = match expression {
            Expr::Ident(identifier) if !state.environment.is_bound(identifier.name()) => {
                vec![identifier.name().to_owned()]
            }
            Expr::Field(field) => {
                let Expr::Ident(prefix) = field.object() else {
                    return None;
                };
                if state.environment.is_bound(prefix.name()) {
                    return None;
                }
                vec![prefix.name().to_owned(), field.field().name().to_owned()]
            }
            _ => return None,
        };
        state.model.resolve_dotted(&segments)
    }

    fn check_call(&mut self, callee: Expr<'_>, args: ArgList<'_>, state: &SemanticState<'_>) {
        if let Expr::Ident(_) = callee {
            if let Some(identity) = self.sdk_type(callee, state) {
                self.check_constructor(&identity, None, args);
            }
            return;
        }
        let Expr::Field(field) = callee else {
            return;
        };
        let object = field.object();
        if let Some(identity) = self.sdk_type(object, state) {
            if self.check_constructor(&identity, Some(field.field().name()), args) {
                return;
            }
            // Static calls match a class name, not its declaring library.
            let is_class = match state.signatures.type_kind(&identity) {
                Some(kind) => matches!(kind, TypeKind::Class | TypeKind::MixinClass),
                None => matches!(identity, DeclarationIdentity::Sdk { .. }),
            };
            if is_class {
                match (identity.name(), field.field().name()) {
                    ("Future", "doWhile") => self.check_arguments(args, &[0], &[]),
                    ("Future", "forEach") => self.check_arguments(args, &[1], &[]),
                    ("Future", "wait") => self.check_arguments(args, &[], &["cleanUp"]),
                    ("Timer", "run") => self.check_arguments(args, &[0], &[]),
                    _ => {}
                }
            }
            return;
        }
        let receiver = if matches!(object, Expr::Super(_)) {
            state.infer_receiver(object)
        } else {
            state.infer(object)
        };
        let method = field.field().name();
        if self.is_sdk_instance(&receiver, "dart:core", "Iterable", state) {
            match method {
                "any" | "every" | "expand" | "map" | "reduce" | "skipWhile" | "takeWhile"
                | "where" => self.check_arguments(args, &[0], &[]),
                "firstWhere" | "lastWhere" | "singleWhere" => {
                    self.check_arguments(args, &[0], &["orElse"])
                }
                "forEach" => self.check_arguments(args, &[0], &[]),
                "fold" => self.check_arguments(args, &[1], &[]),
                _ => {}
            }
        }
        if self.is_sdk_instance(&receiver, "dart:core", "Map", state) {
            match method {
                "forEach" => self.check_arguments(args, &[0], &[]),
                "putIfAbsent" => self.check_arguments(args, &[1], &[]),
                _ => {}
            }
        }
        if matches!(method, "removeWhere" | "retainWhere")
            && (self.is_sdk_instance(&receiver, "dart:core", "List", state)
                || self.is_sdk_instance(&receiver, "dart:core", "Set", state)
                || self.is_sdk_instance(&receiver, "dart:collection", "Queue", state))
        {
            self.check_arguments(args, &[0], &[]);
        }
        if self.is_sdk_instance(&receiver, "dart:core", "String", state) {
            match method {
                "replaceAllMapped" | "replaceFirstMapped" => self.check_arguments(args, &[1], &[]),
                "splitMapJoin" => self.check_arguments(args, &[], &["onMatch", "onNonMatch"]),
                _ => {}
            }
        }
        if self.is_sdk_instance(&receiver, "dart:async", "Future", state) {
            match method {
                "then" => self.check_arguments(args, &[0], &["onError"]),
                "complete" => self.check_arguments(args, &[0], &[]),
                _ => {}
            }
        }
    }

    fn check_constructor(
        &mut self,
        identity: &DeclarationIdentity,
        constructor: Option<&str>,
        args: ArgList<'_>,
    ) -> bool {
        let DeclarationIdentity::Sdk { library, name } = identity else {
            return false;
        };
        match (library.as_str(), name.as_str(), constructor) {
            ("dart:async", "Future", None | Some("microtask") | Some("sync")) => {
                self.check_arguments(args, &[0], &[]);
                true
            }
            ("dart:async", "Timer", None | Some("periodic")) => {
                self.check_arguments(args, &[1], &[]);
                true
            }
            ("dart:core", "List", Some("generate")) => {
                self.check_arguments(args, &[1], &[]);
                true
            }
            _ => false,
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::Call(call) => self.check_call(call.callee(), call.args(), state),
            Expr::New(construction) => {
                if let Some((identity, constructor)) = resolve_new_constructor(
                    construction.dart_type(),
                    construction.constructor_name(),
                    &state.model,
                ) {
                    let constructor = (constructor != "new").then_some(constructor);
                    self.check_constructor(&identity, constructor, construction.args());
                }
            }
            _ => {}
        }
    }
}
