//! Requires explicit type annotations everywhere a type could be inferred.
//!
//! This is the opposite of `omit-local-variable-types`: every variable,
//! loop variable, pattern variable, and parameter needs a declared type;
//! collection literals need explicit type arguments; and generic types need
//! their type arguments spelled out instead of defaulting to `dynamic`.
//!
//! ```dart
//! var count = 0;           // flagged
//! final names = [];        // flagged (twice: declaration and literal)
//! List items = <int>[];    // flagged: raw generic type
//! int count = 0;           // ok
//! ```
//!
//! Raw generic interface types (classes, mixins, enums, extension types, and
//! `FutureOr`) and their generic typedefs need explicit arguments in
//! annotations, inheritance clauses, casts, object patterns (also in pattern
//! assignments), constructor invocations, and type literals (`print(Box)`,
//! `'$Box'`, `case Box:`). `is` checks, constructor tear-offs, static member
//! accesses, and declarations with `@optionalTypeArgs` are exempt. Loaded
//! declaration metadata supplies generic arity; SDK-free analysis retains the
//! existing identity fallback.
//!
//! Diagnostic ranges follow the analyzer's syntax anchors: collection literals
//! and variable patterns highlight their first token, while unmodified untyped
//! parameters highlight the entire parameter (including attached documentation,
//! metadata, and defaults).
//!
//! Adopted from the official `always_specify_types` lint.

use std::collections::HashSet;

use falcon_analyze::resolve::lexical_usage::locally_bound_identifiers;
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, NameIdentity, QualifiedName, ResolvedType, Rule,
    RulePreparation, StructuralRuleVisitor, StructuralTraversal, resolve_call_constructor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::{Program, TokenKind};

use crate::lint::declarations::{documented_node_span, member_names, members_of};
use falcon_analyze::semantic_scope::SemanticState;

const RULE: &str = "always-specify-types";
const MESSAGE: &str = "Missing type annotation.";

/// Setter ASTs retain the name rather than the complete formal parameter range.
pub(crate) fn setter_parameter_span(node: &SetterDecl, ctx: &AnalyzeContext) -> Span {
    let start = ctx
        .syntax
        .tokens_in(Span::new(node.name.span.end, node.param.span.end))
        .skip_while(|token| token.kind != TokenKind::LParen)
        .skip(1)
        .find(|token| !token.is_trivia())
        .map_or(node.param.span.start, |token| token.offset());
    documented_node_span(
        Span::new(start, node.param.span.end),
        &node.param_annotations,
        ctx,
    )
}

#[derive(Default)]
pub struct AlwaysSpecifyTypes;

impl Rule for AlwaysSpecifyTypes {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let state = SemanticState::new(program, ctx);
        let locals = if state.is_some() {
            locally_bound_identifiers(program)
        } else {
            HashSet::new()
        };
        let collector = Collector {
            ctx,
            state,
            locals,
            members: HashSet::new(),
            type_parameters: Vec::new(),
            is_type_test: None,
            reference: None,
            assigned_pattern: false,
            assignment_value: None,
            expression_scopes: Vec::new(),
            diags: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

/// The type parameters a top-level declaration scopes over its body.
fn declared_type_parameters(node: &TopLevelDecl) -> &[TypeParam] {
    match node {
        TopLevelDecl::Class(node) => &node.type_params,
        TopLevelDecl::Mixin(node) => &node.type_params,
        TopLevelDecl::MixinClass(node) => &node.type_params,
        TopLevelDecl::Enum(node) => &node.type_params,
        TopLevelDecl::Extension(node) => &node.type_params,
        TopLevelDecl::ExtensionType(node) => &node.type_params,
        TopLevelDecl::Function(node) => &node.type_params,
        TopLevelDecl::ClassTypeAlias(node) => &node.type_params,
        TopLevelDecl::TypeAlias(node) => &node.type_params,
        TopLevelDecl::Variable(_) | TopLevelDecl::Error(_) => &[],
    }
}

/// The number of type parameters of an SDK or allowlisted package type, which
/// the resolver models by identity only.
pub(crate) fn known_type_arity(identity: &DeclarationIdentity) -> Option<usize> {
    match identity {
        DeclarationIdentity::Sdk { library, name } => {
            Some(match (library.as_str(), name.as_str()) {
                (
                    "dart:core",
                    "Comparable" | "Iterable" | "Iterator" | "List" | "Set" | "Expando"
                    | "WeakReference" | "Finalizer",
                )
                | ("dart:async", "Future" | "FutureOr" | "Completer" | "StreamController")
                | (
                    "dart:collection",
                    "Queue" | "ListQueue" | "DoubleLinkedQueue" | "HashSet" | "LinkedHashSet"
                    | "SplayTreeSet",
                )
                | ("dart:js_interop", "JSArray" | "JSPromise") => 1,
                ("dart:core", "Map" | "MapEntry")
                | ("dart:collection", "HashMap" | "LinkedHashMap" | "SplayTreeMap") => 2,
                _ => 0,
            })
        }
        DeclarationIdentity::Package { package, name } => Some(usize::from(
            package.as_str() == "flutter" && name.as_str() == "State",
        )),
        DeclarationIdentity::Project { .. } => None,
    }
}

/// Whether the resolved declaration opts out of explicit type arguments.
fn optional_type_args(identity: &DeclarationIdentity, state: &SemanticState<'_>) -> bool {
    identity.is_package("flutter", "State")
        || state.signatures.declaration(identity).is_some_and(|facts| {
            facts
                .annotations
                .iter()
                .any(|annotation| annotation.is_package("meta", "optionalTypeArgs"))
        })
}

fn is_just_underscores(name: &str) -> bool {
    name.bytes().all(|byte| byte == b'_')
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    state: Option<SemanticState<'a>>,
    /// Start offsets of identifiers that name a local, never a type.
    locals: HashSet<usize>,
    /// Members of the enclosing declaration, which shadow library types.
    members: HashSet<String>,
    /// Names of the enclosing declarations' type parameters, which shadow
    /// library types.
    type_parameters: Vec<String>,
    /// The type tested by the innermost `is` expression, which may stay raw.
    is_type_test: Option<Span>,
    /// The callee or member-access target of the expression being visited:
    /// a constructor call or tear-off, or a static access, not a type literal.
    reference: Option<Span>,
    /// Inside the pattern of a pattern assignment, whose variables are
    /// assigned rather than declared.
    assigned_pattern: bool,
    assignment_value: Option<*const Expr>,
    expression_scopes: Vec<(Option<Span>, Option<Span>)>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic(RULE, MESSAGE, span));
    }

    fn first_token(&self, span: Span) -> Span {
        self.ctx
            .syntax
            .tokens_in(span)
            .find(|token| !token.is_trivia())
            .map_or(span, |token| {
                Span::new(token.offset(), token.offset() + token.len())
            })
    }

    /// Whether `identity` is generic and does not opt out of its arguments.
    fn is_generic(&self, identity: &DeclarationIdentity) -> bool {
        let Some(state) = &self.state else {
            return false;
        };
        let ResolvedType::Interface { .. } = state.model.declared_type(identity) else {
            return false;
        };
        let arity = state
            .model
            .type_alias_parameter_count(identity)
            .or_else(|| state.signatures.type_parameter_count(identity))
            .or_else(|| known_type_arity(identity))
            .unwrap_or(0);
        arity > 0 && !optional_type_args(identity, state)
    }

    /// Whether a type name without type arguments leaves generic parameters
    /// to default; an enclosing type parameter's name shadows library types.
    fn is_raw_generic(&self, segments: &[Identifier]) -> bool {
        let Some(state) = &self.state else {
            return false;
        };
        if let [name] = segments
            && self.type_parameters.contains(&name.name)
        {
            return false;
        }
        let Some(name) = falcon_analyze::QualifiedName::from_identifiers(segments) else {
            return false;
        };
        state
            .model
            .resolve_name(name)
            .is_some_and(|identity| self.is_generic(&identity))
    }

    /// Whether `identity` declares the constructor `name` (`new` for the
    /// unnamed one, implicit when no constructor is written). Unindexed
    /// declarations answer no.
    fn declares_constructor(&self, identity: &DeclarationIdentity, name: &str) -> bool {
        let Some(state) = &self.state else {
            return false;
        };
        let ResolvedType::Interface { identity, .. } = state.model.declared_type(identity) else {
            return false;
        };
        state.signatures.has_constructor(&identity, name)
    }

    fn push_type_parameters(&mut self, parameters: &[TypeParam]) {
        self.type_parameters.extend(
            parameters
                .iter()
                .map(|parameter| parameter.name.name.clone()),
        );
    }

    fn pop_type_parameters(&mut self, parameters: &[TypeParam]) {
        self.type_parameters
            .truncate(self.type_parameters.len() - parameters.len());
    }

    /// Whether `name`, read as an expression, is a type literal of a raw
    /// generic type: a library type that neither a local, a type parameter,
    /// nor a member of the enclosing declaration shadows. `head` is its first
    /// identifier.
    fn names_raw_generic(&self, head: &Identifier, name: QualifiedName<'_>) -> bool {
        let Some(state) = &self.state else {
            return false;
        };
        !self.locals.contains(&head.span.start)
            && !self.members.contains(&head.name)
            && !self.type_parameters.contains(&head.name)
            && state.model.name_identity(name) == NameIdentity::Type
            && state
                .model
                .resolve_name(name)
                .is_some_and(|identity| self.is_generic(&identity))
    }

    /// `Box` / `p.Box` as an expression naming a raw generic type.
    fn raw_type_literal(&self, node: &Expr) -> Option<Span> {
        match node {
            Expr::Ident(identifier) => self
                .names_raw_generic(identifier, (&identifier.name).into())
                .then_some(identifier.span),
            Expr::Field {
                object,
                field,
                is_null_safe: false,
                span,
            } => match object.as_ref() {
                Expr::Ident(prefix) => self
                    .names_raw_generic(prefix, QualifiedName::prefixed(&prefix.name, &field.name))
                    .then_some(*span),
                _ => None,
            },
            _ => None,
        }
    }

    /// The `var`/`final`/`const` keyword within `start..end`.
    fn keyword(&self, start: usize, end: usize) -> Option<Span> {
        self.ctx
            .syntax
            .tokens_in(Span::new(start, end))
            .find(|token| {
                matches!(
                    token.kind,
                    TokenKind::Var | TokenKind::Final | TokenKind::Const
                )
            })
            .map(|token| Span::new(token.offset(), token.offset() + token.len()))
    }

    fn check_declaration(
        &mut self,
        annotations: &[Annotation],
        span: Span,
        var_type: Option<&DartType>,
        declarators: &[VarDeclarator],
    ) {
        if var_type.is_some() {
            return;
        }
        let Some(first) = declarators.first() else {
            return;
        };
        let start = annotations
            .last()
            .map_or(span.start, |annotation| annotation.span.end);
        let keyword = self
            .keyword(start, first.name.span.start)
            .unwrap_or(Span::new(start, first.name.span.start));
        self.report(keyword);
    }

    /// `for (var x in xs)` / `for (final x in xs)`: the keyword directly
    /// precedes the loop variable name.
    fn check_for_in(&mut self, var_type: Option<&DartType>, name: &Identifier) {
        if var_type.is_some() {
            return;
        }
        let syntax = self.ctx.syntax.syntax_for_span(name.span);
        let range = syntax.token_range(name.span);
        if let Some(token) = syntax.tokens()[..range.start.index()]
            .iter()
            .rev()
            .find(|token| !token.is_trivia())
            .filter(|token| matches!(token.kind, TokenKind::Var | TokenKind::Final))
        {
            self.report(Span::new(token.offset(), token.offset() + token.len()));
        }
    }

    /// A parameter without a type, including a function-typed formal without
    /// a return type (`void f(cb())`); `_`-only names are exempt.
    fn check_untyped_param(&mut self, node: &FormalParam) {
        if node.param_type.is_none()
            && node.initializing.is_none()
            && !is_just_underscores(&node.name.name)
        {
            let span = if node.is_final || node.is_var {
                let start = node
                    .annotations
                    .last()
                    .map_or(node.span.start, |annotation| annotation.span.end);
                self.keyword(start, node.name.span.start)
                    .unwrap_or(node.span)
            } else {
                documented_node_span(node.span, &node.annotations, self.ctx)
            };
            self.report(span);
        }
    }

    /// `Box(1)` / `Box.named(1)` / `p.Box(1)`: an invocation of a generic
    /// class's constructor without type arguments; the span of its type name.
    fn raw_generic_construction(&self, callee: &Expr) -> Option<Span> {
        let state = self.state.as_ref()?;
        let (identity, constructor) = resolve_call_constructor(callee, &state.model, false)?;
        if !self.is_generic(&identity) || !self.declares_constructor(&identity, constructor) {
            return None;
        }
        match callee {
            Expr::Field { object, .. } if constructor != "new" => Some(*object.span()),
            _ => Some(*callee.span()),
        }
    }

    /// `const p.Box.named(1)` parses its type as `p.Box.named` without a
    /// separate constructor name: when the full name is no type, its last
    /// segment names the constructor.
    fn raw_generic_new(&self, named: &NamedType) -> Option<Span> {
        let state = self.state.as_ref()?;
        let (constructor, type_segments) = named.segments.split_last()?;
        if type_segments.is_empty()
            || !named.type_args.is_empty()
            || falcon_analyze::QualifiedName::from_identifiers(&named.segments)
                .and_then(|name| state.model.resolve_name(name))
                .is_some()
        {
            return None;
        }
        let identity =
            state
                .model
                .resolve_name(falcon_analyze::QualifiedName::from_identifiers(
                    type_segments,
                )?)?;
        (self.is_generic(&identity) && self.declares_constructor(&identity, &constructor.name))
            .then(|| {
                type_segments[0]
                    .span
                    .merge(&type_segments[type_segments.len() - 1].span)
            })
    }

    fn check_pattern_shorthands(&mut self, pattern: &Pattern) {
        if let Pattern::Object(object) = pattern {
            for field in &object.fields {
                if field.pattern.is_none() {
                    self.report(field.name.span);
                }
            }
        }
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
        if let TopLevelDecl::Variable(variable) = node {
            self.check_declaration(
                &variable.annotations,
                variable.span,
                variable.var_type.as_ref(),
                &variable.declarators,
            );
        }
        if self.state.is_some()
            && let Some(members) = members_of(node)
        {
            self.members = member_names(members);
        }
        self.push_type_parameters(declared_type_parameters(node));
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, node: &TopLevelDecl) {
        self.pop_type_parameters(declared_type_parameters(node));
        self.members.clear();
    }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        let parameters = match node {
            ClassMember::Method(method) => method.type_params.as_slice(),
            _ => &[],
        };
        self.push_type_parameters(parameters);
        StructuralTraversal::Continue
    }

    fn exit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Method(method) = node {
            self.pop_type_parameters(&method.type_params);
        }
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) -> StructuralTraversal {
        self.check_declaration(
            &node.annotations,
            node.span,
            node.field_type.as_ref(),
            &node.declarators,
        );
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) -> StructuralTraversal {
        if node.var_type.is_none()
            && let Some(keyword) = node.keyword_span
        {
            self.report(keyword);
        } else {
            self.check_declaration(
                &node.annotations,
                node.range(),
                node.var_type.as_ref(),
                &node.declarators,
            );
        }
        StructuralTraversal::Continue
    }

    fn visit_formal_param(&mut self, node: &FormalParam) -> StructuralTraversal {
        self.check_untyped_param(node);
        StructuralTraversal::Continue
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) -> StructuralTraversal {
        if node.param_type.is_none() && !is_just_underscores(&node.param.name) {
            let parameter_span = setter_parameter_span(node, self.ctx);
            let keyword_start = node
                .param_annotations
                .last()
                .map_or(parameter_span.start, |annotation| annotation.span.end);
            let span = self
                .keyword(keyword_start, node.param.span.start)
                .unwrap_or(parameter_span);
            self.report(span);
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node {
            self.push_type_parameters(&function.type_params);
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalFunc(function) = node {
            self.pop_type_parameters(&function.type_params);
        }
    }

    fn visit_pattern_assign_stmt(&mut self, node: &PatternAssignStmt) -> StructuralTraversal {
        self.assigned_pattern = true;
        self.assignment_value = Some(&node.value);
        StructuralTraversal::Continue
    }


    /// Loop variables of `for` statements and collection `for` elements.
    fn visit_for_header(&mut self, node: &ForHeader) -> StructuralTraversal {
        if let Some(ForInit::ForIn {
            declares_variable: true,
            var_type,
            name,
            ..
        }) = &node.init
        {
            self.check_for_in(var_type.as_ref(), name);
        }
        StructuralTraversal::Continue
    }

    fn visit_pattern(&mut self, node: &Pattern) -> StructuralTraversal {
        if !self.assigned_pattern {
            if let Pattern::Variable {
                type_: None,
                name,
                span,
            } = node
                && name.name != "_"
            {
                self.report(self.first_token(*span));
            }
            self.check_pattern_shorthands(node);
        }
        if let Pattern::Const(constant) = node
            && let ConstPatternValue::Name(name) = &constant.value
            && let (Some(head), Some(last), Some(qualified)) = (
                name.first(),
                name.last(),
                QualifiedName::from_identifiers(name),
            )
            && self.names_raw_generic(head, qualified)
        {
            self.report(head.span.merge(&last.span));
        }
        StructuralTraversal::Continue
    }

    fn visit_dart_type(&mut self, node: &DartType) -> StructuralTraversal {
        if let DartType::Named(named) = node
            && named.type_args.is_empty()
            && self.is_type_test != Some(named.span)
            && self.is_raw_generic(&named.segments)
        {
            self.report(named.span);
        }
        if let DartType::Function(function) = node {
            self.push_type_parameters(&function.type_params);
        }
        StructuralTraversal::Continue
    }

    fn exit_dart_type(&mut self, node: &DartType) {
        if let DartType::Function(function) = node {
            self.pop_type_parameters(&function.type_params);
        }
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if self.assignment_value.is_some_and(|value| std::ptr::eq(value, node)) {
            self.assigned_pattern = false;
            self.assignment_value = None;
        }
        self.expression_scopes.push((self.is_type_test, self.reference));
        if self.reference != Some(*node.span())
            && let Some(span) = self.raw_type_literal(node)
        {
            self.report(span);
        }
        self.reference = match node {
            Expr::Call { callee, .. } => Some(*callee.span()),
            Expr::Field { object, .. } if !matches!(object.as_ref(), Expr::Paren { .. }) => {
                Some(*object.span())
            }
            Expr::GenericInstantiation { target, .. } => Some(*target.span()),
            _ => None,
        };
        if let Expr::FuncExpr { type_params, .. } = node {
            self.push_type_parameters(type_params);
            return StructuralTraversal::Continue;
        }
        match node {
            Expr::List {
                type_arg: None,
                span,
                ..
            }
            | Expr::Set {
                type_arg: None,
                span,
                ..
            } => self.report(self.first_token(*span)),
            Expr::Map {
                type_args, span, ..
            } if type_args.is_empty() => self.report(self.first_token(*span)),
            // The tested type of an `is` check may stay raw; its arguments may not.
            Expr::Is { dart_type, .. } => self.is_type_test = Some(*dart_type.span()),
            Expr::Call {
                callee, type_args, ..
            } if type_args.is_empty() => {
                if let Some(span) = self.raw_generic_construction(callee) {
                    self.report(span);
                }
            }
            Expr::New {
                dart_type: DartType::Named(named),
                constructor_name: None,
                ..
            } => {
                if let Some(span) = self.raw_generic_new(named) {
                    self.report(span);
                }
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr { type_params, .. } = node {
            self.pop_type_parameters(type_params);
        }
        if let Some((type_test, reference)) = self.expression_scopes.pop() {
            self.is_type_test = type_test;
            self.reference = reference;
        }
    }
}
}
