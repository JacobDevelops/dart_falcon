use std::collections::{HashMap, HashSet};

use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, bound_names, walk_expr, walk_pattern};

#[derive(Clone, Copy)]
enum Binding {
    Target(usize),
    Other,
}

pub(crate) fn used_parameters(params: &FormalParamList, body: &FunctionBody) -> HashSet<usize> {
    let mut collector = UsageCollector::new(params);
    collector.visit_body(body);
    collector.used
}

pub(crate) fn used_constructor_parameters(
    params: &FormalParamList,
    initializers: &[ConstructorInitializer],
    body: Option<&FunctionBody>,
) -> HashSet<usize> {
    let mut collector = UsageCollector::new(params);
    for initializer in initializers {
        match initializer {
            ConstructorInitializer::SuperCall { args, .. }
            | ConstructorInitializer::ThisCall { args, .. } => {
                for argument in &args.positional {
                    collector.visit_expr(argument);
                }
                for argument in &args.named {
                    collector.visit_expr(&argument.value);
                }
            }
            ConstructorInitializer::FieldInit { value, .. } => collector.visit_expr(value),
            ConstructorInitializer::Assert {
                condition, message, ..
            } => {
                collector.visit_expr(condition);
                if let Some(message) = message {
                    collector.visit_expr(message);
                }
            }
        }
    }
    if let Some(body) = body {
        collector.visit_body(body);
    }
    collector.used
}

struct UsageCollector {
    scopes: Vec<HashMap<String, Binding>>,
    used: HashSet<usize>,
}

impl UsageCollector {
    fn new(params: &FormalParamList) -> Self {
        let mut collector = Self {
            scopes: vec![HashMap::new()],
            used: HashSet::new(),
        };
        for param in all_params(params) {
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

    fn use_name(&mut self, name: &str) {
        if let Some(Binding::Target(span_start)) = self
            .scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
        {
            self.used.insert(span_start);
        }
    }

    fn bind_params(&mut self, params: &FormalParamList) {
        for param in all_params(params) {
            self.declare(&param.name.name, Binding::Other);
        }
    }

    fn bind_pattern(&mut self, pattern: &Pattern) {
        for name in bound_names(pattern) {
            self.declare(&name.name, Binding::Other);
        }
    }

    fn visit_body(&mut self, body: &FunctionBody) {
        match body {
            FunctionBody::Block(block) => {
                for statement in &block.stmts {
                    self.visit_stmt(statement);
                }
            }
            FunctionBody::Arrow(expression, _) => self.visit_expr(expression),
            FunctionBody::Native(_, _) => {}
        }
    }

    fn visit_nested_function(&mut self, params: &FormalParamList, body: &FunctionBody) {
        self.push_scope();
        self.bind_params(params);
        self.visit_body(body);
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
            self.declare(&declarator.name.name, Binding::Other);
        }
    }

    fn visit_for_init(&mut self, init: &ForInit) {
        match init {
            ForInit::VarDecl(declaration) => self.visit_local_declaration(declaration),
            ForInit::ForIn { name, iterable, .. } => {
                self.visit_expr(iterable);
                self.declare(&name.name, Binding::Other);
            }
            ForInit::PatternForIn { pattern, iterable } => {
                self.visit_expr(iterable);
                walk_pattern(self, pattern);
                self.bind_pattern(pattern);
            }
            ForInit::Exprs(expressions) => {
                for expression in expressions {
                    self.visit_expr(expression);
                }
            }
        }
    }

    fn visit_collection_element(&mut self, element: &CollectionElement) {
        match element {
            CollectionElement::Expr(expression)
            | CollectionElement::NullAware {
                expr: expression, ..
            }
            | CollectionElement::Spread {
                expr: expression, ..
            } => self.visit_expr(expression),
            CollectionElement::If {
                condition,
                then_elem,
                else_elem,
                ..
            } => {
                match condition {
                    IfCondition::Expr(expression) => self.visit_expr(expression),
                    IfCondition::Case(expression, pattern, guard) => {
                        self.visit_expr(expression);
                        self.push_scope();
                        walk_pattern(self, pattern);
                        self.bind_pattern(pattern);
                        if let Some(guard) = guard {
                            self.visit_expr(guard);
                        }
                        self.visit_collection_element(then_elem);
                        self.pop_scope();
                        if let Some(else_element) = else_elem {
                            self.visit_collection_element(else_element);
                        }
                        return;
                    }
                }
                self.visit_collection_element(then_elem);
                if let Some(else_element) = else_elem {
                    self.visit_collection_element(else_element);
                }
            }
            CollectionElement::For {
                pattern,
                iterable,
                element,
                ..
            } => {
                self.visit_expr(iterable);
                self.push_scope();
                if let Some(pattern) = pattern {
                    walk_pattern(self, pattern);
                    self.bind_pattern(pattern);
                }
                self.visit_collection_element(element);
                self.pop_scope();
            }
            CollectionElement::CFor {
                init,
                condition,
                updates,
                element,
                ..
            } => {
                self.push_scope();
                if let Some(init) = init {
                    self.visit_for_init(init);
                }
                if let Some(condition) = condition {
                    self.visit_expr(condition);
                }
                for update in updates {
                    self.visit_expr(update);
                }
                self.visit_collection_element(element);
                self.pop_scope();
            }
        }
    }

    fn visit_map_element(&mut self, element: &MapElement) {
        match element {
            MapElement::Entry(entry) => {
                self.visit_expr(&entry.key);
                self.visit_expr(&entry.value);
            }
            MapElement::Spread { expr, .. } => self.visit_expr(expr),
            MapElement::If {
                condition,
                then_entry,
                else_entry,
                ..
            } => {
                match condition {
                    IfCondition::Expr(expression) => self.visit_expr(expression),
                    IfCondition::Case(expression, pattern, guard) => {
                        self.visit_expr(expression);
                        self.push_scope();
                        walk_pattern(self, pattern);
                        self.bind_pattern(pattern);
                        if let Some(guard) = guard {
                            self.visit_expr(guard);
                        }
                        self.visit_map_element(then_entry);
                        self.pop_scope();
                        if let Some(else_entry) = else_entry {
                            self.visit_map_element(else_entry);
                        }
                        return;
                    }
                }
                self.visit_map_element(then_entry);
                if let Some(else_entry) = else_entry {
                    self.visit_map_element(else_entry);
                }
            }
            MapElement::For {
                pattern,
                iterable,
                entry,
                ..
            } => {
                self.visit_expr(iterable);
                self.push_scope();
                if let Some(pattern) = pattern {
                    walk_pattern(self, pattern);
                    self.bind_pattern(pattern);
                }
                self.visit_map_element(entry);
                self.pop_scope();
            }
            MapElement::CFor {
                init,
                condition,
                updates,
                entry,
                ..
            } => {
                self.push_scope();
                if let Some(init) = init {
                    self.visit_for_init(init);
                }
                if let Some(condition) = condition {
                    self.visit_expr(condition);
                }
                for update in updates {
                    self.visit_expr(update);
                }
                self.visit_map_element(entry);
                self.pop_scope();
            }
        }
    }
}

impl Visitor for UsageCollector {
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
                walk_pattern(self, &declaration.pattern);
                self.bind_pattern(&declaration.pattern);
            }
            Stmt::PatternAssign(assignment) => {
                walk_pattern(self, &assignment.pattern);
                // Pattern-assignment targets are writes, not reads, but the names
                // are declared elsewhere — counting them as uses keeps
                // unused-variable rules from flagging a variable this writes to.
                for name in bound_names(&assignment.pattern) {
                    self.use_name(&name.name);
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
                    walk_pattern(self, pattern);
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
                if let Some(init) = &statement.init {
                    self.visit_for_init(init);
                }
                if let Some(condition) = &statement.condition {
                    self.visit_expr(condition);
                }
                for update in &statement.update {
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
                            walk_pattern(self, pattern);
                            self.bind_pattern(pattern);
                            if let Some(guard) = guard.as_ref() {
                                self.visit_expr(guard);
                            }
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
                        self.declare(&name.name, Binding::Other);
                    }
                    if let Some(name) = &catch.stack_trace_var {
                        self.declare(&name.name, Binding::Other);
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
                self.declare(&function.name.name, Binding::Other);
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
            Stmt::Break(_) | Stmt::Continue(_) | Stmt::Error(_) => {}
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Ident(identifier) => self.use_name(&identifier.name),
            Expr::FuncExpr { params, body, .. } => self.visit_nested_function(params, body),
            Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                for element in elements {
                    self.visit_collection_element(element);
                }
            }
            Expr::Map {
                entries, elements, ..
            } => {
                for entry in entries {
                    self.visit_expr(&entry.key);
                    self.visit_expr(&entry.value);
                }
                for element in elements {
                    self.visit_map_element(element);
                }
            }
            Expr::Switch { subject, arms, .. } => {
                self.visit_expr(subject);
                for arm in arms {
                    self.push_scope();
                    walk_pattern(self, &arm.pattern);
                    self.bind_pattern(&arm.pattern);
                    if let Some(guard) = &arm.guard {
                        self.visit_expr(guard);
                    }
                    self.visit_expr(&arm.body);
                    self.pop_scope();
                }
            }
            _ => walk_expr(self, node),
        }
    }
}

fn all_params(params: &FormalParamList) -> impl Iterator<Item = &FormalParam> {
    params
        .positional
        .iter()
        .chain(&params.optional_positional)
        .chain(&params.named)
}

#[cfg(test)]
mod tests {
    use falcon_dart_parser::parse;

    use super::*;

    #[test]
    fn nested_callback_parameter_shadows_target() {
        let (program, errors) = parse("void f(int value) { [1].map((value) => value + 1); }");
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Function(function) = &program.declarations[0] else {
            panic!()
        };
        let used = used_parameters(&function.params, function.body.as_ref().unwrap());
        assert!(
            used.is_empty(),
            "used: {used:?}, target: {}",
            function.params.positional[0].name.span.start
        );
    }

    #[test]
    fn pattern_if_binding_shadows_target_in_guard_and_then_branch() {
        let source = "void f(int value, Object input) { if (input case int value when value > 0) print(value); }";
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Function(function) = &program.declarations[0] else {
            panic!()
        };
        let used = used_parameters(&function.params, function.body.as_ref().unwrap());
        assert!(
            !used.contains(&function.params.positional[0].name.span.start),
            "source: {source}, used: {used:?}"
        );
    }

    #[test]
    fn pattern_if_binding_does_not_escape_then_branch() {
        for source in [
            "void f(int value, Object input) { if (input case int value when value > 0) print(value); else print(value); }",
            "void f(int value, Object input) { if (input case int value when value > 0) print(value); print(value); }",
        ] {
            let (program, errors) = parse(source);
            assert!(errors.is_empty(), "{errors:?}");
            let TopLevelDecl::Function(function) = &program.declarations[0] else {
                panic!()
            };
            let used = used_parameters(&function.params, function.body.as_ref().unwrap());
            assert!(
                used.contains(&function.params.positional[0].name.span.start),
                "source: {source}, used: {used:?}"
            );
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
            let (program, errors) = parse(source);
            assert!(errors.is_empty(), "{errors:?}");
            let TopLevelDecl::Function(function) = &program.declarations[0] else {
                panic!()
            };
            let used = used_parameters(&function.params, function.body.as_ref().unwrap());
            assert!(
                used.contains(&function.params.positional[0].name.span.start),
                "source: {source}, used: {used:?}"
            );
        }
    }
}
