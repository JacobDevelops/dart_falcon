use std::collections::{HashMap, HashSet};

use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, bound_names, walk_class_member, walk_expr, walk_function_body, walk_pattern,
};

pub(crate) struct NameScopes(Vec<HashSet<String>>);

impl Default for NameScopes {
    fn default() -> Self {
        Self(vec![HashSet::new()])
    }
}

impl NameScopes {
    pub(crate) fn push(&mut self) {
        self.0.push(HashSet::new());
    }

    pub(crate) fn pop(&mut self) {
        self.0.pop();
    }

    pub(crate) fn declare(&mut self, name: &str) {
        self.0
            .last_mut()
            .expect("lexical scope")
            .insert(name.to_string());
    }

    pub(crate) fn bound(&self, name: &str) -> bool {
        self.0.iter().rev().any(|scope| scope.contains(name))
    }

    pub(crate) fn params(&mut self, params: &FormalParamList) {
        for param in &params.params {
            self.declare(&param.name.name);
        }
    }

    pub(crate) fn pattern(&mut self, pattern: &Pattern) {
        for name in bound_names(pattern) {
            self.declare(&name.name);
        }
    }
}

#[derive(Clone, Copy)]
enum Binding {
    Target(usize),
    Other,
}

pub(crate) fn used_parameters(params: &FormalParamList, body: &FunctionBody) -> HashSet<usize> {
    let mut collector = UsageCollector::new(params);
    walk_function_body(&mut collector, body);
    collector.used
}

pub(crate) fn used_constructor_parameters(
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

/// How a local binding is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LocalWriteKind {
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
pub(crate) struct LocalWrite {
    pub(crate) declaration: usize,
    pub(crate) kind: LocalWriteKind,
    pub(crate) span: Span,
}

/// Every write to a lexically scoped local (variable, parameter, loop or
/// pattern variable, catch variable) in `program`, in traversal order.
///
/// Writes inside closures resolve to the enclosing declaration unless a nearer
/// binding shadows the name, so the declarations named here are exactly the
/// analyzer's "potentially mutated in scope" set. Writes to names with no local
/// binding (fields, top-level variables) are not reported.
pub(crate) fn local_writes(program: &Program, source: &str) -> Vec<LocalWrite> {
    let mut collector = UsageCollector {
        scopes: vec![HashMap::new()],
        used: HashSet::new(),
        writes: Some(Vec::new()),
        source: Some(source),
        bound_uses: None,
    };
    collector.visit_program(program);
    collector.writes.unwrap_or_default()
}

/// Start offsets of every identifier expression in `program` that resolves to a
/// lexically scoped local — a variable, parameter, loop, pattern or catch
/// variable, or local function — instead of a member or top-level declaration.
pub(crate) fn locally_bound_identifiers(program: &Program) -> HashSet<usize> {
    let mut collector = UsageCollector {
        scopes: vec![HashMap::new()],
        used: HashSet::new(),
        writes: None,
        source: None,
        bound_uses: Some(HashSet::new()),
    };
    collector.visit_program(program);
    collector.bound_uses.unwrap_or_default()
}

/// Whether a for-in header declares its loop variable. `for (x in xs)` parses
/// as [`ForInit::ForIn`] with [`VarKeyword::Var`] and no type, exactly like
/// `for (var x in xs)`; only the source distinguishes the two.
pub(crate) fn for_in_declares_variable(
    keyword: VarKeyword,
    var_type: Option<&DartType>,
    name: &Identifier,
    source: &str,
) -> bool {
    if keyword != VarKeyword::Var || var_type.is_some() {
        return true;
    }
    let before = source.get(..name.span.start).unwrap_or_default().trim_end();
    before.strip_suffix("var").is_some_and(|rest| {
        !rest
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
    })
}

struct UsageCollector<'s> {
    scopes: Vec<HashMap<String, Binding>>,
    used: HashSet<usize>,
    /// `Some` when every local binding is tracked and writes are recorded.
    writes: Option<Vec<LocalWrite>>,
    source: Option<&'s str>,
    /// `Some` when identifier expressions that resolve to a local are recorded.
    bound_uses: Option<HashSet<usize>>,
}

impl UsageCollector<'_> {
    fn new(params: &FormalParamList) -> Self {
        let mut collector = Self {
            scopes: vec![HashMap::new()],
            used: HashSet::new(),
            writes: None,
            source: None,
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
        if let Expr::Ident(identifier) = target {
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
                keyword,
                var_type,
                name,
                iterable,
            } => {
                self.visit_expr(iterable);
                match self.source {
                    Some(source)
                        if !for_in_declares_variable(*keyword, var_type.as_ref(), name, source) =>
                    {
                        self.use_name(&name.name);
                        self.record_write(&name.name, LocalWriteKind::ForIn, name.span);
                    }
                    _ => self.bind(name),
                }
            }
            ForInit::PatternForIn {
                pattern, iterable, ..
            } => {
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
                header, element, ..
            } => {
                self.push_scope();
                self.visit_for_header(header);
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
            MapElement::For { header, entry, .. } => {
                self.push_scope();
                self.visit_for_header(header);
                self.visit_map_element(entry);
                self.pop_scope();
            }
        }
    }
}

impl Visitor for UsageCollector<'_> {
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
                walk_pattern(self, &declaration.pattern);
                self.bind_pattern(&declaration.pattern);
            }
            Stmt::PatternAssign(assignment) => {
                walk_pattern(self, &assignment.pattern);
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

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Ident(identifier) => {
                if self.lookup(&identifier.name).is_some()
                    && let Some(bound) = self.bound_uses.as_mut()
                {
                    bound.insert(identifier.span.start);
                }
                self.use_name(&identifier.name)
            }
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
                    walk_pattern(self, &arm.pattern);
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

/// Span of the first standalone `word` (identifier-bounded) in
/// `source[start..end]`, e.g. a `final`/`var` keyword inside a declaration.
pub(crate) fn find_word(source: &str, start: usize, end: usize, word: &str) -> Option<Span> {
    let is_ident = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$';
    let bytes = source.as_bytes();
    let end = end.min(source.len());
    let haystack = source.get(start..end)?;
    haystack.match_indices(word).find_map(|(offset, _)| {
        let from = start + offset;
        let to = from + word.len();
        let bounded = (from == 0 || !is_ident(bytes[from - 1]))
            && (to >= bytes.len() || !is_ident(bytes[to]));
        bounded.then(|| Span::new(from, to))
    })
}

/// The headers of collection-`for` elements of a literal `node` (looking
/// through nested `if`/`for` elements, which are not expressions).
pub(crate) fn collection_for_inits(node: &Expr) -> Vec<&ForInit> {
    let mut out = Vec::new();
    fn collection_element<'a>(element: &'a CollectionElement, out: &mut Vec<&'a ForInit>) {
        match element {
            CollectionElement::For {
                header, element, ..
            } => {
                out.extend(header.init.as_ref());
                collection_element(element, out);
            }
            CollectionElement::If {
                then_elem,
                else_elem,
                ..
            } => {
                collection_element(then_elem, out);
                if let Some(else_elem) = else_elem {
                    collection_element(else_elem, out);
                }
            }
            CollectionElement::Expr(_)
            | CollectionElement::NullAware { .. }
            | CollectionElement::Spread { .. } => {}
        }
    }
    fn map_element<'a>(element: &'a MapElement, out: &mut Vec<&'a ForInit>) {
        match element {
            MapElement::For { header, entry, .. } => {
                out.extend(header.init.as_ref());
                map_element(entry, out);
            }
            MapElement::If {
                then_entry,
                else_entry,
                ..
            } => {
                map_element(then_entry, out);
                if let Some(else_entry) = else_entry {
                    map_element(else_entry, out);
                }
            }
            MapElement::Entry(_) | MapElement::Spread { .. } => {}
        }
    }
    if let Expr::List { elements, .. } | Expr::Set { elements, .. } = node {
        for element in elements {
            collection_element(element, &mut out);
        }
    } else if let Expr::Map { elements, .. } = node {
        for element in elements {
            map_element(element, &mut out);
        }
    }
    out
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
        let (program, errors) = parse(source);
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
            let (program, errors) = parse(source);
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
            let used = used_parameters(function.params().unwrap(), function.body.as_ref().unwrap());
            assert!(
                used.contains(&function.params().unwrap().params[0].name.span.start),
                "source: {source}, used: {used:?}"
            );
        }
    }
}
