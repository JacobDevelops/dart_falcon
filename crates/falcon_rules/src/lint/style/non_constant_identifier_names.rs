//! Flags non-constant identifiers that are not lowerCamelCase. Ported from
//! package:lints `non_constant_identifier_names`. Covers variables (non-const),
//! formal parameters, catch clause bindings, pattern variables, named
//! constructor names, and function/method/getter/setter names. Operator names
//! are exempt (they are not identifiers). Type names (see `camel-case-types`)
//! and constants (see `constant-identifier-names`) are out of scope.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_constructor_decl, walk_field_decl, walk_formal_param, walk_function_decl,
    walk_getter_decl, walk_method_decl, walk_pattern, walk_setter_decl, walk_stmt,
    walk_top_level_decl,
};

pub struct NonConstantIdentifierNames;

impl Rule for NonConstantIdentifierNames {
    fn name(&self) -> &'static str {
        "non-constant-identifier-names"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

const MESSAGE: &str = "Name non-constant identifiers using lowerCamelCase.";

/// lowerCamelCase per the analyzer's `isLowerCamelCase`: an all-underscore name
/// (a wildcard) is allowed, leading underscores are ignored, then the remainder
/// must start with a lowercase letter or `$` and contain no further underscores.
/// A single uppercase letter is also accepted (mirrors the analyzer helper).
fn is_lower_camel_case(name: &str) -> bool {
    if !name.is_empty() && name.bytes().all(|b| b == b'_') {
        return true;
    }
    if name.len() == 1 && name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
        return true;
    }
    let rest = name.trim_start_matches('_');
    let Some(first) = rest.chars().next() else {
        return true;
    };
    if !(first.is_ascii_lowercase() || first == '$') {
        return false;
    }
    rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '$')
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl Collector {
    fn check(&mut self, name: &Identifier) {
        if !is_lower_camel_case(&name.name) {
            self.diags.push(Diagnostic::new(
                "non-constant-identifier-names",
                Severity::Warning,
                MESSAGE,
                self.file.clone(),
                DiagSpan {
                    start: name.span.start,
                    end: name.span.end,
                },
            ));
        }
    }

    /// Constants are governed by `constant-identifier-names` instead.
    fn check_declarators(&mut self, declarators: &[VarDeclarator], is_const: bool) {
        if is_const {
            return;
        }
        for d in declarators {
            self.check(&d.name);
        }
    }
}

impl Visitor for Collector {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(v) = node {
            self.check_declarators(&v.declarators, v.is_const);
        }
        walk_top_level_decl(self, node);
    }

    /// Covers top-level functions, getters, and setters alike — the analyzer
    /// exempts only operators and augmentations.
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.check(&node.name);
        walk_function_decl(self, node);
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.check(&node.name);
        walk_method_decl(self, node);
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        self.check(&node.name);
        walk_getter_decl(self, node);
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        self.check(&node.name);
        self.check(&node.param);
        walk_setter_decl(self, node);
    }

    /// Only the named part of a constructor is an identifier; the leading type
    /// name is governed by `camel-case-types`.
    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        if let Some(named) = &node.constructor_name {
            self.check(named);
        }
        walk_constructor_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        self.check_declarators(&node.declarators, node.is_const);
        walk_field_decl(self, node);
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        self.check(&node.name);
        walk_formal_param(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::LocalVar(lv) => self.check_declarators(&lv.declarators, lv.is_const),
            Stmt::LocalFunc(lf) => self.check(&lf.name),
            // `ForInit::VarDecl` re-enters as `Stmt::LocalVar` and `PatternForIn`
            // as a pattern, so only the `for-in` binding needs handling here.
            Stmt::For(f) => {
                if let Some(ForInit::ForIn { name, .. }) = &f.init {
                    self.check(name);
                }
            }
            Stmt::TryCatch(tc) => {
                for catch in &tc.catches {
                    if let Some(v) = &catch.exception_var {
                        self.check(v);
                    }
                    if let Some(v) = &catch.stack_trace_var {
                        self.check(v);
                    }
                }
            }
            _ => {}
        }
        walk_stmt(self, node);
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        if let Pattern::Variable { name, .. } = node {
            self.check(name);
        }
        walk_pattern(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    fn run(source: &str) -> usize {
        let program = parse(source).0;
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, source, &config);
        NonConstantIdentifierNames.analyze(&program, &ctx).len()
    }

    #[test]
    fn descends_into_every_expression_kind() {
        // Regression: the old hand-rolled walker only entered FuncExpr, Call,
        // Binary, Await, Throw, Assign.value and Conditional branches.
        let cases = [
            "void f() { var l = [(int Bad_P) => 0]; }",
            "void f() { var s = {(int Bad_P) => 0}; }",
            "void f() { var m = {'k': (int Bad_P) => 0}; }",
            "void f() { var r = (1, (int Bad_P) => 0); }",
            "void f() { var n = new Foo((int Bad_P) => 0); }",
            "void f(x) { x..m((int Bad_P) => 0); }",
            "void f(x) { x[(int Bad_P) => 0]; }",
            "void f(x) { x.g((int Bad_P) => 0).h; }",
            "void f(x) { x = ((int Bad_P) => 0) as X; }",
            "void f(x) { x!((int Bad_P) => 0); }",
            "void f() { var s = '${(int Bad_P) => 0}'; }",
        ];
        for src in cases {
            assert_eq!(run(src), 1, "closure param missed in: {src}");
        }
    }

    #[test]
    fn descends_into_assign_target_and_conditional_condition() {
        assert_eq!(run("void f(m) { m[(int Bad_P) => 0] = 1; }"), 1);
        assert_eq!(run("void f(c) { ((int Bad_P) => true)(1) ? 1 : 2; }"), 1);
    }

    #[test]
    fn scans_top_level_and_field_initializers() {
        assert_eq!(run("final f = (int Bad_P) { return 0; };"), 1);
        assert_eq!(run("class A { final f = (int Bad_P) => 0; }"), 1);
    }

    #[test]
    fn checks_all_for_loop_init_forms() {
        // Defect 3: only `ForInit::ForIn` was handled.
        assert_eq!(
            run("void f() { for (var Bad_I = 0; Bad_I < 1; Bad_I++) {} }"),
            1
        );
        assert_eq!(run("void f(xs) { for (final (Bad_A, Bad_B) in xs) {} }"), 2);
        assert_eq!(run("void f(xs) { for (var Bad_X in xs) {} }"), 1);
    }

    #[test]
    fn checks_catch_clause_bindings() {
        // Defect 4: catch bodies were scanned but the bindings never checked.
        assert_eq!(run("void f() { try {} catch (Bad_E, Bad_St) {} }"), 2);
        assert_eq!(run("void f() { try {} catch (_, __) {} }"), 0);
    }

    #[test]
    fn checks_getter_and_setter_names() {
        // Defect 5: the analyzer exempts only operators, not accessors.
        assert_eq!(run("int get Bad_G => 0;"), 1);
        assert_eq!(run("set Bad_S(int v) {}"), 1);
        assert_eq!(run("class A { int get Bad_G => 0; }"), 1);
        assert_eq!(run("class A { set Bad_S(int v) {} }"), 1);
        assert_eq!(run("class A { int get ok => 0; set ok(int v) {} }"), 0);
    }

    #[test]
    fn operator_names_are_exempt_but_their_params_are_not() {
        assert_eq!(run("class A { A operator +(A Bad_P) => this; }"), 1);
    }

    #[test]
    fn constants_are_out_of_scope() {
        assert_eq!(run("const My_Const = 1;"), 0);
        assert_eq!(run("class A { static const My_Const = 1; }"), 0);
        assert_eq!(run("void f() { const My_Const = 1; }"), 0);
    }
}
