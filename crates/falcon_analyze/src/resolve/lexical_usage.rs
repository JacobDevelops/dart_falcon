use std::collections::{HashMap, HashSet};

use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, bound_names, walk_class_member, walk_collection_element, walk_expr,
    walk_function_body, walk_map_element, walk_pattern,
};

#[derive(Clone, Copy)]
enum Binding {
    Target(usize),
    Other,
}

pub fn used_parameters(params: &FormalParamList, body: &FunctionBody) -> HashSet<usize> {
    let mut collector = UsageCollector::new(params);
    walk_function_body(&mut collector, body);
    collector.used
}

pub fn used_constructor_parameters(
    params: &FormalParamList,
    initializers: &[ConstructorInitializer],
    body: Option<&FunctionBody>,
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
pub fn used_primary_constructor_parameters(
    primary: &PrimaryConstructor,
    members: &[ClassMember],
) -> HashSet<usize> {
    let mut collector = UsageCollector::new(&primary.params);
    for member in members {
        match member {
            ClassMember::PrimaryConstructorBody(body) => {
                collector.visit_initializers(&body.initializers);
                if let Some(body) = &body.body {
                    collector.push_scope();
                    walk_function_body(&mut collector, body);
                    collector.pop_scope();
                }
            }
            ClassMember::Field(field) if !field.is_static && !field.is_late => {
                for declarator in &field.declarators {
                    if let Some(initializer) = &declarator.initializer {
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
/// declaration (identified by the declaring name's span start).
#[derive(Clone, Copy, Debug)]
pub struct LocalWrite {
    pub declaration: usize,
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
pub fn local_writes(program: &Program) -> Vec<LocalWrite> {
    let mut collector = UsageCollector {
        scopes: vec![HashMap::new()],
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
pub fn locally_bound_identifiers(program: &Program) -> HashSet<usize> {
    let mut collector = UsageCollector {
        scopes: vec![HashMap::new()],
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

/// The declaration (declaring name's span start) of every identifier expression
/// and constant-pattern name head in `program` that resolves to a lexically
/// scoped local, keyed by the identifier's span start. Identifiers absent from
/// the map name members or top-level declarations.
pub fn local_declarations(program: &Program) -> HashMap<usize, usize> {
    let mut collector = UsageCollector {
        scopes: vec![HashMap::new()],
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
    scopes: Vec<HashMap<String, Binding>>,
    used: HashSet<usize>,
    /// `Some` when every local binding is tracked and writes are recorded.
    writes: Option<Vec<LocalWrite>>,
    /// `Some` when identifier expressions and constant-pattern name heads that
    /// resolve to a local are recorded, with their declaration in write mode.
    bound_uses: Option<HashMap<usize, Option<usize>>>,
}

impl UsageCollector {
    fn new(params: &FormalParamList) -> Self {
        let mut collector = Self {
            scopes: vec![HashMap::new()],
            used: HashSet::new(),
            writes: None,
            bound_uses: None,
        };
        for param in &params.params {
            collector.declare(&param.name.name, Binding::Target(param.name.span.start));
        }
        collector
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: &str, binding: Binding) {
        self.scopes
            .last_mut()
            .expect("lexical scope")
            .insert(name.to_string(), binding);
    }

    fn lookup(&self, name: &str) -> Option<Binding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn use_name(&mut self, name: &str) {
        if let Some(Binding::Target(span_start)) = self.lookup(name) {
            self.used.insert(span_start);
        }
    }

    fn use_identifier(&mut self, identifier: &Identifier) {
        if let Some(binding) = self.lookup(&identifier.name)
            && let Some(bound) = self.bound_uses.as_mut()
        {
            let declaration = match binding {
                Binding::Target(declaration) => Some(declaration),
                Binding::Other => None,
            };
            bound.insert(identifier.span.start, declaration);
        }
        self.use_name(&identifier.name);
    }

    /// Declares a local binding; tracked as a write target in write mode.
    fn bind(&mut self, name: &Identifier) {
        let binding = if self.writes.is_some() {
            Binding::Target(name.span.start)
        } else {
            Binding::Other
        };
        self.declare(&name.name, binding);
    }

    fn record_write(&mut self, name: &str, kind: LocalWriteKind, span: Span) {
        if self.writes.is_none() {
            return;
        }
        if let Some(Binding::Target(declaration)) = self.lookup(name)
            && let Some(writes) = self.writes.as_mut()
        {
            writes.push(LocalWrite {
                declaration,
                kind,
                span,
            });
        }
    }

    fn record_target_write(&mut self, target: &Expr, kind: LocalWriteKind, span: Span) {
        if let Expr::Ident(identifier) = target.unparenthesized() {
            self.record_write(&identifier.name, kind, span);
        }
    }

    fn bind_params(&mut self, params: &FormalParamList) {
        for param in &params.params {
            self.bind(&param.name);
        }
    }

    fn bind_pattern(&mut self, pattern: &Pattern) {
        for name in bound_names(pattern) {
            self.bind(name);
        }
    }

    fn visit_initializers(&mut self, initializers: &[ConstructorInitializer]) {
        for initializer in initializers {
            match initializer {
                ConstructorInitializer::SuperCall { args, .. }
                | ConstructorInitializer::ThisCall { args, .. } => {
                    for argument in &args.positional {
                        self.visit_expr(argument);
                    }
                    for argument in &args.named {
                        self.visit_expr(&argument.value);
                    }
                }
                ConstructorInitializer::FieldInit { value, .. } => self.visit_expr(value),
                ConstructorInitializer::Assert {
                    condition, message, ..
                } => {
                    self.visit_expr(condition);
                    if let Some(message) = message {
                        self.visit_expr(message);
                    }
                }
            }
        }
    }

    fn visit_declared_function(
        &mut self,
        params: Option<&FormalParamList>,
        body: Option<&FunctionBody>,
    ) {
        self.push_scope();
        if let Some(params) = params {
            self.bind_params(params);
        }
        if let Some(body) = body {
            walk_function_body(self, body);
        }
        self.pop_scope();
    }

    fn visit_nested_function(&mut self, params: &FormalParamList, body: &FunctionBody) {
        self.push_scope();
        self.bind_params(params);
        walk_function_body(self, body);
        self.pop_scope();
    }

    fn visit_scoped_stmt(&mut self, statement: &Stmt) {
        self.push_scope();
        self.visit_stmt(statement);
        self.pop_scope();
    }

    fn visit_local_declaration(&mut self, declaration: &LocalVarDecl) {
        for declarator in &declaration.declarators {
            if let Some(initializer) = &declarator.initializer {
                self.visit_expr(initializer);
            }
            self.bind(&declarator.name);
        }
    }

    fn visit_for_init(&mut self, init: &ForInit) {
        match init {
            ForInit::VarDecl(declaration) => self.visit_local_declaration(declaration),
            ForInit::ForIn {
                declares_variable,
                name,
                iterable,
                ..
            } => {
                self.visit_expr(iterable);
                if *declares_variable {
                    self.bind(name);
                } else {
                    self.use_name(&name.name);
                    self.record_write(&name.name, LocalWriteKind::ForIn, name.span);
                }
            }
            ForInit::PatternForIn {
                pattern, iterable, ..
            } => {
                self.visit_expr(iterable);
                self.visit_pattern(pattern);
                self.bind_pattern(pattern);
            }
            ForInit::Exprs(expressions) => {
                for expression in expressions {
                    self.visit_expr(expression);
                }
            }
        }
    }

    fn visit_for_header(&mut self, header: &ForHeader) {
        if let Some(init) = &header.init {
            self.visit_for_init(init);
        }
        if let Some(condition) = &header.condition {
            self.visit_expr(condition);
        }
        for update in &header.updates {
            self.visit_expr(update);
        }
    }
}

impl Visitor for UsageCollector {
    fn visit_collection_element(&mut self, element: &CollectionElement) {
        match element {
            CollectionElement::If {
                condition: IfCondition::Case(expression, pattern, guard),
                then_elem,
                else_elem,
                ..
            } => {
                self.visit_expr(expression);
                self.push_scope();
                self.visit_pattern(pattern);
                self.bind_pattern(pattern);
                if let Some(guard) = guard {
                    self.visit_expr(guard);
                }
                self.visit_collection_element(then_elem);
                self.pop_scope();
                if let Some(else_element) = else_elem {
                    self.visit_collection_element(else_element);
                }
            }
            CollectionElement::For {
                header, element, ..
            } => {
                self.push_scope();
                self.visit_for_header(header);
                self.visit_collection_element(element);
                self.pop_scope();
            }
            _ => walk_collection_element(self, element),
        }
    }

    fn visit_map_element(&mut self, element: &MapElement) {
        match element {
            MapElement::If {
                condition: IfCondition::Case(expression, pattern, guard),
                then_entry,
                else_entry,
                ..
            } => {
                self.visit_expr(expression);
                self.push_scope();
                self.visit_pattern(pattern);
                self.bind_pattern(pattern);
                if let Some(guard) = guard {
                    self.visit_expr(guard);
                }
                self.visit_map_element(then_entry);
                self.pop_scope();
                if let Some(else_entry) = else_entry {
                    self.visit_map_element(else_entry);
                }
            }
            MapElement::For { header, entry, .. } => {
                self.push_scope();
                self.visit_for_header(header);
                self.visit_map_element(entry);
                self.pop_scope();
            }
            _ => walk_map_element(self, element),
        }
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.visit_declared_function(node.params(), node.body.as_ref());
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.visit_declared_function(Some(&node.params), node.body.as_ref());
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        self.visit_declared_function(None, node.body.as_ref());
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        self.push_scope();
        self.bind(&node.param);
        if let Some(body) = &node.body {
            walk_function_body(self, body);
        }
        self.pop_scope();
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        match node {
            ClassMember::Operator(operator) => {
                self.visit_declared_function(Some(&operator.params), operator.body.as_ref())
            }
            _ => walk_class_member(self, node),
        }
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        // Initializing formals are in scope only in the initializer list; in the
        // body `x` names the field again.
        self.push_scope();
        for param in node
            .params
            .params
            .iter()
            .filter(|p| p.initializing.is_none())
        {
            self.bind(&param.name);
        }
        self.push_scope();
        for param in node
            .params
            .params
            .iter()
            .filter(|p| p.initializing.is_some())
        {
            self.bind(&param.name);
        }
        self.visit_initializers(&node.initializers);
        self.pop_scope();
        if let Some(body) = &node.body {
            walk_function_body(self, body);
        }
        self.pop_scope();
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::Block(block) => {
                self.push_scope();
                for statement in &block.stmts {
                    self.visit_stmt(statement);
                }
                self.pop_scope();
            }
            Stmt::LocalVar(declaration) => self.visit_local_declaration(declaration),
            Stmt::PatternDecl(declaration) => {
                self.visit_expr(&declaration.init);
                self.visit_pattern(&declaration.pattern);
                self.bind_pattern(&declaration.pattern);
            }
            Stmt::PatternAssign(assignment) => {
                self.visit_pattern(&assignment.pattern);
                // Pattern-assignment targets are writes, not reads, but the names
                // are declared elsewhere — counting them as uses keeps
                // unused-variable rules from flagging a variable this writes to.
                let direct = direct_pattern_variables(&assignment.pattern);
                let span = *assignment.pattern.span();
                for name in bound_names(&assignment.pattern) {
                    self.use_name(&name.name);
                    let direct = direct.iter().any(|d| d.span == name.span);
                    self.record_write(&name.name, LocalWriteKind::Pattern { direct }, span);
                }
                self.visit_expr(&assignment.value);
            }
            Stmt::If(statement) => match &statement.condition {
                IfCondition::Expr(condition) => {
                    self.visit_expr(condition);
                    self.visit_scoped_stmt(&statement.then_branch);
                    if let Some(else_branch) = &statement.else_branch {
                        self.visit_scoped_stmt(else_branch);
                    }
                }
                IfCondition::Case(value, pattern, guard) => {
                    self.visit_expr(value);
                    self.push_scope();
                    self.visit_pattern(pattern);
                    self.bind_pattern(pattern);
                    if let Some(guard) = guard {
                        self.visit_expr(guard);
                    }
                    self.visit_scoped_stmt(&statement.then_branch);
                    self.pop_scope();
                    if let Some(else_branch) = &statement.else_branch {
                        self.visit_scoped_stmt(else_branch);
                    }
                }
            },
            Stmt::For(statement) => {
                self.push_scope();
                if let Some(init) = &statement.header.init {
                    self.visit_for_init(init);
                }
                if let Some(condition) = &statement.header.condition {
                    self.visit_expr(condition);
                }
                for update in &statement.header.updates {
                    self.visit_expr(update);
                }
                self.visit_scoped_stmt(&statement.body);
                self.pop_scope();
            }
            Stmt::Switch(statement) => {
                self.visit_expr(&statement.subject);
                for case in &statement.cases {
                    self.push_scope();
                    for kind in &case.cases {
                        if let SwitchCaseKind::Pattern(pattern, guard) = kind {
                            self.visit_pattern(pattern);
                            self.bind_pattern(pattern);
                            if let Some(guard) = guard.as_ref() {
                                self.visit_expr(guard);
                            }
                        } else if let SwitchCaseKind::Expr(expression) = kind {
                            self.visit_expr(expression);
                        }
                    }
                    for statement in &case.body {
                        self.visit_stmt(statement);
                    }
                    self.pop_scope();
                }
            }
            Stmt::TryCatch(statement) => {
                self.push_scope();
                for statement in &statement.body.stmts {
                    self.visit_stmt(statement);
                }
                self.pop_scope();
                for catch in &statement.catches {
                    self.push_scope();
                    if let Some(name) = &catch.exception_var {
                        self.bind(name);
                    }
                    if let Some(name) = &catch.stack_trace_var {
                        self.bind(name);
                    }
                    for statement in &catch.body.stmts {
                        self.visit_stmt(statement);
                    }
                    self.pop_scope();
                }
                if let Some(finally) = &statement.finally {
                    self.push_scope();
                    for statement in &finally.stmts {
                        self.visit_stmt(statement);
                    }
                    self.pop_scope();
                }
            }
            Stmt::LocalFunc(function) => {
                self.bind(&function.name);
                self.visit_nested_function(&function.params, &function.body);
            }
            Stmt::While(statement) => {
                self.visit_expr(&statement.condition);
                self.visit_scoped_stmt(&statement.body);
            }
            Stmt::DoWhile(statement) => {
                self.visit_scoped_stmt(&statement.body);
                self.visit_expr(&statement.condition);
            }
            Stmt::Return(statement) => {
                if let Some(value) = &statement.value {
                    self.visit_expr(value);
                }
            }
            Stmt::Throw(statement) => self.visit_expr(&statement.value),
            Stmt::Labeled(statement) => self.visit_stmt(&statement.stmt),
            Stmt::Assert(statement) => {
                self.visit_expr(&statement.condition);
                if let Some(message) = &statement.message {
                    self.visit_expr(message);
                }
            }
            Stmt::Yield(statement) => self.visit_expr(&statement.value),
            Stmt::Expr(statement) => self.visit_expr(&statement.expr),
            Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Rethrow(_)
            | Stmt::Empty(_)
            | Stmt::Error(_) => {}
        }
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        if let Pattern::Const(ConstPattern {
            value: ConstPatternValue::Name(name),
            ..
        }) = node
            && let Some(head) = name.first()
        {
            self.use_identifier(head);
        }
        walk_pattern(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Ident(identifier) => self.use_identifier(identifier),
            Expr::FuncExpr { params, body, .. } => self.visit_nested_function(params, body),
            Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                for element in elements {
                    self.visit_collection_element(element);
                }
            }
            Expr::Map { elements, .. } => {
                for element in elements {
                    self.visit_map_element(element);
                }
            }
            Expr::Switch { subject, arms, .. } => {
                self.visit_expr(subject);
                for arm in arms {
                    self.push_scope();
                    self.visit_pattern(&arm.pattern);
                    self.bind_pattern(&arm.pattern);
                    if let Some(guard) = &arm.guard {
                        self.visit_expr(guard);
                    }
                    self.visit_expr(&arm.body);
                    self.pop_scope();
                }
            }
            Expr::Assign {
                target, op, span, ..
            } => {
                self.record_target_write(target, LocalWriteKind::Assign(*op), *span);
                walk_expr(self, node);
            }
            Expr::PostfixIncDec { operand, span, .. }
            | Expr::Unary {
                op: UnaryOp::PlusPlus | UnaryOp::MinusMinus,
                operand,
                span,
            } => {
                self.record_target_write(operand, LocalWriteKind::IncDec, *span);
                walk_expr(self, node);
            }
            _ => walk_expr(self, node),
        }
    }
}

/// Variables that are immediate fields/elements of a top-level record, object,
/// list, or map pattern (the analyzer's direct `AssignedVariablePattern`s).
fn direct_pattern_variables(pattern: &Pattern) -> Vec<&Identifier> {
    fn direct(pattern: &Pattern) -> Option<&Identifier> {
        match pattern {
            Pattern::Variable { name, .. } => Some(name),
            _ => None,
        }
    }
    match pattern {
        Pattern::Record(record) => record
            .fields
            .iter()
            .filter_map(|f| direct(&f.pattern))
            .collect(),
        Pattern::Object(object) => object
            .fields
            .iter()
            .filter_map(|f| f.pattern.as_ref().and_then(direct))
            .collect(),
        Pattern::List(list) => list
            .elements
            .iter()
            .filter_map(|e| match e {
                ListPatternElement::Pattern(p) => direct(p),
                ListPatternElement::Rest(..) => None,
            })
            .collect(),
        Pattern::Map(map) => map
            .entries
            .iter()
            .filter_map(|e| direct(&e.pattern))
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use falcon_dart_parser::parse;

    use super::*;

    #[test]
    fn context_shares_exact_writes_and_mutated_declarations() {
        let source = "class C { int value; C(int p) : value = p { p++; } } void f() { var x = 0; () { x++; }; ((int x) { x++; })(0); for (x in [1]) {} }";
        let falcon_dart_parser::ParseResult {
            program,
            syntax,
            errors,
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let config = falcon_config::FalconConfig::default();
        let context =
            crate::AnalyzeContext::new(std::path::Path::new("test.dart"), source, &config, &syntax);
        let expected = local_writes(&program);
        let cached = context.local_writes(&program);
        assert_eq!(cached.len(), 4);
        assert!(std::ptr::eq(cached, context.local_writes(&program)));
        for (actual, expected) in cached.iter().zip(expected) {
            assert_eq!(actual.declaration, expected.declaration);
            assert_eq!(actual.kind, expected.kind);
            assert_eq!(actual.span, expected.span);
        }
        let mutated = context.mutated_declarations(&program);
        assert_eq!(mutated.len(), 3);
        assert!(std::ptr::eq(
            mutated,
            context.mutated_declarations(&program)
        ));
        assert!(
            cached
                .iter()
                .all(|write| mutated.contains(&write.declaration))
        );
    }

    #[test]
    fn legacy_case_expression_reads_parameters() {
        // Source-semantics compatibility: parameter references need not be
        // legal constants to exercise lexical usage in both case AST forms.
        for version in ["2.19", "3.13"] {
            let source = format!(
                "// @dart = {version}\nvoid f(int value, Object input) {{ switch (input) {{ case const [value]: break; }} }}"
            );
            let falcon_dart_parser::ParseResult {
                program, errors, ..
            } = parse(&source);
            assert!(errors.is_empty(), "{errors:?}");
            let TopLevelDecl::Function(function) = &program.declarations[0] else {
                panic!()
            };
            let used = used_parameters(function.params().unwrap(), function.body.as_ref().unwrap());
            assert!(
                used.contains(&function.params().unwrap().params[0].name.span.start),
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
            let falcon_dart_parser::ParseResult {
                program, errors, ..
            } = parse(&source);
            assert!(errors.is_empty(), "{errors:?}");
            let TopLevelDecl::Function(function) = &program.declarations[0] else {
                panic!()
            };
            let params = function.params().unwrap();
            let used = used_parameters(params, function.body.as_ref().unwrap());
            assert_eq!(used.contains(&params.params[0].name.span.start), expected);
        }
    }

    #[test]
    fn const_pattern_names_resolve_to_the_nearest_local_at_each_use() {
        let source = "void f(Object input) { const Box = 1; switch (input) { case Box: break; } { const Box = 2; if (input case [Box]) {} } switch (input) { case Box: break; } }";
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let offsets: Vec<_> = source
            .match_indices("Box")
            .map(|(offset, _)| offset)
            .collect();
        let declarations = local_declarations(&program);
        let bound = locally_bound_identifiers(&program);
        for (usage, declaration) in [(1, 0), (3, 2), (4, 0)] {
            assert_eq!(
                declarations.get(&offsets[usage]),
                Some(&offsets[declaration])
            );
            assert!(bound.contains(&offsets[usage]));
        }
        assert!(!bound.contains(&offsets[0]));
        assert!(!bound.contains(&offsets[2]));
        assert!(local_writes(&program).is_empty());
    }

    #[test]
    fn const_pattern_qualified_names_only_read_the_head() {
        let source = "void f(Object prefix, Object member, Object input) { if (input case prefix.member) {} }";
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Function(function) = &program.declarations[0] else {
            panic!()
        };
        let params = function.params().unwrap();
        let used = used_parameters(params, function.body.as_ref().unwrap());
        assert!(used.contains(&params.params[0].name.span.start));
        assert!(!used.contains(&params.params[1].name.span.start));
        let declarations = local_declarations(&program);
        assert_eq!(
            declarations.get(&source.rfind("prefix").unwrap()),
            Some(&params.params[0].name.span.start)
        );
        assert!(!declarations.contains_key(&source.rfind("member").unwrap()));
    }

    #[test]
    fn object_and_variable_pattern_types_are_not_value_reads() {
        for pattern in ["Box()", "Box value", "Box _"] {
            let source =
                format!("void f(Object Box, Object input) {{ if (input case {pattern}) {{}} }}");
            let falcon_dart_parser::ParseResult {
                program, errors, ..
            } = parse(&source);
            assert!(errors.is_empty(), "{errors:?}");
            let TopLevelDecl::Function(function) = &program.declarations[0] else {
                panic!()
            };
            let params = function.params().unwrap();
            let used = used_parameters(params, function.body.as_ref().unwrap());
            assert!(
                !used.contains(&params.params[0].name.span.start),
                "{pattern}"
            );
            assert!(!local_declarations(&program).contains_key(&source.rfind("Box").unwrap()));
        }
    }

    #[test]
    fn nested_callback_parameter_shadows_target() {
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse("void f(int value) { [1].map((value) => value + 1); }");
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Function(function) = &program.declarations[0] else {
            panic!()
        };
        let used = used_parameters(function.params().unwrap(), function.body.as_ref().unwrap());
        assert!(
            used.is_empty(),
            "used: {used:?}, target: {}",
            function.params().unwrap().params[0].name.span.start
        );
    }

    #[test]
    fn pattern_if_binding_shadows_target_in_guard_and_then_branch() {
        let source = "void f(int value, Object input) { if (input case int value when value > 0) print(value); }";
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Function(function) = &program.declarations[0] else {
            panic!()
        };
        let used = used_parameters(function.params().unwrap(), function.body.as_ref().unwrap());
        assert!(
            !used.contains(&function.params().unwrap().params[0].name.span.start),
            "source: {source}, used: {used:?}"
        );
    }

    #[test]
    fn pattern_if_binding_does_not_escape_then_branch() {
        for source in [
            "void f(int value, Object input) { if (input case int value when value > 0) print(value); else print(value); }",
            "void f(int value, Object input) { if (input case int value when value > 0) print(value); print(value); }",
        ] {
            let falcon_dart_parser::ParseResult {
                program, errors, ..
            } = parse(source);
            assert!(errors.is_empty(), "{errors:?}");
            let TopLevelDecl::Function(function) = &program.declarations[0] else {
                panic!()
            };
            let used = used_parameters(function.params().unwrap(), function.body.as_ref().unwrap());
            assert!(
                used.contains(&function.params().unwrap().params[0].name.span.start),
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
                let falcon_dart_parser::ParseResult {
                    program, errors, ..
                } = parse(&source);
                assert!(errors.is_empty(), "{errors:?}");
                let TopLevelDecl::Function(function) = &program.declarations[0] else {
                    panic!()
                };
                let params = function.params().unwrap();
                let used = used_parameters(params, function.body.as_ref().unwrap());
                assert_eq!(
                    used.contains(&params.params[0].name.span.start),
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
            let falcon_dart_parser::ParseResult {
                program, errors, ..
            } = parse(source);
            assert!(errors.is_empty(), "{errors:?}");
            let TopLevelDecl::Function(function) = &program.declarations[0] else {
                panic!()
            };
            let used = used_parameters(function.params().unwrap(), function.body.as_ref().unwrap());
            assert!(
                used.contains(&function.params().unwrap().params[0].name.span.start),
                "source: {source}, used: {used:?}"
            );
        }
    }
}
