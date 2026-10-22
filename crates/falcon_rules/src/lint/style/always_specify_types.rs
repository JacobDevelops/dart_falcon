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
use falcon_syntax::TokenKind;
use falcon_syntax::cst::*;

use crate::lint::declarations::{documented_node_span, member_names, members_of};
use crate::lint::tokens::significant_tokens_in;
use falcon_analyze::semantic_scope::SemanticState;

const MESSAGE: &str = "Missing type annotation.";

use crate::lint::support::parameter_spans::setter_parameter_span;

#[derive(Default)]
pub struct AlwaysSpecifyTypes;

impl Rule for AlwaysSpecifyTypes {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
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
    members: HashSet<&'a str>,
    /// Names of the enclosing declarations' type parameters, which shadow
    /// library types.
    type_parameters: Vec<&'a str>,
    /// The type tested by the innermost `is` expression, which may stay raw.
    is_type_test: Option<Span>,
    /// The callee or member-access target of the expression being visited:
    /// a constructor call or tear-off, or a static access, not a type literal.
    reference: Option<Span>,
    /// Inside the pattern of a pattern assignment, whose variables are
    /// assigned rather than declared.
    assigned_pattern: bool,
    assignment_value: Option<NodeId>,
    expression_scopes: Vec<(Option<Span>, Option<Span>)>,
    diags: Vec<Diagnostic>,
}

impl<'a> Collector<'a> {
    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic(MESSAGE, span));
    }

    fn first_token(&self, span: Span) -> Span {
        significant_tokens_in(self.ctx.tree(), self.ctx.source(), span)
            .next()
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
            .unwrap_or(0);
        arity > 0 && !optional_type_args(identity, state)
    }

    /// Whether a type name without type arguments leaves generic parameters
    /// to default; an enclosing type parameter's name shadows library types.
    fn is_raw_generic(&self, segments: impl Iterator<Item = Identifier<'a>> + Clone) -> bool {
        let Some(state) = &self.state else {
            return false;
        };
        let mut names = segments.clone();
        if let Some(name) = names.next()
            && names.next().is_none()
            && self.type_parameters.contains(&name.name())
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

    fn push_type_parameters(&mut self, parameters: impl Iterator<Item = TypeParam<'a>>) {
        self.type_parameters
            .extend(parameters.map(|parameter| parameter.name().name()));
    }

    fn pop_type_parameters(&mut self, parameters: impl Iterator<Item = TypeParam<'a>>) {
        self.type_parameters
            .truncate(self.type_parameters.len() - parameters.count());
    }

    /// Whether `name`, read as an expression, is a type literal of a raw
    /// generic type: a library type that neither a local, a type parameter,
    /// nor a member of the enclosing declaration shadows. `head` is its first
    /// identifier.
    fn names_raw_generic(&self, head: Identifier<'a>, name: QualifiedName<'_>) -> bool {
        let Some(state) = &self.state else {
            return false;
        };
        !self.locals.contains(&head.span().start)
            && !self.members.contains(head.name())
            && !self.type_parameters.contains(&head.name())
            && state.model.name_identity(name) == NameIdentity::Type
            && state
                .model
                .resolve_name(name)
                .is_some_and(|identity| self.is_generic(&identity))
    }

    /// `Box` / `p.Box` as an expression naming a raw generic type.
    fn raw_type_literal(&self, node: Expr<'a>) -> Option<Span> {
        match node {
            Expr::Ident(identifier) => self
                .names_raw_generic(identifier, identifier.name().into())
                .then_some(identifier.span()),
            Expr::Field(field) if !field.is_null_safe() => match field.object() {
                Expr::Ident(prefix) => self
                    .names_raw_generic(
                        prefix,
                        QualifiedName::prefixed(prefix.name(), field.field().name()),
                    )
                    .then_some(field.span()),
                _ => None,
            },
            _ => None,
        }
    }

    /// The `var`/`final`/`const` keyword within `start..end`.
    fn keyword(&self, start: usize, end: usize) -> Option<Span> {
        self.ctx
            .tree()
            .tokens_in(self.ctx.source(), Span::new(start, end))
            .find(|token| {
                matches!(
                    token.kind(),
                    TokenKind::Var | TokenKind::Final | TokenKind::Const
                )
            })
            .map(|token| Span::new(token.offset(), token.offset() + token.len()))
    }

    fn check_declaration(
        &mut self,
        annotations: AstChildren<'a, Annotation<'a>>,
        span: Span,
        var_type: Option<DartType<'a>>,
        declarators: AstChildren<'a, VarDeclarator<'a>>,
    ) {
        if var_type.is_some() {
            return;
        }
        let Some(first) = declarators.first() else {
            return;
        };
        let start = annotations
            .last()
            .map_or(span.start, |annotation| annotation.span().end);
        let keyword = self
            .keyword(start, first.name().span().start)
            .unwrap_or(Span::new(start, first.name().span().start));
        self.report(keyword);
    }

    /// `for (var x in xs)` / `for (final x in xs)`: the keyword directly
    /// precedes the loop variable name.
    fn check_for_in(&mut self, var_type: Option<DartType<'a>>, name: Identifier<'a>) {
        if var_type.is_some() {
            return;
        }
        if let Some(token) = name
            .token()
            .or_else(|| {
                self.ctx
                    .tree()
                    .token_at(self.ctx.source(), name.syntax().span().start)
            })
            .and_then(|token| token.prev_significant())
            .filter(|token| matches!(token.kind(), TokenKind::Var | TokenKind::Final))
        {
            self.report(Span::new(token.offset(), token.offset() + token.len()));
        }
    }

    /// A parameter without a type, including a function-typed formal without
    /// a return type (`void f(cb())`); `_`-only names are exempt.
    fn check_untyped_param(&mut self, node: FormalParam<'a>) {
        if node.param_type().is_none()
            && node.initializing().is_none()
            && !is_just_underscores(node.name().name())
        {
            let span = if node.is_final() || node.is_var() {
                let start = node
                    .annotations()
                    .last()
                    .map_or(node.span().start, |annotation| annotation.span().end);
                self.keyword(start, node.name().span().start)
                    .unwrap_or(node.span())
            } else {
                documented_node_span(node.span(), node.annotations(), self.ctx)
            };
            self.report(span);
        }
    }

    /// `Box(1)` / `Box.named(1)` / `p.Box(1)`: an invocation of a generic
    /// class's constructor without type arguments; the span of its type name.
    fn raw_generic_construction(&self, callee: Expr<'a>) -> Option<Span> {
        let state = self.state.as_ref()?;
        let (identity, constructor) = resolve_call_constructor(callee, &state.model, false)?;
        if !self.is_generic(&identity) || !self.declares_constructor(&identity, constructor) {
            return None;
        }
        match callee {
            Expr::Field(field) if constructor != "new" => Some(field.object().span()),
            _ => Some(callee.span()),
        }
    }

    /// `const p.Box.named(1)` parses its type as `p.Box.named` without a
    /// separate constructor name: when the full name is no type, its last
    /// segment names the constructor.
    fn raw_generic_new(&self, named: NamedType<'a>) -> Option<Span> {
        let state = self.state.as_ref()?;
        let segments = named.segments();
        let mut type_segments = segments.clone();
        let constructor = type_segments.next_back()?;
        let first = type_segments.clone().next()?;
        if !named.type_args().is_empty()
            || QualifiedName::from_identifiers(segments)
                .and_then(|name| state.model.resolve_name(name))
                .is_some()
        {
            return None;
        }
        let identity = state
            .model
            .resolve_name(QualifiedName::from_identifiers(type_segments.clone())?)?;
        (self.is_generic(&identity) && self.declares_constructor(&identity, constructor.name()))
            .then(|| {
                first
                    .span()
                    .merge(&type_segments.next_back().unwrap_or(first).span())
            })
    }

    fn check_pattern_shorthands(&mut self, pattern: Pattern<'a>) {
        if let Pattern::Object(object) = pattern {
            for field in object.fields() {
                if field.pattern().is_none() {
                    self.report(field.name().span());
                }
            }
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Variable(variable) = node {
            self.check_declaration(
                variable.annotations(),
                variable.span(),
                variable.var_type(),
                variable.declarators(),
            );
        }
        if self.state.is_some()
            && let Some(members) = members_of(node)
        {
            self.members = member_names(members);
        }
        self.push_type_parameters(node.type_params());
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        self.pop_type_parameters(node.type_params());
        self.members.clear();
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        if let ClassMember::Method(method) = node {
            self.push_type_parameters(method.type_params());
        }
        StructuralTraversal::Continue
    }

    fn exit_class_member(&mut self, node: ClassMember<'a>) {
        if let ClassMember::Method(method) = node {
            self.pop_type_parameters(method.type_params());
        }
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) -> StructuralTraversal {
        self.check_declaration(
            node.annotations(),
            node.span(),
            node.field_type(),
            node.declarators(),
        );
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) -> StructuralTraversal {
        if node.var_type().is_none()
            && let Some(keyword) = node.keyword_span()
        {
            self.report(keyword);
        } else {
            self.check_declaration(
                node.annotations(),
                node.span(),
                node.var_type(),
                node.declarators(),
            );
        }
        StructuralTraversal::Continue
    }

    fn visit_formal_param(&mut self, node: FormalParam<'a>) -> StructuralTraversal {
        self.check_untyped_param(node);
        StructuralTraversal::Continue
    }

    fn visit_setter_decl(&mut self, node: SetterDecl<'a>) -> StructuralTraversal {
        if node.param_type().is_none() && !is_just_underscores(node.param().name()) {
            let parameter_span = setter_parameter_span(node, self.ctx);
            let keyword_start = node
                .param_annotations()
                .last()
                .map_or(parameter_span.start, |annotation| annotation.span().end);
            let span = self
                .keyword(keyword_start, node.param().span().start)
                .unwrap_or(parameter_span);
            self.report(span);
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node {
            self.push_type_parameters(function.type_params());
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, node: Stmt<'a>) {
        if let Stmt::LocalFunc(function) = node {
            self.pop_type_parameters(function.type_params());
        }
    }

    fn visit_pattern_assign_stmt(&mut self, node: PatternAssignStmt<'a>) -> StructuralTraversal {
        self.assigned_pattern = true;
        self.assignment_value = Some(node.value().syntax().id());
        StructuralTraversal::Continue
    }


    /// Loop variables of `for` statements and collection `for` elements.
    fn visit_for_header(&mut self, node: ForHeader<'a>) -> StructuralTraversal {
        if let Some(ForInit::ForIn(init)) = node.init()
            && init.declares_variable()
        {
            self.check_for_in(init.var_type(), init.name());
        }
        StructuralTraversal::Continue
    }

    fn visit_pattern(&mut self, node: Pattern<'a>) -> StructuralTraversal {
        if !self.assigned_pattern {
            if let Pattern::Variable(variable) = node
                && variable.type_().is_none()
                && variable.name().name() != "_"
            {
                self.report(self.first_token(variable.span()));
            }
            self.check_pattern_shorthands(node);
        }
        if let Pattern::Const(constant) = node
            && let ConstPatternValue::Name(name) = constant.value()
            && let (Some(head), Some(last), Some(qualified)) = (
                name.first(),
                name.clone().last(),
                QualifiedName::from_identifiers(name),
            )
            && self.names_raw_generic(head, qualified)
        {
            self.report(head.span().merge(&last.span()));
        }
        StructuralTraversal::Continue
    }

    fn visit_dart_type(&mut self, node: DartType<'a>) -> StructuralTraversal {
        if let DartType::Named(named) = node
            && named.type_args().is_empty()
            && self.is_type_test != Some(named.span())
            && self.is_raw_generic(named.segments())
        {
            self.report(named.span());
        }
        if let DartType::Function(function) = node {
            self.push_type_parameters(function.type_params());
        }
        StructuralTraversal::Continue
    }

    fn exit_dart_type(&mut self, node: DartType<'a>) {
        if let DartType::Function(function) = node {
            self.pop_type_parameters(function.type_params());
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if self.assignment_value == Some(node.syntax().id()) {
            self.assigned_pattern = false;
            self.assignment_value = None;
        }
        self.expression_scopes.push((self.is_type_test, self.reference));
        if self.reference != Some(node.span())
            && let Some(span) = self.raw_type_literal(node)
        {
            self.report(span);
        }
        self.reference = match node {
            Expr::Call(call) => Some(call.callee().span()),
            Expr::Field(field) if !matches!(field.object(), Expr::Paren(_)) => {
                Some(field.object().span())
            }
            Expr::GenericInstantiation(instantiation) => Some(instantiation.target().span()),
            _ => None,
        };
        if let Expr::FuncExpr(function) = node {
            self.push_type_parameters(function.type_params());
            return StructuralTraversal::Continue;
        }
        match node {
            Expr::List(list) if list.type_arg().is_none() => self.report(self.first_token(list.span())),
            Expr::Set(set) if set.type_arg().is_none() => self.report(self.first_token(set.span())),
            Expr::Map(map) if map.type_args().is_empty() => self.report(self.first_token(map.span())),
            // The tested type of an `is` check may stay raw; its arguments may not.
            Expr::Is(is) => self.is_type_test = Some(is.dart_type().span()),
            Expr::Call(call) if call.type_args().is_empty() => {
                if let Some(span) = self.raw_generic_construction(call.callee()) {
                    self.report(span);
                }
            }
            Expr::New(new) if new.constructor_name().is_none() => {
                if let DartType::Named(named) = new.dart_type()
                    && let Some(span) = self.raw_generic_new(named)
                {
                    self.report(span);
                }
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, node: Expr<'a>) {
        if let Expr::FuncExpr(function) = node {
            self.pop_type_parameters(function.type_params());
        }
        if let Some((type_test, reference)) = self.expression_scopes.pop() {
            self.is_type_test = type_test;
            self.reference = reference;
        }
    }
}
}
