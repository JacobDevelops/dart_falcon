//! Flags private function or method parameters whose nullable type is unnecessary.
//!
//! A parameter is reported when every known call argument and assignment is
//! proven non-null. Uncertain references or values that may be null are not
//! reported. This cross-file rule is configured under `cross-file`.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{AnalyzeContext, CrossFileRule, ProjectFile, SemanticIndexes};
use falcon_analyze::{SemanticRuleVisitor, SemanticState, visit_program};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};
use falcon_syntax::cst::visit::{Visitor, walk_expr_chain, walk_function_body};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UnnecessaryNullable;

impl CrossFileRule for UnnecessaryNullable {
    fn prepare_project(
        &self,
        _environment: &falcon_analyze::AnalysisEnvironment,
        files: &[ProjectFile],
        config: &FalconConfig,
        indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let indexes = indexes.expect("unnecessary-nullable requires semantic indexes");
        // Count every function/method by name project-wide so we only reason
        // about names that resolve to exactly one declaration.
        let mut decl_count: HashMap<&str, usize> = HashMap::new();
        for f in files {
            if f.has_parse_errors {
                continue;
            }
            for name in decl_names(f.program()) {
                *decl_count.entry(name).or_default() += 1;
            }
        }

        let mut calls = Vec::new();
        let mut tear_offs = HashSet::new();
        let mut writes = HashMap::new();
        for file in files {
            if file
                .path
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("dart")
            {
                continue;
            }
            let ctx = AnalyzeContext::new(&file.path, config, &file.parsed)
                .with_types(&indexes.types)
                .with_identities(&indexes.identities)
                .with_signatures(&indexes.signatures);
            let mut nullability = ArgumentNullability {
                values: HashMap::new(),
                loops: HashMap::new(),
            };
            Visitor::visit_program(&mut nullability, file.program());
            let state = SemanticState::new(file.program(), &ctx)
                .expect("project identities describe the analyzed files");
            visit_program(&mut nullability, file.program(), state);
            let mut collector = CallCollector {
                calls: Vec::new(),
                tear_offs: HashSet::new(),
                callee: None,
                nullability: &nullability.values,
                declarations: &decl_count,
            };
            collector.visit_program(file.program());
            calls.extend(collector.calls);
            tear_offs.extend(collector.tear_offs);
            writes.insert(file.path.as_path(), nullability.values);
        }

        let mut diags = Vec::new();
        for f in files {
            if f.has_parse_errors {
                continue;
            }
            let Some(file_writes) = writes.get(f.path.as_path()) else {
                continue;
            };
            for target in private_targets(f.program()) {
                if decl_count.get(&target.name).copied() != Some(1) {
                    continue; // ambiguous name — can't attribute call sites
                }
                // Used as a value (tear-off): callers are opaque, so bail.
                if tear_offs.contains(&target.name) {
                    continue;
                }
                let calls: Vec<&CallInfo> =
                    calls.iter().filter(|c| c.name == target.name).collect();
                if calls.is_empty() {
                    continue; // no evidence of how it's called
                }
                let assigned_null = body_null_assignments(target.body, file_writes);
                for param in &target.nullable_params {
                    if assigned_null.contains(&param.name) {
                        continue;
                    }
                    let default_is_null = param.default.is_none_or(|value| {
                        let span = value.span();
                        file_writes
                            .get(&(span.start, span.end))
                            .copied()
                            .unwrap_or(true)
                    });
                    if calls.iter().any(|c| c.passes_null(param, default_is_null)) {
                        continue;
                    }
                    diags.push(Diagnostic::new(
                        "",
                        Severity::Warning,
                        format!(
                            "Parameter `{}` is never null at any call site; \
                             remove the unnecessary `?`.",
                            param.name
                        ),
                        f.path.to_string_lossy().into_owned(),
                        param.span,
                    ));
                }
            }
        }
        diags
    }
}

/// Where a nullable parameter sits in a call's argument list.
enum ParamSlot {
    /// Index among positional parameters; `optional` marks an optional
    /// positional parameter, which a call may legally omit.
    Positional { index: usize, optional: bool },
    /// Named parameter; `optional` when it is not `required`.
    Named { optional: bool },
}

/// A nullable parameter of a private declaration.
struct NullableParam<'a> {
    name: &'a str,
    span: Span,
    slot: ParamSlot,
    default: Option<Expr<'a>>,
}

/// A private top-level function or method under consideration.
struct Target<'a> {
    name: &'a str,
    nullable_params: Vec<NullableParam<'a>>,
    body: Option<FunctionBody<'a>>,
}

/// Collect the private functions/methods of a program with their nullable
/// parameters. Only `_`-prefixed names are considered.
fn private_targets(program: Program<'_>) -> Vec<Target<'_>> {
    let mut out = Vec::new();
    for decl in program.declarations() {
        match decl {
            TopLevelDecl::Function(f)
                if is_private(f.name().name()) && matches!(f.kind(), FunctionKind::Function(_)) =>
            {
                if let Some(params) = f.params() {
                    push_target(&mut out, f.name().name(), params, f.body());
                }
            }
            _ => {
                if let Some(members) = crate::lint::declarations::members_of(decl) {
                    collect_method_targets(&mut out, members);
                }
            }
        }
    }
    out
}

fn collect_method_targets<'a>(
    out: &mut Vec<Target<'a>>,
    members: AstChildren<'a, ClassMember<'a>>,
) {
    for member in members {
        if let ClassMember::Method(m) = member
            && is_private(m.name().name())
        {
            push_target(out, m.name().name(), m.params(), m.body());
        }
    }
}

fn push_target<'a>(
    out: &mut Vec<Target<'a>>,
    name: &'a str,
    params: FormalParamList<'a>,
    body: Option<FunctionBody<'a>>,
) {
    let nullable_params = nullable_params(params);
    if nullable_params.is_empty() {
        return;
    }
    out.push(Target {
        name,
        nullable_params,
        body,
    });
}

/// Extract the nullable parameters of a formal list with their call-site slots.
fn nullable_params(params: FormalParamList<'_>) -> Vec<NullableParam<'_>> {
    let mut out = Vec::new();
    for (i, p) in params.positional().enumerate() {
        if let Some(np) = make_param(
            p,
            ParamSlot::Positional {
                index: i,
                optional: matches!(p.kind(), ParamKind::OptionalPositional),
            },
        ) {
            out.push(np);
        }
    }
    for p in params.named() {
        if let Some(np) = make_param(
            p,
            ParamSlot::Named {
                optional: matches!(p.kind(), ParamKind::Named { required: false }),
            },
        ) {
            out.push(np);
        }
    }
    out
}

fn make_param(p: FormalParam<'_>, slot: ParamSlot) -> Option<NullableParam<'_>> {
    // Skip constructor field/super params and function-typed params — out of
    // scope; only a written nullable type qualifies.
    if p.initializing().is_some() || p.function_params().is_some() {
        return None;
    }
    let ty = p.param_type()?;
    if !ty.is_nullable() {
        return None;
    }
    let default = p.default_value();
    Some(NullableParam {
        name: p.name().name(),
        span: p.name().span(),
        slot,
        default,
    })
}

fn is_private(name: &str) -> bool {
    name.starts_with('_')
}

/// Names of top-level functions and methods (for the ambiguity count).
fn decl_names(program: Program<'_>) -> Vec<&str> {
    let mut out = Vec::new();
    for decl in program.declarations() {
        match decl {
            TopLevelDecl::Function(f)
                if !matches!(f.kind(), FunctionKind::Getter)
                    && !matches!(f.kind(), FunctionKind::Setter(_)) =>
            {
                out.push(f.name().name());
            }
            _ => {
                if let Some(members) = crate::lint::declarations::members_of(decl) {
                    method_names(&mut out, members);
                }
            }
        }
    }
    out
}

fn method_names<'a>(out: &mut Vec<&'a str>, members: AstChildren<'a, ClassMember<'a>>) {
    for member in members {
        if let ClassMember::Method(m) = member {
            out.push(m.name().name());
        }
    }
}

// ── Call-site collection ──────────────────────────────────────────────────────

/// A single call's argument shape relevant to nullability.
struct CallInfo<'a> {
    name: &'a str,
    positional_maybe_null: Vec<bool>,
    named_maybe_null: HashMap<&'a str, bool>,
}

impl CallInfo<'_> {
    /// Whether this call *might* pass null (or omit-as-null) for `param`. A `true`
    /// here suppresses the diagnostic — the rule only flags a param when every
    /// call is proven to pass a non-null value.
    fn passes_null(&self, param: &NullableParam, default_is_null: bool) -> bool {
        match &param.slot {
            ParamSlot::Positional { index, optional } => {
                match self.positional_maybe_null.get(*index) {
                    Some(maybe_null) => *maybe_null,
                    // Argument omitted: for an optional param that defaults to null,
                    // omission means null; a required param can't legally be omitted.
                    None => *optional && default_is_null,
                }
            }
            ParamSlot::Named { optional } => match self.named_maybe_null.get(&param.name) {
                Some(maybe_null) => *maybe_null,
                None => *optional && default_is_null,
            },
        }
    }
}

fn call_info<'a>(
    name: &'a str,
    args: ArgList<'a>,
    collector: &CallCollector<'_, '_>,
) -> CallInfo<'a> {
    let maybe_null = |arg: Expr<'_>| {
        let span = arg.span();
        collector
            .nullability
            .get(&(span.start, span.end))
            .copied()
            .unwrap_or(true)
    };
    CallInfo {
        name,
        positional_maybe_null: args.positional().map(maybe_null).collect(),
        named_maybe_null: args
            .named()
            .map(|arg| (arg.name().name(), maybe_null(arg.value())))
            .collect(),
    }
}

fn callee_name(callee: Expr<'_>) -> Option<&str> {
    match callee.unparenthesized() {
        Expr::Ident(id) => Some(id.name()),
        Expr::Field(field) => Some(field.field().name()),
        Expr::DotShorthand(shorthand) => Some(shorthand.name().name()),
        _ => None,
    }
}

struct CallCollector<'a, 'n> {
    calls: Vec<CallInfo<'a>>,
    /// Names used as bare values (tear-offs), not as a call callee.
    tear_offs: HashSet<&'a str>,
    /// The direct callee is not a tear-off, even through parentheses.
    callee: Option<Span>,
    nullability: &'n HashMap<(usize, usize), bool>,
    declarations: &'n HashMap<&'a str, usize>,
}

struct ArgumentNullability {
    values: HashMap<(usize, usize), bool>,
    loops: HashMap<(usize, usize), (Span, bool)>,
}

impl ArgumentNullability {
    fn record<'a>(&mut self, expr: Expr<'a>, state: &SemanticState<'a>) {
        let span = expr.span();
        let mut shadow = MemberDependency {
            state,
            found: false,
        };
        if state.enclosing_type.is_some() {
            shadow.visit_expr(expr);
        }
        self.values.insert(
            (span.start, span.end),
            shadow.found || !state.signatures.is_non_nullable(&state.infer(expr)),
        );
    }

    fn record_loop(&mut self, header: ForHeader<'_>, is_await: bool) {
        if let Some(ForInit::ForIn(initializer)) = header.init()
            && !initializer.declares_variable()
        {
            let span = initializer.iterable().span();
            let target = initializer.name().span();
            self.loops
                .insert((span.start, span.end), (target, is_await));
            self.values.insert((target.start, target.end), true);
        }
    }

    fn record_arguments(&mut self, args: ArgList<'_>) {
        for value in args.positional().chain(args.named().map(NamedArg::value)) {
            let span = value.span();
            self.values.insert((span.start, span.end), true);
        }
    }
}

impl<'a> Visitor<'a> for ArgumentNullability {
    fn visit_stmt(&mut self, statement: Stmt<'a>) {
        if let Stmt::For(statement) = statement {
            self.record_loop(statement.header(), statement.is_await());
        }
        falcon_syntax::cst::visit::walk_stmt(self, statement);
    }

    fn visit_element<E: Element<'a>>(&mut self, element: E) {
        let kind = element.kind();
        if let ElementKind::For { header, .. } = kind {
            self.record_loop(header, element.is_await());
        }
        falcon_syntax::cst::visit::walk_element(self, kind);
    }

    fn visit_formal_param(&mut self, parameter: FormalParam<'a>) {
        if let Some(value) = parameter.default_value() {
            let span = value.span();
            self.values.insert((span.start, span.end), true);
        }
        falcon_syntax::cst::visit::walk_formal_param(self, parameter);
    }

    fn visit_expr(&mut self, expression: Expr<'a>) {
        walk_expr_chain(self, expression, |visitor, expression| {
            match expression {
                Expr::Assign(assignment) => {
                    let span = assignment.value().span();
                    visitor.values.insert((span.start, span.end), true);
                }
                Expr::Call(call) => visitor.record_arguments(call.args()),
                Expr::Cascade(cascade) => {
                    for section in cascade.sections() {
                        for operation in section.ops() {
                            if let CascadeOp::Call(call) = operation {
                                visitor.record_arguments(call.args());
                            }
                        }
                    }
                }
                _ => {}
            }
            true
        });
    }
}

/// Member-dependent expressions stay conservative until inference can prove
/// their dispatch independently of a same-named library declaration.
struct MemberDependency<'s, 'a> {
    state: &'s SemanticState<'a>,
    found: bool,
}

impl<'a> Visitor<'a> for MemberDependency<'_, 'a> {
    fn visit_expr(&mut self, expr: Expr<'a>) {
        walk_expr_chain(self, expr, |visitor, expr| {
            if visitor.found {
                return false;
            }
            if matches!(expr, Expr::Ident(_))
                && visitor
                    .state
                    .referenced_declaration(expr, false)
                    .is_some_and(|reference| !reference.selector.name.is_empty())
            {
                visitor.found = true;
                false
            } else {
                true
            }
        });
    }
}

impl<'a> SemanticRuleVisitor<'a> for ArgumentNullability {
    fn visit_expr(&mut self, expr: Expr<'a>, state: &SemanticState<'a>) {
        let span = expr.span();
        if self.values.contains_key(&(span.start, span.end)) {
            self.record(expr, state);
        }
        if let Some(&(target, is_await)) = self.loops.get(&(span.start, span.end)) {
            let element = falcon_analyze::FlowContext {
                model: &state.model,
                signatures: &state.signatures,
                type_parameters: &state.type_parameters,
                field_promotions: false,
            }
            .iterable_element(&state.infer(expr), is_await);
            self.values.insert(
                (target.start, target.end),
                !state.signatures.is_non_nullable(&element),
            );
        }
    }
}

impl<'a> Visitor<'a> for CallCollector<'a, '_> {
    fn visit_expr(&mut self, node: Expr<'a>) {
        walk_expr_chain(self, node, |visitor, node| {
            if visitor.callee == Some(node.span()) {
                visitor.callee = None;
                return matches!(node, Expr::Field(_));
            }
            match node {
                Expr::Call(call) => {
                    let callee = call.callee().unparenthesized();
                    if let Some(name) = callee_name(callee) {
                        visitor.calls.push(call_info(name, call.args(), visitor));
                    }
                    // Visit the receiver and arguments, but not the callee name
                    // as a tear-off.
                    if matches!(
                        callee,
                        Expr::Ident(_) | Expr::DotShorthand(_) | Expr::Field(_)
                    ) {
                        visitor.callee = Some(callee.span());
                    }
                }
                Expr::DotShorthand(shorthand)
                    if visitor.declarations.contains_key(shorthand.name().name()) =>
                {
                    visitor.tear_offs.insert(shorthand.name().name());
                }
                Expr::Field(field) => {
                    if visitor.declarations.contains_key(field.field().name()) {
                        visitor.tear_offs.insert(field.field().name());
                    }
                }
                Expr::Ident(id) => {
                    visitor.tear_offs.insert(id.name());
                }
                _ => {}
            }
            true
        });
    }

    fn visit_cascade_op(&mut self, operation: CascadeOp<'a>) {
        match operation {
            CascadeOp::Call(call) => {
                let args = call.args();
                self.calls.push(call_info(call.name().name(), args, self));
                for argument in args.positional() {
                    self.visit_expr(argument);
                }
                for argument in args.named() {
                    self.visit_expr(argument.value());
                }
            }
            CascadeOp::Invoke(call) => {
                for argument in call.args().positional() {
                    self.visit_expr(argument);
                }
                for argument in call.args().named() {
                    self.visit_expr(argument.value());
                }
            }
            CascadeOp::Index(index) => self.visit_expr(index.index()),
            CascadeOp::Assign(assign) => {
                self.visit_expr(assign.target());
                self.visit_expr(assign.value());
            }
            CascadeOp::IndexAssign(assign) => {
                self.visit_expr(assign.target());
                self.visit_expr(assign.value());
            }
            CascadeOp::Field(field) if self.declarations.contains_key(field.name().name()) => {
                self.tear_offs.insert(field.name().name());
            }
            CascadeOp::Field(_) | CascadeOp::NullAssert(_) => {}
        }
    }
}

// ── Body null-assignment scan ─────────────────────────────────────────────────

/// Parameter names assigned a value that is not proven non-null.
fn body_null_assignments<'a>(
    body: Option<FunctionBody<'a>>,
    nullability: &HashMap<(usize, usize), bool>,
) -> HashSet<&'a str> {
    let mut finder = NullAssignFinder {
        names: HashSet::new(),
        nullability,
    };
    if let Some(body) = body {
        walk_function_body(&mut finder, body);
    }
    finder.names
}

struct NullAssignFinder<'a, 'n> {
    names: HashSet<&'a str>,
    nullability: &'n HashMap<(usize, usize), bool>,
}

impl<'a> NullAssignFinder<'a, '_> {
    fn record_loop(&mut self, header: ForHeader<'a>) {
        if let Some(ForInit::ForIn(initializer)) = header.init()
            && !initializer.declares_variable()
            && self
                .nullability
                .get(&(
                    initializer.name().span().start,
                    initializer.name().span().end,
                ))
                .copied()
                .unwrap_or(true)
        {
            self.names.insert(initializer.name().name());
        }
    }
}

struct PatternWrites<'a, 'n>(&'n mut HashSet<&'a str>);

impl<'a> Visitor<'a> for PatternWrites<'a, '_> {
    fn visit_identifier(&mut self, name: Identifier<'a>) {
        self.0.insert(name.name());
    }
}

impl<'a> Visitor<'a> for NullAssignFinder<'a, '_> {
    fn visit_stmt(&mut self, statement: Stmt<'a>) {
        match statement {
            Stmt::PatternAssign(assignment) => {
                PatternWrites(&mut self.names).visit_pattern(assignment.pattern());
            }
            Stmt::For(statement) => self.record_loop(statement.header()),
            _ => {}
        }
        falcon_syntax::cst::visit::walk_stmt(self, statement);
    }

    fn visit_element<E: Element<'a>>(&mut self, element: E) {
        let kind = element.kind();
        if let ElementKind::For { header, .. } = kind {
            self.record_loop(header);
        }
        falcon_syntax::cst::visit::walk_element(self, kind);
    }

    fn visit_expr(&mut self, node: Expr<'a>) {
        walk_expr_chain(self, node, |visitor, node| {
            if let Expr::Assign(assign) = node {
                match assign.target().unparenthesized() {
                    Expr::Ident(id) => {
                        if !matches!(assign.op(), AssignOp::Eq | AssignOp::NullCoalesceEq)
                            || visitor
                                .nullability
                                .get(&(assign.value().span().start, assign.value().span().end))
                                .copied()
                                .unwrap_or(true)
                        {
                            visitor.names.insert(id.name());
                        }
                    }
                    Expr::Field(_) | Expr::Index(_) => {}
                    target => PatternWrites(&mut visitor.names).visit_expr(target),
                }
            }
            let increment_target = match node {
                Expr::Unary(unary)
                    if matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus) =>
                {
                    Some(unary.operand())
                }
                Expr::PostfixIncDec(increment) => Some(increment.operand()),
                _ => None,
            };
            if let Some(Expr::Ident(identifier)) = increment_target.map(Expr::unparenthesized) {
                visitor.names.insert(identifier.name());
            }
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::CrossFileRuleAnalysis;

    fn count(source: &str) -> usize {
        let config = FalconConfig::default();
        let sources = vec![("test.dart".into(), source.into())];
        let (diagnostics, files, indexes) = falcon_analyze::analyze_sequential_collecting_resolving(
            &falcon_analyze::RuleRegistry::new(),
            &sources,
            &config,
            true,
            true,
            falcon_analyze::Workspace::default(),
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        UnnecessaryNullable
            .analyze_project(
                "unnecessary-nullable",
                &falcon_analyze::AnalysisEnvironment::default(),
                &files,
                &config,
                indexes.as_ref(),
            )
            .len()
    }

    #[test]
    fn property_reads_only_record_declared_callable_names() {
        let source = "void _top(int? value) {} class C { int _field = 1; void _method(int? value) {} void _called(int? value) {} void use(C c) { c._field; c.._field; c._method; c.._method; c._called(1); (c._called)(2); c.._called(3); _top(4); (_top)(5); } }";
        let file = ProjectFile {
            path: "test.dart".into(),
            parsed: std::sync::Arc::new(falcon_analyze::ParsedFile {
                source: source.into(),
                tree: falcon_dart_parser::parse(source).tree,
            }),
            has_parse_errors: false,
        };
        let declarations = decl_names(file.program())
            .into_iter()
            .map(|name| (name, 1))
            .collect();
        let nullability = HashMap::new();
        let mut collector = CallCollector {
            calls: Vec::new(),
            tear_offs: HashSet::new(),
            callee: None,
            nullability: &nullability,
            declarations: &declarations,
        };
        collector.visit_program(file.program());
        assert_eq!(collector.tear_offs, HashSet::from(["_method", "c"]));
        assert_eq!(
            collector
                .calls
                .iter()
                .map(|call| call.name)
                .collect::<Vec<_>>(),
            ["_called", "_called", "_called", "_top", "_top"]
        );
    }

    #[test]
    fn unbounded_type_parameters_are_not_nonnull_proof() {
        assert_eq!(
            count("void _take(Object? value) {} void use<T>(T value) { _take(value); }"),
            0
        );
        assert_eq!(
            count(
                "void _take<T>(Object? value, T other) { value = other; } void use<T>(T other) { _take(1, other); }"
            ),
            0
        );
        assert_eq!(
            count(
                "void _take(Object? value) {} void use<T extends Object>(T value) { _take(value); }"
            ),
            1
        );
    }

    #[test]
    fn for_in_writes_use_the_iterable_element_type() {
        for body in [
            "for (value in <int>[1, 2]) {}",
            "final values = [for (value in <int>[1, 2]) value];",
            "final values = {for (value in <int>[1, 2]) 1: value};",
        ] {
            assert_eq!(
                count(&format!(
                    "void _take(int? value) {{ {body} }} void use() {{ _take(1); }}"
                )),
                1,
                "{body}"
            );
        }
        for iterable in ["<int?>[null]", "unknown"] {
            assert_eq!(
                count(&format!(
                    "void _take(int? value, dynamic unknown) {{ for (value in {iterable}) {{}} }} void use() {{ _take(1, []); }}"
                )),
                0,
                "{iterable}"
            );
        }
        assert_eq!(
            count(
                "void _take<T>(Object? value, Iterable<T> values) { for (value in values) {} } void use<T>(Iterable<T> values) { _take(1, values); }"
            ),
            0
        );
    }

    #[test]
    #[should_panic(expected = "unnecessary-nullable requires semantic indexes")]
    fn semantic_indexes_are_required_even_for_empty_projects() {
        UnnecessaryNullable.analyze_project(
            "unnecessary-nullable",
            &falcon_analyze::AnalysisEnvironment::default(),
            &[],
            &FalconConfig::default(),
            None,
        );
    }

    #[test]
    fn implicit_writes_and_nullable_defaults_preserve_nullable_parameters() {
        for body in [
            "final result = ([value] = <int?>[null]);",
            "for (int i = 0; i < 1; [value] = <int?>[null]) { i++; }",
            "final values = {for (value in <int?>[null]) 1: 1};",
            "[value] = <int?>[null];",
            "for (value in <int?>[null]) {}",
            "final values = [for (value in <int?>[null]) value];",
        ] {
            assert_eq!(
                count(&format!(
                    "void _take(int? value) {{ {body} }} void use() {{ _take(1); }}"
                )),
                0,
                "{body}"
            );
        }
        for parameter in ["[int? value = none]", "{int? value = none}"] {
            assert_eq!(
                count(&format!(
                    "const int? none = null; void _take({parameter}) {{}} void use() {{ _take(); }}"
                )),
                0,
                "{parameter}"
            );
        }
        assert_eq!(
            count("void _take([int? value = 1]) {} void use() { _take(); }"),
            1
        );
        assert_eq!(
            count("void _take({int? value = 1}) {} void use() { _take(); }"),
            1
        );
    }

    #[test]
    fn member_and_index_writes_do_not_reassign_parameters() {
        assert_eq!(
            count(
                "class Foo { int x = 0; } void _set(Foo? foo) { foo?.x = 1; } void use() { _set(Foo()); }"
            ),
            1
        );
        assert_eq!(
            count(
                "void _set(List<int> items, int? index) { items[index!] = 1; } void use() { _set([0], 0); }"
            ),
            1
        );
    }

    #[test]
    fn arguments_are_inferred_after_prior_argument_writes() {
        assert_eq!(
            count(
                "int? maybe() => null; void _take(int a, int? b) {} \
             void use() { int? x = maybe(); if (x == null) return; \
             _take((x = null) == null ? 1 : 2, x); }"
            ),
            0
        );
    }

    #[test]
    fn shorthand_calls_contribute_nullable_arguments() {
        assert_eq!(
            count(
                "class C { static C _make(int? value) => C(); \
             static void use() { C a = ._make(null); _make(1); } }"
            ),
            0
        );
    }

    #[test]
    fn invoked_callback_results_preserve_private_function_escape_evidence() {
        for invocation in ["holder..callback()(null)", "holder.callback()(null)"] {
            let source = format!(
                "void _take(int? value) {{}} \
                 class Holder {{ void Function(int?) callback() => _take; }} \
                 void use(Holder holder) {{ _take(1); {invocation}; }}"
            );
            assert_eq!(count(&source), 0, "{invocation}");
        }
        assert_eq!(
            count("void _take(int? value) {} void use() { _take(1); }"),
            1
        );
    }

    #[test]
    fn method_tear_offs_escape_even_with_nonnull_direct_calls() {
        for reference in [
            "this._take",
            "_take",
            "this.._take",
            "this.._take.call(null)",
        ] {
            let source = format!(
                "void accept(dynamic callback) {{}} class C {{ \
                 void _take(int? value) {{}} \
                 void use() {{ _take(1); accept({reference}); }} }}"
            );
            assert_eq!(count(&source), 0, "{reference}");
        }
    }

    #[test]
    fn parameter_writes_require_nonnull_proof() {
        for value in ["maybe()", "unknown", "flag ? 1 : null"] {
            let source = format!(
                "int? maybe() => null; void _take(int? value) {{ \
                 dynamic unknown; bool flag = true; value = {value}; }} \
                 void use() {{ _take(1); }}"
            );
            assert_eq!(count(&source), 0, "{value}");
        }
        assert_eq!(
            count("void _take(int? value) { value = 2; } void use() { _take(1); }"),
            1
        );
    }

    #[test]
    fn package_import_proves_nonnull_with_retained_indexes() {
        let root = std::env::temp_dir().join(format!(
            "falcon-nullable-package-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("lib")).unwrap();
        let manifest = root.join("pubspec.yaml");
        std::fs::write(&manifest, "name: app\n").unwrap();
        let config = FalconConfig::default();
        let mut registry = falcon_analyze::CrossFileRuleRegistry::new();
        registry.register("unnecessary-nullable", Box::new(UnnecessaryNullable));
        for (return_type, value, expected) in [("int", "1", 1), ("int?", "null", 0)] {
            let sources = vec![
                (root.join("lib/a.dart"), format!("{return_type} value() => {value};")),
                (
                    root.join("lib/b.dart"),
                    "import 'package:app/a.dart'; void _take(int? x) {} void f() { _take(value()); }".into(),
                ),
            ];
            for (path, source) in &sources {
                std::fs::write(path, source).unwrap();
            }
            let packages =
                falcon_analyze::package_identities(sources.iter().map(|(path, _)| path.as_path()));
            let (diagnostics, mut files, indexes) =
                falcon_analyze::analyze_sequential_collecting_resolving(
                    &falcon_analyze::RuleRegistry::new(),
                    &sources,
                    &config,
                    true,
                    true,
                    falcon_analyze::Workspace {
                        packages: &packages,
                        environment: None,
                    },
                );
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            let tree = falcon_dart_parser::lex_tree("name: app\n");
            files.push(ProjectFile {
                path: manifest.clone(),
                parsed: std::sync::Arc::new(falcon_analyze::ParsedFile {
                    source: "name: app\n".into(),
                    tree,
                }),
                has_parse_errors: false,
            });
            let diagnostics = registry
                .run_all(
                    &falcon_analyze::AnalysisEnvironment::default(),
                    &files,
                    &config,
                    indexes.as_ref(),
                )
                .unwrap();
            assert_eq!(
                diagnostics.len(),
                expected,
                "{return_type}: {diagnostics:?}"
            );
            if expected == 1 {
                assert_eq!(diagnostics[0].rule, "unnecessary-nullable");
                assert_eq!(diagnostics[0].file_path, sources[1].0.to_string_lossy());
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn known_return_proves_nonnull_but_local_shadow_does_not() {
        assert_eq!(
            count("int value() => 1; void _take(int? x) {} void f() { _take(value()); }"),
            1
        );
        assert_eq!(
            count(
                "int value() => 1; void _take(int? x) {} void f(int? Function() value) { _take(value()); }"
            ),
            0
        );
        assert_eq!(
            count(
                "int value() => 1; void _take(int? x) {} void f() { int? value() => null; _take(value()); }"
            ),
            0
        );
        assert_eq!(
            count("void _take(int? x) {} void f(int? x) { _take(x); }"),
            0
        );
        assert_eq!(
            count(
                "int value() => 1; void _take(int? x) {} class C { int? Function() get value => () => null; void f() { _take(value()); } }"
            ),
            0
        );
        assert_eq!(
            count(
                "int value() => 1; void _take(int? x) {} void f() { for (final value in <int? Function()>[() => null]) { _take(value()); } }"
            ),
            0
        );
    }
}
