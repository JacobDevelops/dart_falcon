//! Reject literal `null` in the closure slots of curated SDK APIs.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

pub struct NullClosures;

impl Rule for NullClosures {
    fn name(&self) -> &'static str {
        "null-closures"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector {
    diagnostics: Vec<Diagnostic>,
    file: String,
}

impl Collector {
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

    fn report_null(&mut self, expression: &Expr) {
        if !matches!(expression, Expr::NullLit { .. }) {
            return;
        }
        let span = expression.span();
        self.diagnostics.push(Diagnostic::new(
            "null-closures",
            Severity::Warning,
            "Pass a closure instead of null.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }

    fn check_arguments(&mut self, args: &ArgList, positional: &[usize], named: &[&str]) {
        for index in positional {
            if let Some(argument) = args.positional.get(*index) {
                self.report_null(argument);
            }
        }
        for argument in &args.named {
            if named.contains(&argument.name.name.as_str()) {
                self.report_null(&argument.value);
            }
        }
    }

    fn sdk_type(
        &self,
        expression: &Expr,
        state: &SemanticState<'_>,
    ) -> Option<DeclarationIdentity> {
        let segments = match expression {
            Expr::Ident(identifier) if !state.environment.is_bound(&identifier.name) => {
                vec![identifier.name.clone()]
            }
            Expr::Field { object, field, .. } => {
                let Expr::Ident(prefix) = object.as_ref() else {
                    return None;
                };
                if state.environment.is_bound(&prefix.name) {
                    return None;
                }
                vec![prefix.name.clone(), field.name.clone()]
            }
            _ => return None,
        };
        state.model.resolve_name(&segments)
    }

    fn applicable_non_sdk_member(
        &self,
        receiver: &ResolvedType,
        method: &str,
        state: &SemanticState<'_>,
    ) -> bool {
        if matches!(
            state
                .signatures
                .resolved_member_owner(receiver, method, &state.model),
            Some(DeclarationIdentity::Project { .. })
        ) {
            return true;
        }
        state
            .signatures
            .has_applicable_extension_member(receiver, method, &state.model)
    }

    fn check_call(&mut self, callee: &Expr, args: &ArgList, state: &SemanticState<'_>) {
        if let Expr::Ident(identifier) = callee {
            if !state.environment.is_bound(&identifier.name)
                && let Some(DeclarationIdentity::Sdk { library, name }) = state
                    .model
                    .resolve_sdk_member(std::slice::from_ref(&identifier.name))
                && library == "dart:async"
                && name == "scheduleMicrotask"
            {
                self.check_arguments(args, &[0], &[]);
                return;
            }
            if let Some(identity) = self.sdk_type(callee, state) {
                self.check_constructor(&identity, None, args);
            }
            return;
        }
        let Expr::Field { object, field, .. } = callee else {
            return;
        };
        if let Some(identity) = self.sdk_type(object, state) {
            if self.check_constructor(&identity, Some(&field.name), args) {
                return;
            }
            if let DeclarationIdentity::Sdk { library, name } = identity {
                match (library.as_str(), name.as_str(), field.name.as_str()) {
                    ("dart:async", "Future", "doWhile") => self.check_arguments(args, &[0], &[]),
                    ("dart:async", "Future", "forEach") => self.check_arguments(args, &[1], &[]),
                    ("dart:async", "Future", "wait") => {
                        self.check_arguments(args, &[], &["cleanUp"])
                    }
                    ("dart:async", "Timer", "run") => self.check_arguments(args, &[0], &[]),
                    _ => {}
                }
            }
            return;
        }
        let receiver = state.infer(object);
        let method = field.name.as_str();
        if self.applicable_non_sdk_member(&receiver, method, state) {
            return;
        }
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
        if self.is_sdk_instance(&receiver, "dart:async", "Future", state) && method == "then" {
            self.check_arguments(args, &[0], &["onError"]);
        }
    }

    fn check_constructor(
        &mut self,
        identity: &DeclarationIdentity,
        constructor: Option<&str>,
        args: &ArgList,
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

impl SemanticRuleVisitor for Collector {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::Call { callee, args, .. } => self.check_call(callee, args, state),
            Expr::New {
                dart_type,
                constructor_name,
                args,
                ..
            } => {
                let resolved = state
                    .model
                    .resolve_type_in(dart_type, &state.type_parameters);
                if let ResolvedType::Interface { identity, .. } = resolved {
                    self.check_constructor(
                        &identity,
                        constructor_name.as_ref().map(|name| name.name.as_str()),
                        args,
                    );
                }
            }
            _ => {}
        }
    }
}
