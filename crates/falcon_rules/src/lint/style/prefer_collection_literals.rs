//! Flags default `List()`/`Map()`/`Set()`/`LinkedHashMap()`/`LinkedHashSet()`
//! constructor invocations that a collection literal (`[]`/`{}`) would express.
//! Adopted from package:lints `prefer_collection_literals`.
//!
//! Type knowledge only ever *suppresses*. Two positively-proven facts withhold a
//! diagnostic that would otherwise change semantics:
//!
//! * **Concrete context** — a `LinkedHashMap()` / `LinkedHashSet()` whose declared
//!   context type (a variable/field/top-level/return annotation) *is* that concrete
//!   type. A `{}` literal has static type `Map`/`Set`, which is not assignable back
//!   to `LinkedHashMap`/`LinkedHashSet`, so the constructor is required.
//! * **User-declared shadow** — a `List()`/`Map()`/`Set()`/… whose name is a
//!   *user-declared* type in the [`TypeIndex`] (not `dart:core`). The literal would
//!   construct the core collection, not the user's type.
//!
//! Everything else — including every constructor when no type index is attached, and
//! any `var`/unannotated context — keeps firing exactly as before.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule, TypeIndex};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_expr, walk_field_decl, walk_function_decl, walk_getter_decl, walk_method_decl,
    walk_stmt, walk_top_level_decl,
};

pub struct PreferCollectionLiterals;

/// Constructors whose default invocation is expressible as a literal.
const NAMES: [&str; 5] = ["List", "Map", "Set", "LinkedHashMap", "LinkedHashSet"];

/// Concrete collection types a `{}`/`[]` literal cannot stand in for — a matching
/// declared context type *requires* the constructor.
const CONCRETE: [&str; 2] = ["LinkedHashMap", "LinkedHashSet"];

impl Rule for PreferCollectionLiterals {
    fn name(&self) -> &'static str {
        "prefer-collection-literals"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        // Pre-pass: spans of constructors sitting in a concrete-typed slot, which a
        // collection literal would not preserve.
        let mut scan = ContextScan {
            suppressed: HashSet::new(),
        };
        scan.visit_program(program);

        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            types: ctx.types,
            suppressed: &scan.suppressed,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    file: String,
    types: Option<&'a TypeIndex>,
    suppressed: &'a HashSet<(usize, usize)>,
}

impl Collector<'_> {
    fn push(&mut self, span: &Span) {
        self.diags.push(Diagnostic::new(
            "prefer-collection-literals",
            Severity::Warning,
            "Use a collection literal instead of a constructor invocation.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Some((span, name)) = default_collection_ctor(node) {
            // A user type shadowing a core collection name → the literal would
            // build the core type, not the user's. `type_kind` is `Some` only for
            // user declarations (core names carry none).
            let user_shadow = self.types.is_some_and(|t| t.type_kind(name).is_some());
            let context_required = self.suppressed.contains(&(span.start, span.end));
            if !user_shadow && !context_required {
                self.push(span);
            }
        }
        walk_expr(self, node);
    }
}

/// Pre-pass collecting constructor spans that must be suppressed because their
/// immediate declared context requires the concrete collection type.
struct ContextScan {
    suppressed: HashSet<(usize, usize)>,
}

impl ContextScan {
    /// Record any declarator initializer that constructs the concrete type named
    /// by `ty`.
    fn record_decls(&mut self, ty: &Option<DartType>, decls: &[VarDeclarator]) {
        let Some(ty) = ty else { return };
        let Some(expected) = concrete_context(ty) else {
            return;
        };
        for d in decls {
            if let Some(init) = &d.initializer {
                self.record_if_matching(init, expected);
            }
        }
    }

    /// Record `expr`'s span when it is a default constructor for `expected`.
    fn record_if_matching(&mut self, expr: &Expr, expected: &str) {
        if let Some((span, base)) = default_collection_ctor(expr)
            && base == expected
        {
            self.suppressed.insert((span.start, span.end));
        }
    }

    /// Record returns in a function body whose declared return type is `expected`.
    fn collect_returns(&mut self, body: &FunctionBody, expected: &str) {
        match body {
            FunctionBody::Arrow(e, _) => self.record_if_matching(e, expected),
            FunctionBody::Block(b) => {
                for s in &b.stmts {
                    self.collect_stmt_returns(s, expected);
                }
            }
            FunctionBody::Native(_, _) => {}
        }
    }

    /// Recurse through control-flow statements recording `return` values that
    /// construct `expected`. Stops at nested function boundaries (a `LocalFunc`
    /// carries its own return context and is walked separately).
    fn collect_stmt_returns(&mut self, stmt: &Stmt, expected: &str) {
        match stmt {
            Stmt::Return(r) => {
                if let Some(v) = &r.value {
                    self.record_if_matching(v, expected);
                }
            }
            Stmt::Block(b) => {
                for s in &b.stmts {
                    self.collect_stmt_returns(s, expected);
                }
            }
            Stmt::If(i) => {
                self.collect_stmt_returns(&i.then_branch, expected);
                if let Some(e) = &i.else_branch {
                    self.collect_stmt_returns(e, expected);
                }
            }
            Stmt::While(w) => self.collect_stmt_returns(&w.body, expected),
            Stmt::DoWhile(d) => self.collect_stmt_returns(&d.body, expected),
            Stmt::For(f) => self.collect_stmt_returns(&f.body, expected),
            Stmt::TryCatch(tc) => {
                for s in &tc.body.stmts {
                    self.collect_stmt_returns(s, expected);
                }
                for catch in &tc.catches {
                    for s in &catch.body.stmts {
                        self.collect_stmt_returns(s, expected);
                    }
                }
                if let Some(fin) = &tc.finally {
                    for s in &fin.stmts {
                        self.collect_stmt_returns(s, expected);
                    }
                }
            }
            Stmt::Switch(sw) => {
                for case in &sw.cases {
                    for s in &case.body {
                        self.collect_stmt_returns(s, expected);
                    }
                }
            }
            Stmt::Labeled(l) => self.collect_stmt_returns(&l.stmt, expected),
            _ => {}
        }
    }
}

impl Visitor for ContextScan {
    fn visit_field_decl(&mut self, node: &FieldDecl) {
        self.record_decls(&node.field_type, &node.declarators);
        walk_field_decl(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalVar(lv) = node {
            self.record_decls(&lv.var_type, &lv.declarators);
        }
        walk_stmt(self, node);
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(v) = node {
            self.record_decls(&v.var_type, &v.declarators);
        }
        walk_top_level_decl(self, node);
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        if let Some(rt) = &node.return_type
            && let Some(expected) = concrete_context(rt)
            && let Some(body) = &node.body
        {
            self.collect_returns(body, expected);
        }
        walk_function_decl(self, node);
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        if let Some(rt) = &node.return_type
            && let Some(expected) = concrete_context(rt)
            && let Some(body) = &node.body
        {
            self.collect_returns(body, expected);
        }
        walk_method_decl(self, node);
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        if let Some(rt) = &node.return_type
            && let Some(expected) = concrete_context(rt)
            && let Some(body) = &node.body
        {
            self.collect_returns(body, expected);
        }
        walk_getter_decl(self, node);
    }
}

/// The concrete collection type name (`LinkedHashMap`/`LinkedHashSet`) written as
/// `ty`, ignoring nullability and type arguments; `None` for anything else.
fn concrete_context(ty: &DartType) -> Option<&str> {
    if let DartType::Named(nt) = ty
        && let Some(seg) = nt.segments.last()
        && CONCRETE.contains(&seg.name.as_str())
    {
        return Some(seg.name.as_str());
    }
    None
}

/// Span and base type name of a *default* constructor invocation of a known
/// collection type with no arguments (`List()`, `Map<K, V>()`, `LinkedHashSet.new()`,
/// `new Set()`); `None` for named constructors (`List.filled`, `Map.of`) and any
/// invocation that carries arguments.
fn default_collection_ctor(expr: &Expr) -> Option<(&Span, &str)> {
    match expr {
        Expr::Call {
            callee, args, span, ..
        } => {
            if !args.positional.is_empty() || !args.named.is_empty() {
                return None;
            }
            match &**callee {
                Expr::Ident(id) if NAMES.contains(&id.name.as_str()) => {
                    Some((span, id.name.as_str()))
                }
                Expr::Field { object, field, .. } if field.name == "new" => match &**object {
                    Expr::Ident(id) if NAMES.contains(&id.name.as_str()) => {
                        Some((span, id.name.as_str()))
                    }
                    _ => None,
                },
                _ => None,
            }
        }
        Expr::New {
            dart_type: DartType::Named(nt),
            constructor_name,
            args,
            span,
            ..
        } => {
            if !args.positional.is_empty() || !args.named.is_empty() {
                return None;
            }
            let base = nt.segments.last()?;
            if !NAMES.contains(&base.name.as_str()) {
                return None;
            }
            match constructor_name {
                None => Some((span, base.name.as_str())),
                Some(c) if c.name == "new" => Some((span, base.name.as_str())),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    fn count_no_types(source: &str) -> usize {
        let program = parse(source).0;
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, source, &config);
        PreferCollectionLiterals.analyze(&program, &ctx).len()
    }

    fn count_with_types(source: &str) -> usize {
        let program = parse(source).0;
        let types = TypeIndex::from_program(&program);
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, source, &config).with_types(&types);
        PreferCollectionLiterals.analyze(&program, &ctx).len()
    }

    #[test]
    fn bare_constructors_fire_with_and_without_types() {
        let src = "import 'dart:collection'; \
                   void f() { var a = List(); var b = Map(); var c = LinkedHashMap(); }";
        assert_eq!(count_no_types(src), 3);
        assert_eq!(count_with_types(src), 3);
    }

    #[test]
    fn concrete_context_suppresses_only_relevant_ctor() {
        // Declared `LinkedHashMap` → `{}` would not preserve it → suppress. Context
        // suppression is syntactic, so it holds with or without a type index.
        let src = "import 'dart:collection'; \
                   void f() { LinkedHashMap<int, int> m = LinkedHashMap(); }";
        assert_eq!(count_no_types(src), 0);
        assert_eq!(count_with_types(src), 0);
    }

    #[test]
    fn non_concrete_context_still_fires() {
        // Declared `Map` → a `{}` literal is assignable → still fires.
        let src = "import 'dart:collection'; \
                   void f() { Map<int, int> m = LinkedHashMap(); }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn var_context_still_fires() {
        // No annotation on `var` → nothing required → fires.
        let src = "import 'dart:collection'; void f() { var m = LinkedHashMap(); }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn nested_arg_position_still_fires() {
        // The constructor is an argument, not the direct initializer, so the
        // concrete context does not reach it.
        let src = "import 'dart:collection'; \
                   LinkedHashMap<int, int> id(LinkedHashMap<int, int> x) => x; \
                   void f() { LinkedHashMap<int, int> m = id(LinkedHashMap()); }";
        // Outer `id(...)` initializer is a call, not a ctor; inner `LinkedHashMap()`
        // is an argument → fires once.
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn concrete_return_type_suppresses() {
        let src = "import 'dart:collection'; \
                   LinkedHashSet<int> make() => LinkedHashSet(); \
                   LinkedHashMap<int, int> build() { return LinkedHashMap(); }";
        assert_eq!(count_with_types(src), 0);
    }

    #[test]
    fn concrete_field_suppresses() {
        let src = "import 'dart:collection'; \
                   class C { LinkedHashMap<int, int> m = LinkedHashMap(); }";
        assert_eq!(count_with_types(src), 0);
    }

    #[test]
    fn user_declared_shadow_suppresses_only_with_types() {
        // A user `List` shadows core `List` → the literal would build the wrong type.
        let src = "class List {} void f() { var a = List(); }";
        assert_eq!(count_no_types(src), 1, "no type index → fire (baseline)");
        assert_eq!(count_with_types(src), 0, "user-declared List → suppress");
    }

    #[test]
    fn named_constructors_and_literals_never_fire() {
        let src = "void f() { var a = List.filled(3, 0); var b = <int>[]; var c = {}; }";
        assert_eq!(count_no_types(src), 0);
    }
}
