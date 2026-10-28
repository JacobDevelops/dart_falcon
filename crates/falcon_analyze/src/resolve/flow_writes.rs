//! Variables each loop and `try` region writes, collected once per program
//! with lexical resolution; flow analysis forgets their promotions where
//! control may re-enter the region (a loop's back edge, a `catch`).
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};

use falcon_syntax::cst::visit::{
    Visitor, walk_class_member, walk_element, walk_expr, walk_expr_chain, walk_function_decl,
    walk_pattern, walk_stmt,
};
use falcon_syntax::cst::*;

use super::lexical_scope::LexicalScopes;

#[derive(Debug, Default)]
pub struct FlowWrites {
    /// Direct writes of bindings enclosing each loop, switch, or try/catch region.
    pub region_writes: HashMap<usize, HashSet<String>>,
    /// Enclosing bindings written by closures declared inside each region;
    /// captures by earlier closures are already marked in the environment.
    pub captured_region_writes: HashMap<usize, HashSet<String>>,
    /// Enclosing bindings written anywhere in each nested callable's executable.
    pub anywhere_writes: HashMap<usize, HashSet<String>>,
    /// Enclosing bindings each nested callable sees that some closure writes,
    /// wherever in the executable that closure is declared.
    pub anywhere_captured: HashMap<usize, HashSet<String>>,
    /// Enclosing bindings each nested callable itself writes, directly or in
    /// deeper closures: the captures its declaration actually performs.
    pub callable_captured_writes: HashMap<usize, HashSet<String>>,
    /// Parsed revision that produced cached facts, independent of declaration retention.
    pub(crate) source_revision: Weak<crate::ParsedFile>,
    scopes: LexicalScopes<HashMap<String, usize>>,
    callable_scopes: Vec<usize>,
    callable_ids: Vec<Option<usize>>,
    callable_captures: HashMap<usize, HashSet<usize>>,
    regions: Vec<(usize, usize, usize)>,
    callable_bindings: HashMap<usize, HashMap<String, usize>>,
    captured_bindings: HashSet<usize>,
    written_bindings: HashSet<usize>,
    assigning_pattern: bool,
}

impl FlowWrites {
    pub(crate) fn from_parsed(parsed: &Arc<crate::ParsedFile>) -> Self {
        Self {
            source_revision: Arc::downgrade(parsed),
            ..Self::program(parsed.program())
        }
    }

    pub fn program(program: Program<'_>) -> Self {
        let mut visitor = Self {
            scopes: LexicalScopes::new(),
            ..Self::default()
        };
        visitor.visit_program(program);
        visitor.anywhere_writes =
            callable_names(&visitor.callable_bindings, &visitor.written_bindings);
        visitor.anywhere_captured =
            callable_names(&visitor.callable_bindings, &visitor.captured_bindings);
        visitor.callable_captured_writes = visitor
            .callable_bindings
            .iter()
            .filter_map(|(callable, bindings)| {
                let names = names_of(bindings, visitor.callable_captures.get(callable)?);
                (!names.is_empty()).then_some((*callable, names))
            })
            .collect();
        // Cached flow facts outlive traversal; binding snapshots and capture
        // bookkeeping must not remain attached to each file's result.
        Self {
            region_writes: visitor.region_writes,
            captured_region_writes: visitor.captured_region_writes,
            anywhere_writes: visitor.anywhere_writes,
            anywhere_captured: visitor.anywhere_captured,
            callable_captured_writes: visitor.callable_captured_writes,
            ..Self::default()
        }
    }

    fn bindings(&self) -> HashMap<String, usize> {
        self.scopes
            .iter()
            .flat_map(|scope| scope.iter().map(|(name, id)| (name.clone(), *id)))
            .collect()
    }

    fn bind(&mut self, identifier: Identifier<'_>) {
        self.scopes
            .declare(identifier.name().to_owned(), identifier.span().start);
    }

    fn record(&mut self, identifier: Identifier<'_>) {
        let name = identifier.name();
        let binding = self.scopes.lookup(name).map(|(scope, id)| (scope, *id));
        if let Some((scope, id)) = binding {
            self.written_bindings.insert(id);
            if self
                .callable_scopes
                .last()
                .is_some_and(|entry| scope < *entry)
            {
                self.captured_bindings.insert(id);
            }
            for (&entry, &callable) in self.callable_scopes.iter().zip(&self.callable_ids) {
                if let Some(callable) = callable.filter(|_| scope < entry) {
                    self.callable_captures
                        .entry(callable)
                        .or_default()
                        .insert(id);
                }
            }
            // Resolve captures while lexical scopes are still available instead
            // of copying every visible binding when a region begins.
            for &(region, entry, depth) in &self.regions {
                if scope < entry
                    && self
                        .callable_scopes
                        .get(depth)
                        .is_some_and(|callable| scope < *callable)
                {
                    let names = self.captured_region_writes.entry(region).or_default();
                    if !names.contains(name) {
                        names.insert(name.to_owned());
                    }
                }
            }
        }
        for &(region, scope, depth) in &self.regions {
            if depth == self.callable_scopes.len()
                && binding.is_none_or(|(binding, _)| binding < scope)
            {
                self.region_writes
                    .entry(region)
                    .or_default()
                    .insert(name.to_owned());
            }
        }
    }

    fn pattern_variable(&mut self, identifier: Identifier<'_>) {
        if self.assigning_pattern {
            self.record(identifier);
        } else {
            self.bind(identifier);
        }
    }

    fn scoped(&mut self, visit: impl FnOnce(&mut Self)) {
        self.scopes.push_scope();
        visit(self);
        self.scopes.pop_scope();
    }

    /// Visits a callable body; `id` keys nested callables' facts.
    fn callable(&mut self, id: Option<usize>, visit: impl FnOnce(&mut Self)) {
        self.callable_scopes.push(self.scopes.len());
        self.callable_ids.push(id);
        self.scoped(visit);
        self.callable_ids.pop();
        self.callable_scopes.pop();
    }

    fn region(&mut self, id: usize, visit: impl FnOnce(&mut Self)) {
        self.regions
            .push((id, self.scopes.len(), self.callable_scopes.len()));
        visit(self);
        self.regions.pop();
    }
    fn conditional(&mut self, condition: IfCondition<'_>, then: impl FnOnce(&mut Self)) {
        match condition {
            IfCondition::Expr(value) => {
                self.visit_expr(value);
                self.scoped(then);
            }
            IfCondition::Case(case) => {
                self.visit_expr(case.expr());
                self.scoped(|visitor| {
                    visitor.visit_pattern(case.pattern());
                    if let Some(guard) = case.guard() {
                        visitor.visit_expr(guard);
                    }
                    then(visitor);
                });
            }
        }
    }

    fn loop_header(&mut self, header: ForHeader<'_>) {
        if let Some(ForInit::ForIn(init)) = header.init()
            && !init.declares_variable()
        {
            self.record(init.name());
        }
        if let Some(condition) = header.condition() {
            self.visit_expr(condition);
        }
        for update in header.updates() {
            self.visit_expr(update);
        }
    }

    fn loop_init(&mut self, header: ForHeader<'_>) {
        if let Some(init) = header.init() {
            match init {
                ForInit::VarDecl(declaration) => self.visit_local_var(declaration),
                ForInit::ForIn(init) => {
                    self.visit_expr(init.iterable());
                    if init.declares_variable() {
                        self.bind(init.name());
                    }
                }
                ForInit::PatternForIn(init) => {
                    self.visit_expr(init.iterable());
                    self.visit_pattern(init.pattern());
                }
                ForInit::Exprs(expressions) => {
                    for expression in expressions.exprs() {
                        self.visit_expr(expression);
                    }
                }
            }
        }
    }
}

/// Names in `bindings` whose binding is in `selected`.
fn names_of(bindings: &HashMap<String, usize>, selected: &HashSet<usize>) -> HashSet<String> {
    bindings
        .iter()
        .filter(|(_, binding)| selected.contains(binding))
        .map(|(name, _)| name.clone())
        .collect()
}

fn callable_names(
    callables: &HashMap<usize, HashMap<String, usize>>,
    selected: &HashSet<usize>,
) -> HashMap<usize, HashSet<String>> {
    callables
        .iter()
        .filter_map(|(callable, bindings)| {
            let names = names_of(bindings, selected);
            (!names.is_empty()).then_some((*callable, names))
        })
        .collect()
}

impl<'a> Visitor<'a> for FlowWrites {
    fn visit_local_var(&mut self, declaration: LocalVarDecl<'a>) {
        for declarator in declaration.declarators() {
            if let Some(initializer) = declarator.initializer() {
                self.visit_expr(initializer);
            }
            self.bind(declarator.name());
        }
    }

    fn visit_formal_param(&mut self, parameter: FormalParam<'a>) {
        self.bind(parameter.name());
        if let Some(default) = parameter.default_value() {
            self.visit_expr(default);
        }
    }

    fn visit_class_member(&mut self, member: ClassMember<'a>) {
        match member {
            ClassMember::Field(_) | ClassMember::Error(_) => walk_class_member(self, member),
            _ => self.callable(None, |visitor| walk_class_member(visitor, member)),
        }
    }

    fn visit_function_decl(&mut self, declaration: FunctionDecl<'a>) {
        self.bind(declaration.name());
        self.callable(None, |visitor| walk_function_decl(visitor, declaration));
    }

    fn visit_stmt(&mut self, statement: Stmt<'a>) {
        match statement {
            Stmt::Block(_) => self.scoped(|visitor| walk_stmt(visitor, statement)),
            Stmt::If(statement) => {
                self.conditional(statement.condition(), |visitor| {
                    visitor.visit_stmt(statement.then_branch());
                });
                if let Some(branch) = statement.else_branch() {
                    self.scoped(|visitor| visitor.visit_stmt(branch));
                }
            }
            Stmt::For(statement) => self.scoped(|visitor| {
                visitor.loop_init(statement.header());
                visitor.region(statement.span().start, |visitor| {
                    visitor.loop_header(statement.header());
                    visitor.scoped(|visitor| visitor.visit_stmt(statement.body()));
                });
            }),
            Stmt::While(statement) => self.region(statement.span().start, |visitor| {
                visitor.visit_expr(statement.condition());
                visitor.scoped(|visitor| visitor.visit_stmt(statement.body()));
            }),
            Stmt::DoWhile(statement) => self.region(statement.span().start, |visitor| {
                visitor.scoped(|visitor| visitor.visit_stmt(statement.body()));
                visitor.visit_expr(statement.condition());
            }),
            Stmt::TryCatch(statement) => {
                self.region(statement.span().start, |visitor| {
                    visitor.scoped(|visitor| {
                        for statement in statement.body().stmts() {
                            visitor.visit_stmt(statement);
                        }
                    })
                });
                for catch in statement.catches() {
                    self.scoped(|visitor| {
                        visitor.region(catch.body().span().start, |visitor| {
                            if let Some(name) = catch.exception_var() {
                                visitor.bind(name);
                            }
                            if let Some(name) = catch.stack_trace_var() {
                                visitor.bind(name);
                            }
                            for statement in catch.body().stmts() {
                                visitor.visit_stmt(statement);
                            }
                        });
                    });
                }
                if let Some(finally) = statement.finally() {
                    self.scoped(|visitor| {
                        for statement in finally.stmts() {
                            visitor.visit_stmt(statement);
                        }
                    });
                }
            }
            Stmt::Switch(statement) => self.region(statement.span().start, |visitor| {
                visitor.visit_expr(statement.subject());
                for case in statement.cases() {
                    visitor.scoped(|visitor| {
                        for label in case.cases() {
                            if let SwitchCaseKind::Pattern(label) = label {
                                visitor.visit_pattern(label.pattern());
                                if let Some(guard) = label.guard() {
                                    visitor.visit_expr(guard);
                                }
                            }
                        }
                        for statement in case.body() {
                            visitor.visit_stmt(statement);
                        }
                    });
                }
            }),
            Stmt::PatternDecl(declaration) => {
                self.visit_expr(declaration.init());
                self.visit_pattern(declaration.pattern());
            }
            Stmt::PatternAssign(assignment) => {
                self.visit_expr(assignment.value());
                self.assigning_pattern = true;
                self.visit_pattern(assignment.pattern());
                self.assigning_pattern = false;
            }
            Stmt::LocalFunc(function) => {
                self.bind(function.name());
                self.callable_bindings
                    .insert(function.span().start, self.bindings());
                self.callable(Some(function.span().start), |visitor| {
                    walk_stmt(visitor, statement)
                });
            }
            _ => walk_stmt(self, statement),
        }
    }

    fn visit_element<E: Element<'a>>(&mut self, element: E) {
        match element.kind() {
            ElementKind::If {
                condition,
                then_element,
                else_element,
            } => {
                self.conditional(condition, |visitor| visitor.visit_element(then_element));
                if let Some(element) = else_element {
                    self.scoped(|visitor| visitor.visit_element(element));
                }
            }
            ElementKind::For { header, body, .. } => {
                self.scoped(|visitor| {
                    visitor.loop_init(header);
                    visitor.region(element.span().start, |visitor| {
                        visitor.loop_header(header);
                        visitor.visit_element(body);
                    });
                });
            }
            kind => walk_element(self, kind),
        }
    }

    fn visit_expr(&mut self, expression: Expr<'a>) {
        walk_expr_chain(self, expression, |visitor, expression| {
            let target = match expression {
                Expr::Assign(assignment) => Some(assignment.target()),
                Expr::Unary(unary)
                    if matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus) =>
                {
                    Some(unary.operand())
                }
                Expr::PostfixIncDec(postfix) => Some(postfix.operand()),
                _ => None,
            };
            if let Some(Expr::Ident(name)) = target.map(Expr::unparenthesized) {
                visitor.record(name);
            }
            match expression {
                Expr::FuncExpr(function) => {
                    visitor
                        .callable_bindings
                        .insert(function.span().start, visitor.bindings());
                    visitor.callable(Some(function.span().start), |visitor| {
                        walk_expr(visitor, expression)
                    });
                    false
                }
                Expr::Switch(switch) => {
                    visitor.visit_expr(switch.subject());
                    for arm in switch.arms() {
                        visitor.scoped(|visitor| {
                            visitor.visit_pattern(arm.pattern());
                            if let Some(guard) = arm.guard() {
                                visitor.visit_expr(guard);
                            }
                            visitor.visit_expr(arm.body());
                        });
                    }
                    false
                }
                _ => true,
            }
        });
    }

    fn visit_pattern(&mut self, pattern: Pattern<'a>) {
        match pattern {
            Pattern::Variable(variable) => self.pattern_variable(variable.name()),
            // `Type(:name)` binds the getter's name.
            Pattern::Object(object) => {
                for field in object.fields().filter(|field| field.pattern().is_none()) {
                    self.pattern_variable(field.name());
                }
            }
            _ => {}
        }
        walk_pattern(self, pattern);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use falcon_dart_parser::parse;

    use super::FlowWrites;

    #[test]
    fn long_binary_chains_preserve_writes_and_closure_scopes() {
        let mut source = String::from("void f(int a, int b, int c) { while (true) { ");
        source.push_str("(a = 1)");
        for _ in 1..99_999 {
            source.push_str(" + a");
        }
        source.push_str(" + (() { b = 2; return c++; })(); } }");
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let writes = FlowWrites::program(parsed.tree.program(&source));
        let region = source.find("while").unwrap();
        assert_eq!(
            writes.region_writes[&region],
            HashSet::from(["a".to_owned()])
        );
        assert_eq!(
            writes.captured_region_writes[&region],
            HashSet::from(["b".to_owned(), "c".to_owned()])
        );
        assert_eq!(
            writes.callable_captured_writes[&source.find("() {").unwrap()],
            HashSet::from(["b".to_owned(), "c".to_owned()])
        );
    }

    #[test]
    fn regions_record_direct_writes_of_enclosing_bindings() {
        let source = "void f(int? a, int? b, int c) { while (a != null) { a = b; var d = 0; d = 1; \
            void g() { c = 1; } } for (var i = 0; i < 1; i++) { (b, _) = (null, 0); } \
            try { c = 2; } catch (e) {} do { c++; } while (c < 3); }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let writes = FlowWrites::program(parsed.tree.program(source));
        let region = |text: &str| source.find(text).expect(text);
        assert_eq!(
            writes.region_writes,
            HashMap::from([
                (region("while (a"), HashSet::from(["a".to_owned()])),
                (
                    region("for ("),
                    HashSet::from(["i".to_owned(), "b".to_owned()])
                ),
                (region("try"), HashSet::from(["c".to_owned()])),
                (region("do {"), HashSet::from(["c".to_owned()])),
            ]),
        );
    }
    #[test]
    fn region_captures_come_only_from_closures_inside_the_region() {
        let source = "void f(int? x, List<int?> xs, List<Function> cbs) { \
            cbs.add(() { print(x); }); \
            while (true) { void h() { x = null; } } for (x in xs) {} try {} catch (e) {} \
            cbs.add(() => x = null); for (var x in xs) { x = null; } \
            var g = () { do { x = null; } while (true); }; }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let writes = FlowWrites::program(parsed.tree.program(source));
        let at = |text: &str| source.find(text).expect(text);
        let x = HashSet::from(["x".to_owned()]);
        assert_eq!(
            writes.captured_region_writes,
            HashMap::from([(at("while (true)"), x.clone())]),
        );
        for region in ["for (x in", "for (var x", "do {"] {
            assert_eq!(writes.region_writes[&at(region)], x);
        }
        // Every closure sees `x` as captured, including closures declared
        // before the one that writes it and closures that only read it.
        assert_eq!(writes.anywhere_captured.len(), 4);
        assert_eq!(writes.anywhere_captured[&at("() { print")], x);
        for callable in [at("() => x"), at("() { do")] {
            assert_eq!(writes.anywhere_captured[&callable], x);
        }
        assert!(writes.anywhere_captured.values().all(|names| names == &x));
        // Only the closures that write `x` capture it.
        assert_eq!(writes.callable_captured_writes.len(), 3);
        for callable in [at("() => x"), at("() { do")] {
            assert_eq!(writes.callable_captured_writes[&callable], x);
        }
    }

    #[test]
    fn captures_resolve_region_entry_bindings_without_scope_snapshots() {
        let source = "void f(int? x) { while (true) { \
            { int? x; void inner() { x = null; } } \
            while (true) { void outer() { x = null; } } } }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let writes = FlowWrites::program(parsed.tree.program(source));
        let x = HashSet::from(["x".to_owned()]);
        for (start, _) in source.match_indices("while") {
            assert_eq!(writes.captured_region_writes[&start], x);
        }
        assert_eq!(writes.captured_region_writes.len(), 2);
        let shadowed_source =
            "void f(int? x) { while (true) { int? x; void inner() { x = null; } } }";
        let shadowed = parse(shadowed_source);
        assert!(shadowed.errors.is_empty(), "{:?}", shadowed.errors);
        assert!(
            FlowWrites::program(shadowed.tree.program(shadowed_source))
                .captured_region_writes
                .is_empty()
        );
    }

    #[test]
    fn collection_case_bindings_do_not_escape_the_then_element() {
        let source = "void f(int? x, Object value) { while (true) { \
            var list = [if (value case int x) x else (x = null), (x = null)]; \
            var map = {if (value case int x) x: x else 0: (x = null)}; } }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let writes = FlowWrites::program(parsed.tree.program(source));
        assert_eq!(
            writes.region_writes[&source.find("while").unwrap()],
            HashSet::from(["x".to_owned()])
        );
    }

    #[test]
    fn cached_results_drop_traversal_only_bindings() {
        let source = "void f(int x) { void g() { x = 1; } while (true) { g(); } }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let writes = FlowWrites::program(parsed.tree.program(source));
        assert!(!writes.anywhere_writes.is_empty());
        assert!(!writes.anywhere_captured.is_empty());
        assert!(!writes.callable_captured_writes.is_empty());
        assert!(writes.scopes.is_empty());
        assert!(writes.callable_scopes.is_empty());
        assert!(writes.callable_ids.is_empty());
        assert!(writes.callable_captures.is_empty());
        assert!(writes.regions.is_empty());
        assert!(writes.callable_bindings.is_empty());
        assert!(writes.captured_bindings.is_empty());
        assert!(writes.written_bindings.is_empty());
    }
}
