use std::collections::{HashMap, HashSet};

use falcon_syntax::cst::visit::{
    Visitor, bound_names, walk_class_member, walk_element, walk_expr_chain, walk_function_body,
    walk_pattern,
};
use falcon_syntax::cst::*;

use super::lexical_scope::LexicalScopes;

#[derive(Clone, Copy)]
enum Binding {
    Parameter(usize),
    Declaration(NodeId),
    Other,
}

pub fn used_parameters<'a>(params: FormalParamList<'a>, body: FunctionBody<'a>) -> HashSet<usize> {
    let mut collector = UsageCollector::new(params);
    walk_function_body(&mut collector, body);
    collector.used
}

pub fn used_constructor_parameters<'a>(
    params: FormalParamList<'a>,
    initializers: AstChildren<'a, ConstructorInitializer<'a>>,
    body: Option<FunctionBody<'a>>,
) -> HashSet<usize> {
    let mut collector = UsageCollector::new(params);
    collector.visit_initializers(initializers);
    if let Some(body) = body {
        walk_function_body(&mut collector, body);
    }
    collector.used
}

/// Primary parameters are also in scope in eager instance field initializers,
/// but not in late/static fields or other members of the enclosing type.
pub fn used_primary_constructor_parameters<'a>(
    primary: PrimaryConstructor<'a>,
    members: AstChildren<'a, ClassMember<'a>>,
) -> HashSet<usize> {
    let mut collector = UsageCollector::new(primary.params());
    for member in members {
        match member {
            ClassMember::PrimaryConstructorBody(body) => {
                collector.visit_initializers(body.initializers());
                if let Some(body) = body.body() {
                    collector.scopes.push_scope();
                    walk_function_body(&mut collector, body);
                    collector.scopes.pop_scope();
                }
            }
            ClassMember::Field(field) if !field.is_static() && !field.is_late() => {
                for declarator in field.declarators() {
                    if let Some(initializer) = declarator.initializer() {
                        collector.visit_expr(initializer);
                    }
                }
            }
            _ => {}
        }
    }
    collector.used
}

/// How a local binding is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalWriteKind {
    /// `x = e`, `x += e`, `x ??= e`, ...
    Assign(AssignOp),
    /// `x++`, `x--`, `++x`, `--x`.
    IncDec,
    /// A pattern assignment `(x, y) = e;`. `direct` is true when the variable
    /// is an immediate field/element of the top-level record, object, list, or
    /// map pattern; `span` of the write is then the whole pattern.
    Pattern { direct: bool },
    /// A for-in loop over an existing variable: `for (x in xs)`.
    ForIn,
}

/// One write to a local variable or parameter, resolved lexically to its
/// declaration (identified by the declaring name's syntax node).
#[derive(Clone, Copy, Debug)]
pub struct LocalWrite {
    pub declaration: NodeId,
    pub kind: LocalWriteKind,
    pub span: Span,
}

/// Every write to a lexically scoped local (variable, parameter, loop or
/// pattern variable, catch variable) in `program`, in traversal order.
///
/// Writes inside closures resolve to the enclosing declaration unless a nearer
/// binding shadows the name, so the declarations named here are exactly the
/// analyzer's "potentially mutated in scope" set. Writes to names with no local
/// binding (fields, top-level variables) are not reported.
pub fn local_writes(program: Program<'_>) -> Vec<LocalWrite> {
    let mut collector = UsageCollector {
        scopes: LexicalScopes::new(),
        used: HashSet::new(),
        writes: Some(Vec::new()),
        bound_uses: None,
    };
    collector.visit_program(program);
    collector.writes.unwrap_or_default()
}

/// Start offsets of identifier expressions and constant-pattern name heads in
/// `program` that resolve to a lexically scoped local — a variable, parameter,
/// loop, pattern or catch variable, or local function — instead of a member or
/// top-level declaration.
pub fn locally_bound_identifiers(program: Program<'_>) -> HashSet<usize> {
    let mut collector = UsageCollector {
        scopes: LexicalScopes::new(),
        used: HashSet::new(),
        writes: None,
        bound_uses: Some(HashMap::new()),
    };
    collector.visit_program(program);
    collector
        .bound_uses
        .unwrap_or_default()
        .into_keys()
        .collect()
}

/// The declaration node of every identifier expression
/// and constant-pattern name head in `program` that resolves to a lexically
/// scoped local, keyed by the identifier's span start. Identifiers absent from
/// the map name members or top-level declarations.
pub fn local_declarations(program: Program<'_>) -> HashMap<usize, NodeId> {
    let mut collector = UsageCollector {
        scopes: LexicalScopes::new(),
        used: HashSet::new(),
        writes: Some(Vec::new()),
        bound_uses: Some(HashMap::new()),
    };
    collector.visit_program(program);
    collector
        .bound_uses
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(usage, declaration)| Some((usage, declaration?)))
        .collect()
}

struct UsageCollector {
    scopes: LexicalScopes<HashMap<String, Binding>>,
    used: HashSet<usize>,
    /// `Some` when every local binding is tracked and writes are recorded.
    writes: Option<Vec<LocalWrite>>,
    /// `Some` when identifier expressions and constant-pattern name heads that
    /// resolve to a local are recorded, with their declaration in write mode.
    bound_uses: Option<HashMap<usize, Option<NodeId>>>,
}

impl UsageCollector {
    fn new(params: FormalParamList<'_>) -> Self {
        let mut collector = Self {
            scopes: LexicalScopes::new(),
            used: HashSet::new(),
            writes: None,
            bound_uses: None,
        };
        for param in params.params() {
            collector.scopes.declare(
                param.name().name().to_owned(),
                Binding::Parameter(param.name().span().start),
            );
        }
        collector
    }

    fn use_name(&mut self, name: &str) {
        if let Some((_, Binding::Parameter(span_start))) = self.scopes.lookup(name) {
            self.used.insert(*span_start);
        }
    }

    fn use_identifier(&mut self, identifier: Identifier<'_>) {
        if let Some((_, binding)) = self.scopes.lookup(identifier.name())
            && let Some(bound) = self.bound_uses.as_mut()
        {
            let declaration = match binding {
                Binding::Declaration(declaration) => Some(*declaration),
                Binding::Parameter(_) | Binding::Other => None,
            };
            bound.insert(identifier.span().start, declaration);
        }
        self.use_name(identifier.name());
    }

    /// Declares a local binding; tracked as a write target in write mode.
    fn bind(&mut self, name: Identifier<'_>) {
        let binding = if self.writes.is_some() {
            Binding::Declaration(name.syntax().id())
        } else {
            Binding::Other
        };
        self.scopes.declare(name.name().to_owned(), binding);
    }

    fn record_write(&mut self, name: &str, kind: LocalWriteKind, span: Span) {
        if self.writes.is_none() {
            return;
        }
        if let Some((_, Binding::Declaration(declaration))) = self.scopes.lookup(name)
            && let Some(writes) = self.writes.as_mut()
        {
            writes.push(LocalWrite {
                declaration: *declaration,
                kind,
                span,
            });
        }
    }

    fn record_target_write(&mut self, target: Expr<'_>, kind: LocalWriteKind, span: Span) {
        if let Expr::Ident(identifier) = target.unparenthesized() {
            self.record_write(identifier.name(), kind, span);
        }
    }

    fn bind_params(&mut self, params: FormalParamList<'_>) {
        for param in params.params() {
            self.bind(param.name());
        }
    }

    fn bind_pattern(&mut self, pattern: Pattern<'_>) {
        for name in bound_names(pattern) {
            self.bind(name);
        }
    }

    fn visit_initializers<'a>(
        &mut self,
        initializers: AstChildren<'a, ConstructorInitializer<'a>>,
    ) {
        for initializer in initializers {
            match initializer {
                ConstructorInitializer::SuperCall(call) => self.visit_arg_list(call.args()),
                ConstructorInitializer::ThisCall(call) => self.visit_arg_list(call.args()),
                ConstructorInitializer::FieldInit(field) => self.visit_expr(field.value()),
                ConstructorInitializer::Assert(assertion) => {
                    self.visit_expr(assertion.condition());
                    if let Some(message) = assertion.message() {
                        self.visit_expr(message);
                    }
                }
            }
        }
    }

    fn visit_declared_function<'a>(
        &mut self,
        params: Option<FormalParamList<'a>>,
        body: Option<FunctionBody<'a>>,
    ) {
        self.scopes.push_scope();
        if let Some(params) = params {
            self.bind_params(params);
        }
        if let Some(body) = body {
            walk_function_body(self, body);
        }
        self.scopes.pop_scope();
    }

    fn visit_nested_function<'a>(&mut self, params: FormalParamList<'a>, body: FunctionBody<'a>) {
        self.scopes.push_scope();
        self.bind_params(params);
        walk_function_body(self, body);
        self.scopes.pop_scope();
    }

    fn visit_scoped_stmt(&mut self, statement: Stmt<'_>) {
        self.scopes.push_scope();
        self.visit_stmt(statement);
        self.scopes.pop_scope();
    }

    fn visit_local_declaration(&mut self, declaration: LocalVarDecl<'_>) {
        for declarator in declaration.declarators() {
            if let Some(initializer) = declarator.initializer() {
                self.visit_expr(initializer);
            }
            self.bind(declarator.name());
        }
    }

    fn visit_for_init(&mut self, init: ForInit<'_>) {
        match init {
            ForInit::VarDecl(declaration) => self.visit_local_declaration(declaration),
            ForInit::ForIn(init) => {
                self.visit_expr(init.iterable());
                let name = init.name();
                if init.declares_variable() {
                    self.bind(name);
                } else {
                    self.use_name(name.name());
                    self.record_write(name.name(), LocalWriteKind::ForIn, name.span());
                }
            }
            ForInit::PatternForIn(init) => {
                self.visit_expr(init.iterable());
                self.visit_pattern(init.pattern());
                self.bind_pattern(init.pattern());
            }
            ForInit::Exprs(expressions) => {
                for expression in expressions.exprs() {
                    self.visit_expr(expression);
                }
            }
        }
    }

    fn visit_for_header(&mut self, header: ForHeader<'_>) {
        if let Some(init) = header.init() {
            self.visit_for_init(init);
        }
        if let Some(condition) = header.condition() {
            self.visit_expr(condition);
        }
        for update in header.updates() {
            self.visit_expr(update);
        }
    }

    fn visit_if_usage(&mut self, statement: IfStmt<'_>) {
        match statement.condition() {
            IfCondition::Expr(condition) => {
                self.visit_expr(condition);
                self.visit_scoped_stmt(statement.then_branch());
                if let Some(else_branch) = statement.else_branch() {
                    self.visit_scoped_stmt(else_branch);
                }
            }
            IfCondition::Case(case) => {
                self.visit_expr(case.expr());
                let pattern = case.pattern();
                let guard = case.guard();
                self.scopes.push_scope();
                self.visit_pattern(pattern);
                self.bind_pattern(pattern);
                if let Some(guard) = guard {
                    self.visit_expr(guard);
                }
                self.visit_scoped_stmt(statement.then_branch());
                self.scopes.pop_scope();
                if let Some(else_branch) = statement.else_branch() {
                    self.visit_scoped_stmt(else_branch);
                }
            }
        }
    }

    fn visit_switch_usage(&mut self, statement: SwitchStmt<'_>) {
        self.visit_expr(statement.subject());
        for case in statement.cases() {
            self.scopes.push_scope();
            for kind in case.cases() {
                if let SwitchCaseKind::Pattern(label) = kind {
                    self.visit_pattern(label.pattern());
                    self.bind_pattern(label.pattern());
                    if let Some(guard) = label.guard() {
                        self.visit_expr(guard);
                    }
                } else if let SwitchCaseKind::Expr(label) = kind {
                    self.visit_expr(label.expr());
                }
            }
            for statement in case.body() {
                self.visit_stmt(statement);
            }
            self.scopes.pop_scope();
        }
    }

    fn visit_try_catch_usage(&mut self, statement: TryCatchStmt<'_>) {
        self.scopes.push_scope();
        for statement in statement.body().stmts() {
            self.visit_stmt(statement);
        }
        self.scopes.pop_scope();
        for catch in statement.catches() {
            self.scopes.push_scope();
            if let Some(name) = catch.exception_var() {
                self.bind(name);
            }
            if let Some(name) = catch.stack_trace_var() {
                self.bind(name);
            }
            for statement in catch.body().stmts() {
                self.visit_stmt(statement);
            }
            self.scopes.pop_scope();
        }
        if let Some(finally) = statement.finally() {
            self.scopes.push_scope();
            for statement in finally.stmts() {
                self.visit_stmt(statement);
            }
            self.scopes.pop_scope();
        }
    }
}

impl<'a> Visitor<'a> for UsageCollector {
    fn visit_element<E: Element<'a>>(&mut self, element: E) {
        let kind = element.kind();
        match kind {
            ElementKind::If {
                condition,
                then_element,
                else_element,
            } => {
                let IfCondition::Case(case) = condition else {
                    walk_element(self, kind);
                    return;
                };
                let expression = case.expr();
                let pattern = case.pattern();
                let guard = case.guard();
                self.visit_expr(expression);
                self.scopes.push_scope();
                self.visit_pattern(pattern);
                self.bind_pattern(pattern);
                if let Some(guard) = guard {
                    self.visit_expr(guard);
                }
                self.visit_element(then_element);
                self.scopes.pop_scope();
                if let Some(element) = else_element {
                    self.visit_element(element);
                }
            }
            ElementKind::For { header, body, .. } => {
                self.scopes.push_scope();
                self.visit_for_header(header);
                self.visit_element(body);
                self.scopes.pop_scope();
            }
            kind => walk_element(self, kind),
        }
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) {
        self.visit_declared_function(node.params(), node.body());
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) {
        self.visit_declared_function(Some(node.params()), node.body());
    }

    fn visit_getter_decl(&mut self, node: GetterDecl<'a>) {
        self.visit_declared_function(None, node.body());
    }

    fn visit_setter_decl(&mut self, node: SetterDecl<'a>) {
        self.scopes.push_scope();
        self.bind(node.param());
        if let Some(body) = node.body() {
            walk_function_body(self, body);
        }
        self.scopes.pop_scope();
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) {
        match node {
            ClassMember::Operator(operator) => {
                self.visit_declared_function(Some(operator.params()), operator.body())
            }
            _ => walk_class_member(self, node),
        }
    }

    fn visit_constructor_decl(&mut self, node: ConstructorDecl<'a>) {
        // Initializing formals are in scope only in the initializer list; in the
        // body `x` names the field again.
        self.scopes.push_scope();
        for param in node
            .params()
            .params()
            .filter(|p| p.initializing().is_none())
        {
            self.bind(param.name());
        }
        self.scopes.push_scope();
        for param in node
            .params()
            .params()
            .filter(|p| p.initializing().is_some())
        {
            self.bind(param.name());
        }
        self.visit_initializers(node.initializers());
        self.scopes.pop_scope();
        if let Some(body) = node.body() {
            walk_function_body(self, body);
        }
        self.scopes.pop_scope();
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) {
        match node {
            Stmt::Block(block) => {
                self.scopes.push_scope();
                for statement in block.stmts() {
                    self.visit_stmt(statement);
                }
                self.scopes.pop_scope();
            }
            Stmt::LocalVar(declaration) => self.visit_local_declaration(declaration),
            Stmt::PatternDecl(declaration) => {
                self.visit_expr(declaration.init());
                self.visit_pattern(declaration.pattern());
                self.bind_pattern(declaration.pattern());
            }
            Stmt::PatternAssign(assignment) => {
                self.visit_pattern(assignment.pattern());
                // Pattern-assignment targets are writes, not reads, but the names
                // are declared elsewhere — counting them as uses keeps
                // unused-variable rules from flagging a variable this writes to.
                let direct = direct_pattern_variables(assignment.pattern());
                let span = assignment.pattern().span();
                for name in bound_names(assignment.pattern()) {
                    self.use_name(name.name());
                    let direct = direct.iter().any(|d| d.span() == name.span());
                    self.record_write(name.name(), LocalWriteKind::Pattern { direct }, span);
                }
                self.visit_expr(assignment.value());
            }
            Stmt::If(statement) => self.visit_if_usage(statement),
            Stmt::For(statement) => {
                self.scopes.push_scope();
                self.visit_for_header(statement.header());
                self.visit_scoped_stmt(statement.body());
                self.scopes.pop_scope();
            }
            Stmt::Switch(statement) => self.visit_switch_usage(statement),
            Stmt::TryCatch(statement) => self.visit_try_catch_usage(statement),
            Stmt::LocalFunc(function) => {
                self.bind(function.name());
                self.visit_nested_function(function.params(), function.body());
            }
            Stmt::While(statement) => {
                self.visit_expr(statement.condition());
                self.visit_scoped_stmt(statement.body());
            }
            Stmt::DoWhile(statement) => {
                self.visit_scoped_stmt(statement.body());
                self.visit_expr(statement.condition());
            }
            Stmt::Return(statement) => {
                if let Some(value) = statement.value() {
                    self.visit_expr(value);
                }
            }
            Stmt::Throw(statement) => self.visit_expr(statement.value()),
            Stmt::Labeled(statement) => self.visit_stmt(statement.stmt()),
            Stmt::Assert(statement) => {
                self.visit_expr(statement.condition());
                if let Some(message) = statement.message() {
                    self.visit_expr(message);
                }
            }
            Stmt::Yield(statement) => self.visit_expr(statement.value()),
            Stmt::Expr(statement) => self.visit_expr(statement.expr()),
            Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Rethrow(_)
            | Stmt::Empty(_)
            | Stmt::Error(_) => {}
        }
    }

    fn visit_pattern(&mut self, node: Pattern<'a>) {
        if let Pattern::Const(pattern) = node
            && let ConstPatternValue::Name(name) = pattern.value()
            && let Some(head) = name.first()
        {
            self.use_identifier(head);
        }
        walk_pattern(self, node);
    }

    fn visit_expr(&mut self, node: Expr<'a>) {
        walk_expr_chain(self, node, |collector, node| {
            match node {
                Expr::Ident(identifier) => collector.use_identifier(identifier),
                Expr::FuncExpr(function) => {
                    collector.visit_nested_function(function.params(), function.body())
                }
                Expr::List(list) => {
                    for element in list.elements() {
                        collector.visit_element(element);
                    }
                }
                Expr::Set(set) => {
                    for element in set.elements() {
                        collector.visit_element(element);
                    }
                }
                Expr::Map(map) => {
                    for element in map.elements() {
                        collector.visit_element(element);
                    }
                }
                Expr::Switch(switch) => {
                    collector.visit_expr(switch.subject());
                    for arm in switch.arms() {
                        collector.scopes.push_scope();
                        collector.visit_pattern(arm.pattern());
                        collector.bind_pattern(arm.pattern());
                        if let Some(guard) = arm.guard() {
                            collector.visit_expr(guard);
                        }
                        collector.visit_expr(arm.body());
                        collector.scopes.pop_scope();
                    }
                }
                Expr::Assign(assignment) => {
                    collector.record_target_write(
                        assignment.target(),
                        LocalWriteKind::Assign(assignment.op()),
                        assignment.span(),
                    );
                    return true;
                }
                Expr::PostfixIncDec(increment) => {
                    collector.record_target_write(
                        increment.operand(),
                        LocalWriteKind::IncDec,
                        increment.span(),
                    );
                    return true;
                }
                Expr::Unary(unary)
                    if matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus) =>
                {
                    collector.record_target_write(
                        unary.operand(),
                        LocalWriteKind::IncDec,
                        unary.span(),
                    );
                    return true;
                }
                _ => return true,
            }
            false
        });
    }
}

/// Variables that are immediate fields/elements of a top-level record, object,
/// list, or map pattern (the analyzer's direct `AssignedVariablePattern`s).
fn direct_pattern_variables(pattern: Pattern<'_>) -> Vec<Identifier<'_>> {
    fn direct(pattern: Pattern<'_>) -> Option<Identifier<'_>> {
        match pattern {
            Pattern::Variable(variable) => Some(variable.name()),
            _ => None,
        }
    }
    match pattern {
        Pattern::Record(record) => record
            .fields()
            .filter_map(|f| direct(f.pattern()))
            .collect(),
        Pattern::Object(object) => object
            .fields()
            .filter_map(|f| f.pattern().and_then(direct))
            .collect(),
        Pattern::List(list) => list
            .elements()
            .filter_map(|e| match e {
                ListPatternElement::Pattern(p) => direct(p),
                ListPatternElement::Rest(..) => None,
            })
            .collect(),
        Pattern::Map(map) => map.entries().filter_map(|e| direct(e.pattern())).collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use falcon_dart_parser::parse;

    use super::*;

    #[test]
    fn long_binary_chain_preserves_lexical_uses_and_writes() {
        let mut source = String::from("void f(int value) { value = value");
        source.push_str(&" + value".repeat(99_999));
        source.push_str("; value++; }");
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let program = parsed.tree.program(&source);
        let TopLevelDecl::Function(function) = program.declarations().first().unwrap() else {
            panic!()
        };
        let params = function.params().unwrap();
        let parameter = params.params().first().unwrap().name();
        assert_eq!(
            used_parameters(params, function.body().unwrap()),
            HashSet::from([parameter.span().start])
        );
        let expected_uses: HashSet<_> = source
            .match_indices("value")
            .skip(1)
            .map(|(offset, _)| offset)
            .collect();
        assert_eq!(expected_uses.len(), 100_002);
        assert_eq!(locally_bound_identifiers(program), expected_uses);
        let declarations = local_declarations(program);
        assert_eq!(declarations.len(), expected_uses.len());
        for offset in expected_uses {
            assert_eq!(declarations[&offset], parameter.syntax().id());
        }
        let writes = local_writes(program);
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0].kind, LocalWriteKind::Assign(AssignOp::Eq));
        assert_eq!(writes[0].span.start, source.find("value =").unwrap());
        assert_eq!(writes[1].kind, LocalWriteKind::IncDec);
        assert_eq!(writes[1].span.start, source.rfind("value++").unwrap());
        assert!(
            writes
                .iter()
                .all(|write| write.declaration == parameter.syntax().id())
        );
    }

    #[test]
    fn context_shares_exact_writes_and_mutated_declarations() {
        let source = "class C { int value; C(int p) : value = p { p++; } } void f() { var x = 0; () { x++; }; ((int x) { x++; })(0); for (x in [1]) {} }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(crate::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let program = parsed.program();
        let config = falcon_config::FalconConfig::default();
        let context =
            crate::AnalyzeContext::new(std::path::Path::new("test.dart"), &config, &parsed);
        let expected = local_writes(program);
        let cached = context.local_writes(program);
        assert_eq!(cached.len(), 4);
        assert!(std::ptr::eq(cached, context.local_writes(program)));
        for (actual, expected) in cached.iter().zip(expected) {
            assert_eq!(actual.declaration, expected.declaration);
            assert_eq!(actual.kind, expected.kind);
            assert_eq!(actual.span, expected.span);
        }
        let mutated = context.mutated_declarations(program);
        assert_eq!(mutated.len(), 3);
        assert!(std::ptr::eq(mutated, context.mutated_declarations(program)));
        assert!(
            cached
                .iter()
                .all(|write| mutated.contains(&write.declaration))
        );
    }

    #[test]
    fn legacy_case_expression_reads_parameters() {
        // Source-semantics compatibility: parameter references need not be
        // legal constants to exercise lexical usage in both pattern syntaxes.
        for version in ["2.19", "3.13"] {
            let source = format!(
                "// @dart = {version}\nvoid f(int value, Object input) {{ switch (input) {{ case const [value]: break; }} }}"
            );
            let parsed = parse(&source);
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            let program = parsed.tree.program(&source);
            let TopLevelDecl::Function(function) = program.declarations().first().unwrap() else {
                panic!()
            };
            let used = used_parameters(function.params().unwrap(), function.body().unwrap());
            assert!(
                used.contains(
                    &function
                        .params()
                        .unwrap()
                        .params()
                        .first()
                        .unwrap()
                        .name()
                        .span()
                        .start
                ),
                "source: {source}, used: {used:?}"
            );
        }
    }

    #[test]
    fn const_pattern_parameter_reads_respect_local_shadowing() {
        for (body, expected) in [
            ("switch (input) { case value: break; }", true),
            ("return switch (input) { value => 1, _ => 0 };", true),
            ("const value = 1; if (input case value) {}", false),
        ] {
            // Parameter constants need not be valid Dart to test lexical
            // resolution independently of constant evaluation.
            let source = format!("Object? f(int value, Object input) {{ {body} }}");
            let parsed = parse(&source);
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            let program = parsed.tree.program(&source);
            let TopLevelDecl::Function(function) = program.declarations().first().unwrap() else {
                panic!()
            };
            let params = function.params().unwrap();
            let used = used_parameters(params, function.body().unwrap());
            assert_eq!(
                used.contains(&params.params().first().unwrap().name().span().start),
                expected
            );
        }
    }

    #[test]
    fn const_pattern_names_resolve_to_the_nearest_local_at_each_use() {
        let source = "void f(Object input) { const Box = 1; switch (input) { case Box: break; } { const Box = 2; if (input case [Box]) {} } switch (input) { case Box: break; } }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let program = parsed.tree.program(source);
        let offsets: Vec<_> = source
            .match_indices("Box")
            .map(|(offset, _)| offset)
            .collect();
        let declarations = local_declarations(program);
        let bound = locally_bound_identifiers(program);
        for (usage, declaration) in [(1, 0), (3, 2), (4, 0)] {
            assert_eq!(
                declarations.get(&offsets[usage]),
                Some(
                    &parsed
                        .tree
                        .token_at(source, offsets[declaration])
                        .unwrap()
                        .parent()
                        .id()
                )
            );
            assert!(bound.contains(&offsets[usage]));
        }
        assert!(!bound.contains(&offsets[0]));
        assert!(!bound.contains(&offsets[2]));
        assert!(local_writes(program).is_empty());
    }

    #[test]
    fn nested_local_binding_restores_outer_parameter() {
        let source = "void f(int x) { { var x = 0; print(x); } print(x); }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let program = parsed.tree.program(source);
        let offsets: Vec<_> = source
            .match_indices('x')
            .map(|(offset, _)| offset)
            .collect();
        let declarations = local_declarations(program);
        for (usage, declaration) in [(2, 1), (3, 0)] {
            assert_eq!(
                declarations.get(&offsets[usage]),
                Some(
                    &parsed
                        .tree
                        .token_at(source, offsets[declaration])
                        .unwrap()
                        .parent()
                        .id()
                )
            );
        }
        let TopLevelDecl::Function(function) = program.declarations().first().unwrap() else {
            panic!()
        };
        assert_eq!(
            used_parameters(function.params().unwrap(), function.body().unwrap()),
            HashSet::from([offsets[0]])
        );
    }

    #[test]
    fn const_pattern_qualified_names_only_read_the_head() {
        let source = "void f(Object prefix, Object member, Object input) { if (input case prefix.member) {} }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let program = parsed.tree.program(source);
        let TopLevelDecl::Function(function) = program.declarations().first().unwrap() else {
            panic!()
        };
        let params = function.params().unwrap();
        let used = used_parameters(params, function.body().unwrap());
        assert!(used.contains(&params.params().first().unwrap().name().span().start));
        assert!(!used.contains(&params.params().nth(1).unwrap().name().span().start));
        let declarations = local_declarations(program);
        assert_eq!(
            declarations.get(&source.rfind("prefix").unwrap()),
            Some(&params.params().first().unwrap().name().syntax().id())
        );
        assert!(!declarations.contains_key(&source.rfind("member").unwrap()));
    }

    #[test]
    fn object_and_variable_pattern_types_are_not_value_reads() {
        for pattern in ["Box()", "Box value", "Box _"] {
            let source =
                format!("void f(Object Box, Object input) {{ if (input case {pattern}) {{}} }}");
            let parsed = parse(&source);
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            let program = parsed.tree.program(&source);
            let TopLevelDecl::Function(function) = program.declarations().first().unwrap() else {
                panic!()
            };
            let params = function.params().unwrap();
            let used = used_parameters(params, function.body().unwrap());
            assert!(
                !used.contains(&params.params().first().unwrap().name().span().start),
                "{pattern}"
            );
            assert!(!local_declarations(program).contains_key(&source.rfind("Box").unwrap()));
        }
    }

    #[test]
    fn nested_callback_parameter_shadows_target() {
        let source = "void f(int value) { [1].map((value) => value + 1); }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let program = parsed.tree.program(source);
        let TopLevelDecl::Function(function) = program.declarations().first().unwrap() else {
            panic!()
        };
        let used = used_parameters(function.params().unwrap(), function.body().unwrap());
        assert!(
            used.is_empty(),
            "used: {used:?}, target: {}",
            function
                .params()
                .unwrap()
                .params()
                .first()
                .unwrap()
                .name()
                .span()
                .start
        );
    }

    #[test]
    fn pattern_if_binding_shadows_target_in_guard_and_then_branch() {
        let source = "void f(int value, Object input) { if (input case int value when value > 0) print(value); }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let program = parsed.tree.program(source);
        let TopLevelDecl::Function(function) = program.declarations().first().unwrap() else {
            panic!()
        };
        let used = used_parameters(function.params().unwrap(), function.body().unwrap());
        assert!(
            !used.contains(
                &function
                    .params()
                    .unwrap()
                    .params()
                    .first()
                    .unwrap()
                    .name()
                    .span()
                    .start
            ),
            "source: {source}, used: {used:?}"
        );
    }

    #[test]
    fn pattern_if_binding_does_not_escape_then_branch() {
        for source in [
            "void f(int value, Object input) { if (input case int value when value > 0) print(value); else print(value); }",
            "void f(int value, Object input) { if (input case int value when value > 0) print(value); print(value); }",
        ] {
            let parsed = parse(source);
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            let program = parsed.tree.program(source);
            let TopLevelDecl::Function(function) = program.declarations().first().unwrap() else {
                panic!()
            };
            let used = used_parameters(function.params().unwrap(), function.body().unwrap());
            assert!(
                used.contains(
                    &function
                        .params()
                        .unwrap()
                        .params()
                        .first()
                        .unwrap()
                        .name()
                        .span()
                        .start
                ),
                "source: {source}, used: {used:?}"
            );
        }
    }

    #[test]
    fn nested_collection_bindings_shadow_parameters_without_escaping() {
        for literal in [
            "[if (condition) for (var value in [1]) value]",
            "<int, int>{if (condition) for (var value in [1]) value: value}",
            "[if (condition) if (input case int value when value > 0) value]",
            "<int, int>{if (condition) if (input case int value when value > 0) value: value}",
        ] {
            for suffix in ["", "print(value);"] {
                let source = format!(
                    "void f(int value, bool condition, Object input) {{ {literal}; {suffix} }}"
                );
                let parsed = parse(&source);
                assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
                let program = parsed.tree.program(&source);
                let TopLevelDecl::Function(function) = program.declarations().first().unwrap()
                else {
                    panic!()
                };
                let params = function.params().unwrap();
                let used = used_parameters(params, function.body().unwrap());
                assert_eq!(
                    used.contains(&params.params().first().unwrap().name().span().start),
                    !suffix.is_empty(),
                    "source: {source}, used: {used:?}"
                );
            }
        }
    }

    #[test]
    fn unbraced_control_flow_declarations_do_not_escape() {
        for source in [
            "void f(int value, bool condition) { if (condition) int value = 1; print(value); }",
            "void f(int value, bool condition) { if (condition) print('then'); else int value = 1; print(value); }",
            "void f(int value, bool condition) { while (condition) int value = 1; print(value); }",
            "void f(int value, bool condition) { do int value = 1; while (condition); print(value); }",
            "void f(int value, bool condition) { for (; condition;) int value = 1; print(value); }",
        ] {
            let parsed = parse(source);
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            let program = parsed.tree.program(source);
            let TopLevelDecl::Function(function) = program.declarations().first().unwrap() else {
                panic!()
            };
            let used = used_parameters(function.params().unwrap(), function.body().unwrap());
            assert!(
                used.contains(
                    &function
                        .params()
                        .unwrap()
                        .params()
                        .first()
                        .unwrap()
                        .name()
                        .span()
                        .start
                ),
                "source: {source}, used: {used:?}"
            );
        }
    }
}
