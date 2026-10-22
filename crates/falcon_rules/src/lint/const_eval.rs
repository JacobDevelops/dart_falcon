use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    ConstantValue, DeclarationIdentity, MemberFacts, MemberKind, NameIdentity, QualifiedName,
    ResolvedType, SemanticModel, SignatureIndex, evaluate_constant,
    evaluate_constant_with_local_values, evaluate_construction_with_local_values,
    expression_segments, invocation_type_args, resolve_call_constructor,
};
use falcon_syntax::cst::visit::{
    Visitor, for_each_expr, walk_annotation, walk_arg_list, walk_enum_decl, walk_expr,
    walk_field_decl, walk_formal_param, walk_local_var, walk_pattern, walk_program,
    walk_switch_case, walk_top_level_decl,
};
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::SemanticState;

/// Source ranges that Dart evaluates in a constant context.
#[derive(Default)]
pub(crate) struct ConstructorConstContexts {
    spans: Vec<Span>,
    const_constructor_asserts: Vec<Span>,
}

impl ConstructorConstContexts {
    pub(crate) fn new(program: Program<'_>) -> Self {
        let mut contexts = Self::default();
        contexts.visit_program(program);
        contexts
    }

    /// Whether `span` lies inside a constant context.
    pub(crate) fn contains(&self, span: &Span) -> bool {
        self.spans.iter().any(|context| context.contains(*span))
    }

    /// Whether `span` lies inside an assert initializer of a `const` constructor.
    pub(crate) fn in_const_constructor_assert(&self, span: &Span) -> bool {
        self.const_constructor_asserts
            .iter()
            .any(|context| context.contains(*span))
    }
}

impl<'a> Visitor<'a> for ConstructorConstContexts {
    fn visit_program(&mut self, node: Program<'a>) {
        for declaration in node.declarations() {
            if let TopLevelDecl::Variable(variable) = declaration
                && variable.keyword() == VarKeyword::Const
            {
                self.spans.extend(
                    variable
                        .declarators()
                        .filter_map(|declarator| declarator.initializer())
                        .map(|initializer| initializer.span()),
                );
            }
        }
        walk_program(self, node);
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) {
        if node.keyword() == VarKeyword::Const {
            self.spans.extend(
                node.declarators()
                    .filter_map(|declarator| declarator.initializer())
                    .map(|initializer| initializer.span()),
            );
        }
        walk_field_decl(self, node);
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) {
        if node.keyword() == VarKeyword::Const {
            self.spans.extend(
                node.declarators()
                    .filter_map(|declarator| declarator.initializer())
                    .map(|initializer| initializer.span()),
            );
        }
        walk_local_var(self, node);
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'a>) {
        if node.is_const {
            for initializer in node.initializers {
                if let ConstructorInitializer::Assert(assertion) = initializer {
                    self.const_constructor_asserts.push(assertion.span());
                }
            }
        }
    }

    fn visit_enum_decl(&mut self, node: EnumDecl<'a>) {
        self.spans.extend(
            node.variants()
                .filter_map(|variant| variant.args())
                .map(|args| args.span()),
        );
        walk_enum_decl(self, node);
    }

    fn visit_annotation(&mut self, node: Annotation<'a>) {
        if let Some(args) = node.args() {
            self.spans.push(args.span());
        }
        walk_annotation(self, node);
    }

    fn visit_formal_param(&mut self, node: FormalParam<'a>) {
        if let Some(default) = node.default_value() {
            self.spans.push(default.span());
        }
        walk_formal_param(self, node);
    }

    fn visit_pattern(&mut self, node: Pattern<'a>) {
        self.spans.push(node.span());
        walk_pattern(self, node);
    }

    fn visit_switch_case(&mut self, node: SwitchCase<'a>) {
        for kind in node.cases() {
            if let SwitchCaseKind::Expr(expression) = kind {
                self.spans.push(expression.span());
            }
        }
        walk_switch_case(self, node);
    }

    fn visit_expr(&mut self, node: Expr<'a>) {
        let explicit = match node {
            Expr::List(node) => node.is_const(),
            Expr::Map(node) => node.is_const(),
            Expr::Set(node) => node.is_const(),
            Expr::Record(node) => node.is_const(),
            Expr::New(node) => node.is_const(),
            Expr::DotShorthand(node) => node.is_const(),
            _ => false,
        };
        if explicit {
            self.spans.push(node.span());
        }
        if let Expr::Call(call) = node
            && matches!(call.callee().unparenthesized(),
                Expr::DotShorthand(shorthand) if shorthand.is_const())
        {
            self.spans.push(call.span());
        }
        walk_expr(self, node);
    }
}

pub(crate) fn arguments_potentially_constant<'a>(
    args: ArgList<'a>,
    allowed: &HashSet<&str>,
    shadowed: &HashSet<&str>,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, Expr<'a>>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    args.positional()
        .chain(args.named().map(|argument| argument.value()))
        .all(|argument| {
            potentially_constant_with_shadowed(
                argument, allowed, shadowed, owner, fields, model, signatures,
            )
        })
}

pub(crate) fn potentially_constant<'a>(
    expr: Expr<'a>,
    allowed: &HashSet<&str>,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, Expr<'a>>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    potentially_constant_with_shadowed(expr, allowed, allowed, owner, fields, model, signatures)
}

pub(crate) fn potentially_constant_with_shadowed<'a>(
    expr: Expr<'a>,
    allowed: &HashSet<&str>,
    shadowed: &HashSet<&str>,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, Expr<'a>>,
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
        initializer: false,
        valid: true,
    };
    checker.visit_expr(expr);
    checker.valid
}

/// Whether `expr` is a potentially constant expression in the initializer
/// list (or field initializer) of a constructor that would be `const`, as the
/// analyzer's constant verifier accepts it: `parameters` may be referenced;
/// operators, `is`, `as`, conditionals, string interpolation, and record
/// literals recurse without type checks; `.length` is the only property
/// access and `identical` the only call; constructor invocations and
/// collection literals must be explicitly `const` with constant operands;
/// type literals and function and constructor tear-offs are constant; `!` is
/// rejected.
pub(crate) fn initializer_potentially_constant<'a>(
    expr: Expr<'a>,
    parameters: &HashSet<&str>,
    shadowed: &HashSet<&str>,
    owner: &DeclarationIdentity,
    fields: &HashMap<&str, Expr<'a>>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    let mut checker = PotentiallyConstant {
        allowed: parameters,
        shadowed,
        owner,
        fields,
        model,
        signatures,
        initializer: true,
        valid: true,
    };
    checker.visit_expr(expr);
    checker.valid
}

struct PotentiallyConstant<'a, 'model, 'tree> {
    allowed: &'a HashSet<&'a str>,
    shadowed: &'a HashSet<&'a str>,
    owner: &'a DeclarationIdentity,
    fields: &'a HashMap<&'a str, Expr<'tree>>,
    model: &'a SemanticModel<'model>,
    signatures: &'a SignatureIndex,
    /// Checking a constructor initializer (not a constant context): nested
    /// invocations are not implicitly const and only `.length` is readable.
    initializer: bool,
    valid: bool,
}

impl<'tree> Visitor<'tree> for PotentiallyConstant<'_, '_, 'tree> {
    fn visit_expr(&mut self, node: Expr<'tree>) {
        if matches!(node, Expr::Paren(_)) {
            falcon_syntax::cst::visit::walk_expr(self, node);
            return;
        }
        if !self.valid {
            return;
        }
        if !matches!(node, Expr::Call(_) | Expr::New(_))
            && !(self.initializer && has_non_const_collection(node))
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
        if self.initializer {
            self.visit_initializer_expr(node);
            return;
        }
        match node {
            Expr::IntLit(_)
            | Expr::DoubleLit(_)
            | Expr::BoolLit(_)
            | Expr::NullLit(_)
            | Expr::SymbolLit(_) => {}
            Expr::Ident(identifier) => {
                self.valid = self.allowed.contains(identifier.name())
                    || evaluate_constant(
                        node,
                        Some(self.owner),
                        self.fields,
                        self.model,
                        self.signatures,
                    )
                    .is_some();
            }
            Expr::Call(call) => {
                let callee = call.callee();
                let shadowed = expression_segments(callee, false)
                    .and_then(|segments| segments.into_iter().next())
                    .is_some_and(|name| self.shadowed.contains(name));
                self.valid = !shadowed && const_constructor(callee, self.model, self.signatures);
                if self.valid {
                    self.visit_arguments(call.args());
                }
            }
            Expr::New(construction) if construction.is_const() => {
                self.visit_arguments(construction.args())
            }
            Expr::Field(field) if field.field().name() == "length" => walk_expr(self, node),
            // A constant type literal receiver does not make an unresolved
            // member read constant (for example, `C.staticFinal`).
            Expr::Field(_) => self.valid = self.constant_member(node),
            Expr::StringLit(_)
            | Expr::Unary(_)
            | Expr::Binary(_)
            | Expr::Conditional(_)
            | Expr::As(_)
            | Expr::NullAssert(_)
            | Expr::List(_)
            | Expr::Map(_)
            | Expr::Set(_)
            | Expr::Record(_) => walk_expr(self, node),
            _ => self.valid = false,
        }
    }
}

impl<'tree> PotentiallyConstant<'_, '_, 'tree> {
    fn visit_arguments(&mut self, args: ArgList<'tree>) {
        walk_arg_list(self, args);
    }

    fn visit_initializer_expr(&mut self, node: Expr<'tree>) {
        match node.unparenthesized() {
            Expr::IntLit(_)
            | Expr::DoubleLit(_)
            | Expr::BoolLit(_)
            | Expr::NullLit(_)
            | Expr::SymbolLit(_) => {}
            Expr::Ident(identifier) => {
                self.valid = self.allowed.contains(identifier.name())
                    || (!self.shadowed.contains(identifier.name())
                        && self.type_literal_or_tear_off(identifier.name()));
            }
            Expr::Field(field) if field.field().name() == "length" => walk_expr(self, node),
            Expr::Field(_) => self.valid = self.constant_member(node),
            Expr::Call(call) => {
                self.valid = matches!(call.callee().unparenthesized(), Expr::Ident(name)
                    if name.name() == "identical"
                        && !self.shadowed.contains(name.name())
                        && self.model.resolve_sdk_member("identical").is_some());
                if self.valid {
                    self.visit_arguments(call.args());
                }
            }
            expression
                if match expression {
                    Expr::New(node) => node.is_const(),
                    Expr::List(node) => node.is_const(),
                    Expr::Map(node) => node.is_const(),
                    Expr::Set(node) => node.is_const(),
                    Expr::Record(node) => node.is_const(),
                    _ => false,
                } =>
            {
                let constant = HashSet::new();
                self.valid = potentially_constant_with_shadowed(
                    node,
                    &constant,
                    self.shadowed,
                    self.owner,
                    self.fields,
                    self.model,
                    self.signatures,
                );
            }
            Expr::StringLit(_)
            | Expr::Unary(_)
            | Expr::Binary(_)
            | Expr::Conditional(_)
            | Expr::Is(_)
            | Expr::As(_)
            | Expr::Record(_) => walk_expr(self, node),
            _ => self.valid = false,
        }
    }

    /// Whether `name` denotes a type (a constant type literal) or a top-level
    /// function (a constant tear-off).
    fn type_literal_or_tear_off(&self, name: &str) -> bool {
        match self.model.name_identity(name) {
            NameIdentity::Type => true,
            NameIdentity::Value => self
                .model
                .resolve_value(name)
                .is_some_and(|identity| self.signatures.function(&identity).is_some()),
            NameIdentity::Unknown => false,
        }
    }

    /// Constant eligibility depends on declarations, not evaluator coverage.
    fn constant_member(&self, expr: Expr<'tree>) -> bool {
        let Some(segments) = expression_segments(expr, false) else {
            return false;
        };
        if segments
            .first()
            .is_some_and(|name| self.shadowed.contains(*name))
        {
            return false;
        }
        if let Some(identity) =
            QualifiedName::from_segments(&segments).and_then(|name| self.model.resolve_value(name))
            && (self.signatures.function(&identity).is_some()
                || self.signatures.constant_initializer(&identity).is_some())
        {
            return true;
        }
        let Some((member, owner)) = segments.split_last() else {
            return false;
        };
        let Some(owner) = self.model.resolve_dotted(owner) else {
            return false;
        };
        self.model.member_visible(&owner, member)
            && (self.constructor_tear_off(expr)
                || self.signatures.declaration(&owner).is_some_and(|facts| {
                    facts
                        .static_consts
                        .iter()
                        .any(|constant| constant.name == *member)
                })
                || self
                    .signatures
                    .declared_member_facts(&owner, member)
                    .iter()
                    .any(|facts| facts.is_static && facts.kind == MemberKind::Method))
    }

    /// Whether `expr` tears off a constructor (`C.named`, `C.new`): a constant.
    fn constructor_tear_off(&self, expr: Expr<'tree>) -> bool {
        let Some((identity, name)) = resolve_call_constructor(expr, self.model, false) else {
            return false;
        };
        // A bare class name resolves as its unnamed constructor; only an
        // explicit `.new` tears it off.
        let explicit = name != "new"
            || matches!(expr.unparenthesized(), Expr::Field(field) if field.field().name() == "new");
        explicit
            && !self.signatures.is_enum(&identity)
            && self.signatures.has_constructor(&identity, name)
    }
}

/// Whether `expr` contains a collection literal outside any constant
/// expression: such a literal is not implicitly `const`.
fn has_non_const_collection(expr: Expr<'_>) -> bool {
    struct Finder(bool);
    impl<'a> Visitor<'a> for Finder {
        fn visit_expr(&mut self, node: Expr<'a>) {
            match node {
                Expr::List(list) => self.0 |= !list.is_const(),
                Expr::Map(map) => self.0 |= !map.is_const(),
                Expr::Set(set) => self.0 |= !set.is_const(),
                Expr::Record(record) if record.is_const() => {}
                Expr::New(construction) if construction.is_const() => {}
                _ => walk_expr(self, node),
            }
        }
    }
    let mut finder = Finder(false);
    finder.visit_expr(expr);
    finder.0
}

fn const_constructor(
    callee: Expr<'_>,
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
    pub(crate) fn of(program: Program<'_>) -> Self {
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
    fn initializers<'a>(
        &mut self,
        keyword: VarKeyword,
        declarators: AstChildren<'a, VarDeclarator<'a>>,
    ) {
        if keyword == VarKeyword::Const {
            self.0.extend(
                declarators
                    .filter_map(|declarator| declarator.initializer())
                    .map(|initializer| widen(initializer.span())),
            );
        }
    }
}

/// Widens a root span by one byte on each side so the root expression itself
/// counts as strictly inside the context.
fn widen(span: Span) -> Span {
    Span::new(span.start.saturating_sub(1), span.end + 1)
}

impl<'a> Visitor<'a> for ConstContextCollector {
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        if let TopLevelDecl::Variable(variable) = node {
            self.initializers(variable.keyword(), variable.declarators());
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) {
        self.initializers(node.keyword(), node.declarators());
        walk_field_decl(self, node);
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) {
        self.initializers(node.keyword(), node.declarators());
        walk_local_var(self, node);
    }

    fn visit_annotation(&mut self, node: Annotation<'a>) {
        self.0.push(widen(node.span()));
    }

    fn visit_enum_decl(&mut self, node: EnumDecl<'a>) {
        self.0
            .extend(node.variants().map(|variant| widen(variant.span())));
        walk_enum_decl(self, node);
    }
    fn visit_switch_case(&mut self, node: SwitchCase<'a>) {
        for kind in node.cases() {
            if let SwitchCaseKind::Expr(expression) = kind {
                self.0.push(widen(expression.span()));
            }
        }
        walk_switch_case(self, node);
    }

    fn visit_expr(&mut self, node: Expr<'a>) {
        let explicit = match node {
            Expr::New(node) => node.is_const(),
            Expr::List(node) => node.is_const(),
            Expr::Map(node) => node.is_const(),
            Expr::Set(node) => node.is_const(),
            Expr::Record(node) => node.is_const(),
            // The invocation includes the shorthand's constant arguments.
            Expr::Call(call) => matches!(call.callee().unparenthesized(),
                Expr::DotShorthand(shorthand) if shorthand.is_const()),
            _ => false,
        };
        if explicit {
            self.0.push(node.span());
        }
        walk_expr(self, node);
    }
}

/// The prefixes of the program's deferred imports: names through them are
/// never constant.
pub(crate) fn deferred_prefixes(program: Program<'_>) -> Vec<&str> {
    program
        .imports()
        .filter(|import| import.is_deferred())
        .filter_map(|import| import.as_name())
        .map(|prefix| prefix.name())
        .collect()
}

/// Whether `expression`, placed in a constant context at its current position,
/// is a valid constant expression — the analyzer's `canBeConst` question for
/// a non-const operand. Unresolvable references answer `false`.
pub(crate) fn constant_in_context(
    expression: Expr<'_>,
    deferred: &[&str],
    state: &SemanticState<'_>,
) -> bool {
    ConstantExpressions { state, deferred }.constant(expression)
}

/// Whether invoking `identity`'s constructor `constructor` with `type_args`
/// and `args` is a valid constant expression in a constant context.
pub(crate) fn constant_invocation(
    identity: &DeclarationIdentity,
    constructor: &str,
    type_args: AstChildren<'_, DartType<'_>>,
    args: ArgList<'_>,
    deferred: &[&str],
    state: &SemanticState<'_>,
) -> bool {
    ConstantExpressions { state, deferred }.invocation(identity, constructor, type_args, args)
}

struct ConstantExpressions<'s, 'a> {
    state: &'s SemanticState<'a>,
    /// Deferred import prefixes.
    deferred: &'s [&'s str],
}

impl ConstantExpressions<'_, '_> {
    fn constant(&self, expression: Expr<'_>) -> bool {
        match expression {
            Expr::Paren(paren) => self.constant(paren.expr().unparenthesized()),
            Expr::IntLit(_)
            | Expr::DoubleLit(_)
            | Expr::BoolLit(_)
            | Expr::NullLit(_)
            | Expr::SymbolLit(_) => true,
            Expr::StringLit(string) => self.string(string),
            Expr::Ident(_) | Expr::Field(_) => self.reference(expression),
            Expr::Unary(_) | Expr::Binary(_) | Expr::Conditional(_) => {
                self.value(expression).is_some()
            }
            Expr::Is(test) => self.constant(test.expr()) && self.type_constant(test.dart_type()),
            Expr::As(cast) => {
                self.constant(cast.expr())
                    && self.type_constant(cast.dart_type())
                    && self.value(cast.expr()).is_none_or(|value| {
                        let target = self
                            .state
                            .model
                            .resolve_type_in(cast.dart_type(), &self.state.type_parameters);
                        falcon_analyze::constant_value::constant_cast_validity(
                            &value,
                            &target,
                            &self.state.model,
                            &self.state.signatures,
                        ) != Some(false)
                    })
            }
            Expr::List(list) => {
                list.type_arg().is_none_or(|ty| self.type_constant(ty))
                    && list.elements().all(|element| self.element(element, false))
            }
            Expr::Set(set) => {
                set.type_arg().is_none_or(|ty| self.type_constant(ty))
                    && set.elements().all(|element| self.element(element, true))
            }
            Expr::Map(map) => {
                map.type_args().all(|ty| self.type_constant(ty))
                    && map.elements().all(|element| self.map_element(element))
            }
            Expr::Record(record) => record.fields().all(|field| self.constant(field.value())),
            // An explicit `new` is a compile error in a constant context.
            Expr::New(construction) => construction.is_const(),
            Expr::Call(call) => self.call(call.callee(), call.type_args(), call.args()),
            Expr::This(_)
            | Expr::Super(_)
            | Expr::PostfixIncDec(_)
            | Expr::Assign(_)
            | Expr::Index(_)
            | Expr::Cascade(_)
            | Expr::FuncExpr(_)
            | Expr::DotShorthand(_)
            | Expr::Await(_)
            | Expr::Throw(_)
            | Expr::Switch(_)
            | Expr::NullAssert(_)
            | Expr::Error(_) => false,
            // `List<int>` type literals and `f<int>` tear-off instantiations.
            Expr::GenericInstantiation(instantiation) => {
                let target = instantiation.target();
                let type_literal = expression_segments(target, true)
                    .is_some_and(|segments| self.state.model.resolve_dotted(&segments).is_some());
                // Analyzer treats type literals and constructor tear-offs as
                // constant here even with type-parameter or deferred arguments.
                (type_literal || instantiation.type_args().all(|ty| self.type_constant(ty)))
                    && self.reference(target)
            }
        }
    }

    /// Interpolations must be constants of type `num`, `bool`, `String`, or
    /// `Null`; an unparsed interpolation (a bare `$` in a non-raw literal)
    /// cannot be proven constant.
    fn string(&self, string: StringLitNode<'_>) -> bool {
        if string.interpolations().next().is_none() {
            return string.has_raw_segment() || !has_unescaped_dollar(&string.raw());
        }
        string.interpolations().all(|interpolation| {
            matches!(
                self.value(interpolation.expr()),
                Some(
                    ConstantValue::Null
                        | ConstantValue::Bool(_)
                        | ConstantValue::Number(_)
                        | ConstantValue::String(_)
                )
            )
        })
    }

    /// The constant value of `expression`, using declaration-time values for
    /// `const` locals; `None` when it references any other local binding.
    fn value(&self, expression: Expr<'_>) -> Option<ConstantValue> {
        let mut local = false;
        for_each_expr(expression, &mut |node| match node {
            Expr::Ident(identifier)
                if self.state.environment.is_bound(identifier.name())
                    && self.state.local_constant(identifier.name()).is_none()
                    && !(self
                        .state
                        .environment
                        .is_bound_in_root_scope(identifier.name())
                        && self.state.constant_owner.as_ref().is_some_and(|owner| {
                            self.state
                                .signatures
                                .declared_member_facts(owner, identifier.name())
                                .iter()
                                .any(|facts| facts.is_static && facts.kind == MemberKind::Method)
                                || self
                                    .state
                                    .signatures
                                    .declaration(owner)
                                    .is_some_and(|facts| {
                                        facts
                                            .static_consts
                                            .iter()
                                            .any(|constant| constant.name == identifier.name())
                                    })
                        })) =>
            {
                local = true;
            }
            Expr::Field(field) if matches!(field.object().unparenthesized(), Expr::Ident(prefix) if self.is_deferred(prefix.name())) =>
            {
                local = true;
            }
            _ => {}
        });
        if local {
            return None;
        }
        evaluate_constant_with_local_values(
            expression,
            self.state.constant_owner.as_ref(),
            &self.state.visible_local_constants(),
            &self.state.model,
            &self.state.signatures,
        )
    }

    /// A name or property access: a constant variable, enum value, type
    /// literal, or function or constructor tear-off.
    fn reference(&self, expression: Expr<'_>) -> bool {
        let Some(segments) = expression_segments(expression, true) else {
            // `constant.length` on a constant string.
            return matches!(expression.unparenthesized(), Expr::Field(field)
                if !field.is_null_safe() && field.field().name() == "length"
                    && matches!(self.value(field.object()), Some(ConstantValue::String(_))));
        };
        let first = segments[0];
        if segments.len() > 1 && self.is_deferred(first) {
            return false;
        }
        if self.state.type_parameters.contains(first) {
            return false;
        }
        if self.value(expression).is_some() {
            return true;
        }
        if self.state.environment.is_bound(first) {
            return segments.len() == 1 && self.state.local_constant(first).is_some();
        }
        let model = &self.state.model;
        let signatures = &self.state.signatures;
        if model.resolve_dotted(&segments).is_some() {
            return true;
        }
        if let Some(name) = QualifiedName::from_segments(&segments)
            && let Some(identity) = model.resolve_value(name)
            && (signatures.function(&identity).is_some()
                || signatures.constant_initializer(&identity).is_some())
        {
            return true;
        }
        let (member, owner) = segments.split_last().expect("non-empty segments");
        let Some(owner) = (!owner.is_empty())
            .then(|| model.resolve_dotted(owner))
            .flatten()
        else {
            return false;
        };
        (*member == "values" && signatures.is_enum(&owner))
            || (!signatures.is_enum(&owner) && signatures.has_constructor(&owner, member))
            || (model.member_visible(&owner, member)
                && signatures.declaration(&owner).is_some_and(|facts| {
                    facts
                        .static_consts
                        .iter()
                        .any(|constant| constant.name == *member)
                }))
            || signatures
                .declared_member_facts(&owner, member)
                .iter()
                .any(|facts| facts.is_static && facts.kind == MemberKind::Method)
    }

    fn call<'a>(
        &self,
        callee: Expr<'a>,
        type_args: AstChildren<'a, DartType<'a>>,
        args: ArgList<'a>,
    ) -> bool {
        if let Expr::DotShorthand(shorthand) = callee.unparenthesized() {
            return matches!(self.state.expected(), Some(ResolvedType::Interface { identity, .. })
                if self.invocation(identity, shorthand.name().name(), type_args, args));
        }
        let Some(segments) = expression_segments(callee, true) else {
            return false;
        };
        if self.state.environment.is_bound(segments[0])
            || (segments.len() > 1 && self.is_deferred(segments[0]))
        {
            return false;
        }
        let model = &self.state.model;
        if segments.len() == 1
            && model
                .resolve_value(segments[0])
                .or_else(|| model.resolve_sdk_member(segments[0]))
                .is_some_and(|identity| identity.is_sdk("dart:core", "identical"))
        {
            return args.positional().len() == 2
                && args.named().is_empty()
                && args.positional().all(|argument| self.constant(argument));
        }
        let Some((identity, constructor)) = resolve_call_constructor(callee, model, true) else {
            return false;
        };
        self.invocation(
            &identity,
            constructor,
            invocation_type_args(callee, type_args),
            args,
        )
    }

    fn invocation(
        &self,
        identity: &DeclarationIdentity,
        constructor: &str,
        type_args: AstChildren<'_, DartType<'_>>,
        args: ArgList<'_>,
    ) -> bool {
        let state = self.state;
        if !state.signatures.is_const_constructor(identity, constructor)
            || !type_args.clone().all(|ty| self.type_constant(ty))
            || !args
                .positional()
                .chain(args.named().map(|argument| argument.value()))
                .all(|argument| self.constant(argument))
        {
            return false;
        }
        // Running the constructor proves errors such as a failing assertion.
        let type_args: Vec<_> = type_args
            .into_iter()
            .map(|ty| state.model.resolve_type_in(ty, &state.type_parameters))
            .collect();
        !evaluate_construction_with_local_values(
            identity,
            constructor,
            &type_args,
            args,
            &state.visible_local_constants(),
            &state.model,
            &state.signatures,
        )
        .fails
    }

    fn is_deferred(&self, prefix: &str) -> bool {
        !self.state.environment.is_bound(prefix) && self.deferred.contains(&prefix)
    }

    fn type_constant(&self, ty: DartType<'_>) -> bool {
        struct DeferredTypes<F> {
            is_deferred: F,
            found: bool,
        }
        impl<'a, F: Fn(&str) -> bool> Visitor<'a> for DeferredTypes<F> {
            fn visit_dart_type(&mut self, ty: DartType<'a>) {
                if let DartType::Named(named) = ty
                    && named.segments().nth(1).is_some()
                    && (self.is_deferred)(
                        named.segments().next().expect("named type segment").name(),
                    )
                {
                    self.found = true;
                }
                falcon_syntax::cst::visit::walk_dart_type(self, ty);
            }
        }
        let mut deferred = DeferredTypes {
            is_deferred: |prefix: &str| self.is_deferred(prefix),
            found: false,
        };
        deferred.visit_dart_type(ty);
        !deferred.found
            && !mentions_type_parameter(
                &self
                    .state
                    .model
                    .resolve_type_in(ty, &self.state.type_parameters),
            )
    }

    /// A list or set element; set elements must have primitive equality.
    fn element(&self, element: CollectionElement<'_>, set: bool) -> bool {
        match element {
            CollectionElement::Expr(expression) => {
                self.constant(expression) && (!set || self.primitive_equality(expression, 0))
            }
            CollectionElement::NullAware(element) => {
                let expression = element.expr();
                self.constant(expression) && (!set || self.primitive_equality(expression, 0))
            }
            CollectionElement::Spread(spread) => self.constant(spread.expr()),
            CollectionElement::If(element) => {
                self.condition(element.condition())
                    && self.element(element.then_elem(), set)
                    && element
                        .else_elem()
                        .is_none_or(|element| self.element(element, set))
            }
            CollectionElement::For(_) => false,
        }
    }

    /// A map element; keys must have primitive equality.
    fn map_element(&self, element: MapElement<'_>) -> bool {
        match element {
            MapElement::Entry(entry) => {
                self.constant(entry.key())
                    && self.constant(entry.value())
                    && self.primitive_equality(entry.key(), 0)
            }
            MapElement::Spread(spread) => self.constant(spread.expr()),
            MapElement::If(element) => {
                self.condition(element.condition())
                    && self.map_element(element.then_entry())
                    && element
                        .else_entry()
                        .is_none_or(|entry| self.map_element(entry))
            }
            MapElement::For(_) => false,
        }
    }

    fn condition(&self, condition: IfCondition<'_>) -> bool {
        match condition {
            IfCondition::Expr(expression) => {
                matches!(self.value(expression), Some(ConstantValue::Bool(_)))
            }
            IfCondition::Case(..) => false,
        }
    }

    /// Whether the constant `expression` provably has primitive equality, as
    /// constant set elements and map keys must: it does not use an `==`
    /// other than `Object`'s (`double` overrides it). Follows constant
    /// references through their initializers, `depth` levels deep.
    fn primitive_equality(&self, expression: Expr<'_>, depth: usize) -> bool {
        if depth > 16 {
            return false;
        }
        let model = &self.state.model;
        let signatures = &self.state.signatures;
        match expression {
            Expr::Paren(paren) => self.primitive_equality(paren.expr().unparenthesized(), depth),
            Expr::IntLit(_)
            | Expr::StringLit(_)
            | Expr::BoolLit(_)
            | Expr::NullLit(_)
            | Expr::SymbolLit(_)
            | Expr::List(_)
            | Expr::Set(_)
            | Expr::Map(_) => true,
            Expr::Record(record) => record
                .fields()
                .all(|field| self.primitive_equality(field.value(), depth + 1)),
            Expr::Conditional(conditional) => {
                self.primitive_equality(conditional.then_expr(), depth + 1)
                    && self.primitive_equality(conditional.else_expr(), depth + 1)
            }
            Expr::Unary(_) | Expr::Binary(_) => matches!(
                self.value(expression),
                Some(ConstantValue::Null | ConstantValue::Bool(_) | ConstantValue::String(_))
            ),
            Expr::New(construction) => matches!(
                model.resolve_type(construction.dart_type()),
                ResolvedType::Interface { identity, .. } if self.class_primitive(&identity)
            ),
            Expr::Call(call) => resolve_call_constructor(call.callee(), model, true)
                .is_some_and(|(identity, _)| self.class_primitive(&identity)),
            Expr::Ident(_) | Expr::Field(_) => {
                let Some(segments) = expression_segments(expression, false) else {
                    return false;
                };
                if self.state.environment.is_bound(segments[0]) {
                    return segments.len() == 1
                        && self
                            .state
                            .local_constant(segments[0])
                            .is_some_and(|initializer| {
                                self.primitive_equality(initializer, depth + 1)
                            });
                }
                let (member, owner) = segments.split_last().expect("non-empty segments");
                if let Some(owner) = (!owner.is_empty())
                    .then(|| model.resolve_dotted(owner))
                    .flatten()
                {
                    if signatures.is_enum_value(&owner, member) {
                        return true;
                    }
                    if let Some(constant) = signatures.declaration(&owner).and_then(|facts| {
                        facts
                            .static_consts
                            .iter()
                            .find(|constant| constant.name == *member)
                    }) {
                        return self.primitive_equality(constant.initializer.expr(), depth + 1);
                    }
                }
                QualifiedName::from_segments(&segments)
                    .and_then(|name| model.resolve_value(name))
                    .and_then(|identity| signatures.constant_initializer(&identity))
                    .is_some_and(|initializer| self.primitive_equality(initializer, depth + 1))
            }
            Expr::DoubleLit(_)
            | Expr::This(_)
            | Expr::Super(_)
            | Expr::PostfixIncDec(_)
            | Expr::Assign(_)
            | Expr::Is(_)
            | Expr::As(_)
            | Expr::Index(_)
            | Expr::Cascade(_)
            | Expr::FuncExpr(_)
            | Expr::DotShorthand(_)
            | Expr::Await(_)
            | Expr::Throw(_)
            | Expr::Switch(_)
            | Expr::NullAssert(_)
            | Expr::GenericInstantiation(_)
            | Expr::Error(_) => false,
        }
    }

    /// Whether instances of `identity` use `Object`'s `==`; unknown
    /// hierarchies answer `false`.
    fn class_primitive(&self, identity: &DeclarationIdentity) -> bool {
        let signatures = &self.state.signatures;
        if signatures.is_enum(identity) || *identity == DeclarationIdentity::OBJECT {
            return true;
        }
        let concrete = |facts: &MemberFacts| {
            !facts.is_static
                && !facts.is_abstract
                && facts.declaring_type != DeclarationIdentity::OBJECT
        };
        !signatures
            .declared_member_facts(identity, "==")
            .iter()
            .any(concrete)
            && signatures
                .inherited_member_facts(identity, "==")
                .is_some_and(|facts| !facts.iter().any(concrete))
    }
}

/// Whether a non-raw literal's source contains a `$` not escaped by `\`.
fn has_unescaped_dollar(raw: &str) -> bool {
    let mut escaped = false;
    for character in raw.chars() {
        match (escaped, character) {
            (true, _) => escaped = false,
            (false, '\\') => escaped = true,
            (false, '$') => return true,
            (false, _) => {}
        }
    }
    false
}

/// Whether `ty` mentions a type parameter anywhere.
pub(crate) fn mentions_type_parameter(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::TypeParameter { .. } | ResolvedType::NonNullTypeParameter { .. } => true,
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
