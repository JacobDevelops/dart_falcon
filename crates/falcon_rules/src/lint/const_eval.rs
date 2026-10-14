use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    DeclarationIdentity, ResolvedType, SemanticModel, SignatureIndex, evaluate_constant,
    expression_segments,
};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_enum_decl, walk_expr, walk_field_decl, walk_local_var, walk_top_level_decl,
};

pub(crate) fn arguments_potentially_constant(
    args: &ArgList,
    allowed: &HashSet<String>,
    shadowed: &HashSet<String>,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    args.positional
        .iter()
        .chain(args.named.iter().map(|argument| &argument.value))
        .all(|argument| {
            potentially_constant_with_shadowed(
                argument, allowed, shadowed, owner, fields, model, signatures,
            )
        })
}

pub(crate) fn potentially_constant(
    expr: &Expr,
    allowed: &HashSet<String>,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    potentially_constant_with_shadowed(expr, allowed, allowed, owner, fields, model, signatures)
}

pub(crate) fn potentially_constant_with_shadowed(
    expr: &Expr,
    allowed: &HashSet<String>,
    shadowed: &HashSet<String>,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, &Expr>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    let mut checker = PotentiallyConstant {
        allowed,
        shadowed,
        owner,
        fields,
        model,
        signatures,
        valid: true,
    };
    checker.visit_expr(expr);
    checker.valid
}

struct PotentiallyConstant<'a, 'model> {
    allowed: &'a HashSet<String>,
    shadowed: &'a HashSet<String>,
    owner: &'a DeclarationIdentity,
    fields: &'a HashMap<&'a str, &'a Expr>,
    model: &'a SemanticModel<'model>,
    signatures: &'a SignatureIndex,
    valid: bool,
}

impl Visitor for PotentiallyConstant<'_, '_> {
    fn visit_expr(&mut self, node: &Expr) {
        if !self.valid {
            return;
        }
        if !matches!(node, Expr::Call { .. } | Expr::New { .. })
            && evaluate_constant(
                node,
                Some(self.owner),
                self.fields,
                self.model,
                self.signatures,
            )
            .is_some()
        {
            return;
        }
        match node {
            Expr::IntLit { .. }
            | Expr::DoubleLit { .. }
            | Expr::BoolLit { .. }
            | Expr::NullLit { .. }
            | Expr::SymbolLit { .. } => {}
            Expr::Ident(identifier) => {
                self.valid = self.allowed.contains(&identifier.name)
                    || evaluate_constant(
                        node,
                        Some(self.owner),
                        self.fields,
                        self.model,
                        self.signatures,
                    )
                    .is_some();
            }
            Expr::Call { callee, args, .. } => {
                let shadowed = expression_segments(callee, false)
                    .and_then(|segments| segments.into_iter().next())
                    .is_some_and(|name| self.shadowed.contains(name));
                self.valid = !shadowed && const_constructor(callee, self.model, self.signatures);
                if self.valid {
                    self.visit_arguments(args);
                }
            }
            Expr::New {
                is_const: true,
                args,
                ..
            } => self.visit_arguments(args),
            Expr::StringLit(_)
            | Expr::Unary { .. }
            | Expr::Binary { .. }
            | Expr::Conditional { .. }
            | Expr::As { .. }
            | Expr::Field { .. }
            | Expr::NullAssert { .. }
            | Expr::List { .. }
            | Expr::Map { .. }
            | Expr::Set { .. }
            | Expr::Record { .. } => walk_expr(self, node),
            _ => self.valid = false,
        }
    }
}

impl PotentiallyConstant<'_, '_> {
    fn visit_arguments(&mut self, args: &ArgList) {
        for argument in args
            .positional
            .iter()
            .chain(args.named.iter().map(|argument| &argument.value))
        {
            self.visit_expr(argument);
        }
    }
}

fn const_constructor(
    callee: &Expr,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    let Some((identity, constructor)) =
        falcon_analyze::resolve_call_constructor(callee, model, false)
    else {
        return false;
    };
    signatures.is_const_constructor(&identity, constructor)
}

/// Source ranges that are constant contexts: the operands of explicitly
/// `const` expressions, `const` variable initializers, annotations, and
/// enum-constant arguments. An expression strictly inside one of these is
/// implicitly constant.
pub(crate) struct ConstContexts {
    /// Sorted by `(start, end)`. Widened roots may partially overlap
    /// neighbours, so lookups do not assume nesting.
    spans: Vec<Span>,
    /// `max_end[i]` is the greatest `end` among `spans[..=i]`.
    max_end: Vec<usize>,
}

impl ConstContexts {
    pub(crate) fn of(program: &Program) -> Self {
        let mut collector = ConstContextCollector(Vec::new());
        collector.visit_program(program);
        let mut spans = collector.0;
        spans.sort_unstable_by_key(|span| (span.start, span.end));
        let max_end = spans
            .iter()
            .scan(0, |max, span| {
                *max = span.end.max(*max);
                Some(*max)
            })
            .collect();
        Self { spans, max_end }
    }

    /// Whether `span` lies inside a constant context (a const expression's own
    /// span does not count as inside itself).
    pub(crate) fn contains(&self, span: &Span) -> bool {
        // A context starting strictly before `span` differs from it, so it
        // contains `span` iff it reaches `span.end`.
        let before = self
            .spans
            .partition_point(|context| context.start < span.start);
        if before > 0 && self.max_end[before - 1] >= span.end {
            return true;
        }
        // Among contexts starting with `span`, the longest is last; it must
        // strictly outreach `span` to differ from it.
        let through = self
            .spans
            .partition_point(|context| context.start <= span.start);
        through > before && self.spans[through - 1].end > span.end
    }
}

struct ConstContextCollector(Vec<Span>);

impl ConstContextCollector {
    fn initializers(&mut self, keyword: VarKeyword, declarators: &[VarDeclarator]) {
        if keyword == VarKeyword::Const {
            self.0.extend(
                declarators
                    .iter()
                    .filter_map(|declarator| declarator.initializer.as_ref())
                    .map(|initializer| widen(*initializer.span())),
            );
        }
    }
}

/// Widens a root span by one byte on each side so the root expression itself
/// counts as strictly inside the context.
fn widen(span: Span) -> Span {
    Span::new(span.start.saturating_sub(1), span.end + 1)
}

impl Visitor for ConstContextCollector {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(variable) = node {
            self.initializers(variable.keyword, &variable.declarators);
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        self.initializers(node.keyword, &node.declarators);
        walk_field_decl(self, node);
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        self.initializers(node.keyword, &node.declarators);
        walk_local_var(self, node);
    }

    fn visit_annotation(&mut self, node: &Annotation) {
        self.0.push(widen(node.span));
    }

    fn visit_enum_decl(&mut self, node: &EnumDecl) {
        self.0
            .extend(node.variants.iter().map(|variant| widen(variant.span)));
        walk_enum_decl(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::New { is_const: true, .. }
            | Expr::List { is_const: true, .. }
            | Expr::Map { is_const: true, .. }
            | Expr::Set { is_const: true, .. }
            | Expr::Record { is_const: true, .. } => self.0.push(*node.span()),
            // `const .named(...)`: the shorthand node covers only the name, so the
            // invocation that wraps it carries the constant arguments.
            Expr::Call { callee, span, .. }
                if matches!(callee.as_ref(), Expr::DotShorthand { is_const: true, .. }) =>
            {
                self.0.push(*span);
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}

/// Whether `expression` is provably a constant expression once placed in a
/// constant context at its current position: it evaluates to a known constant,
/// references no local or member name (which may shadow a top-level
/// constant), and names no in-scope type parameter in any type argument.
pub(crate) fn constant_in_context(
    expression: &Expr,
    state: &crate::lint::semantic_scope::SemanticState<'_>,
) -> bool {
    let mut references = ScopedReferences {
        state,
        found: false,
    };
    references.visit_expr(expression);
    !references.found
        && evaluate_constant(
            expression,
            None,
            &HashMap::new(),
            &state.model,
            &state.signatures,
        )
        .is_some()
}

/// Whether `ty` mentions a type parameter anywhere.
pub(crate) fn mentions_type_parameter(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::TypeParameter { .. } => true,
        ResolvedType::Interface { arguments, .. } => arguments.iter().any(mentions_type_parameter),
        ResolvedType::Function {
            return_type,
            positional,
            named,
            ..
        } => {
            mentions_type_parameter(return_type)
                || positional.iter().any(mentions_type_parameter)
                || named.iter().any(|(_, ty)| mentions_type_parameter(ty))
        }
        ResolvedType::Record {
            positional, named, ..
        } => {
            positional.iter().any(mentions_type_parameter)
                || named.iter().any(|(_, ty)| mentions_type_parameter(ty))
        }
        ResolvedType::Unknown
        | ResolvedType::Dynamic
        | ResolvedType::Void
        | ResolvedType::Never
        | ResolvedType::Null => false,
    }
}

/// Finds identifiers bound in the lexical environment and type arguments that
/// name an in-scope type parameter.
struct ScopedReferences<'a, 'state> {
    state: &'a crate::lint::semantic_scope::SemanticState<'state>,
    found: bool,
}

impl Visitor for ScopedReferences<'_, '_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Ident(identifier) = node
            && self.state.environment.is_bound(&identifier.name)
        {
            self.found = true;
        }
        walk_expr(self, node);
    }

    fn visit_dart_type(&mut self, node: &DartType) {
        let ty = self
            .state
            .model
            .resolve_type_in(node, &self.state.type_parameters);
        if mentions_type_parameter(&ty) {
            self.found = true;
        }
    }
}
