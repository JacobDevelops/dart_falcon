//! Flags non-constant identifiers that are not written in lowerCamelCase.
//!
//! Dart convention reserves lowerCamelCase for everything that holds a runtime
//! value — non-const variables and fields, formal and closure parameters, for-in
//! and pattern variables, catch clause bindings, named constructors, and
//! function, method, getter, and setter names. Consistent casing lets readers
//! distinguish these from types (UpperCamelCase) and compile-time constants at a
//! glance. Operator names are exempt: they are not identifiers. The check mirrors
//! the analyzer's `isLowerCamelCase`: leading underscores are ignored, an
//! all-underscore wildcard name is accepted, a single uppercase letter is
//! tolerated, and the remainder must begin with a lowercase letter or `$` and
//! contain no further underscores. Type names (see `camel-case-types`) and
//! constants (see `constant-identifier-names`) are covered by separate rules and
//! are out of scope here.

use crate::lint::naming::is_lower_camel_case;
use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_field_decl, walk_formal_param, walk_function_decl, walk_getter_decl,
    walk_method_decl, walk_pattern, walk_setter_decl, walk_stmt, walk_top_level_decl,
};

#[derive(Default)]
pub struct NonConstantIdentifierNames;

impl Rule for NonConstantIdentifierNames {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

const MESSAGE: &str = "Name non-constant identifiers using lowerCamelCase.";

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Collector<'_> {
    fn check(&mut self, name: &Identifier) {
        if !is_lower_camel_case(&name.name) {
            self.diags.push(self.ctx.diagnostic(
                "non-constant-identifier-names",
                MESSAGE,
                name.span,
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

impl Visitor for Collector<'_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(v) = node {
            self.check_declarators(
                &v.declarators,
                v.keyword == falcon_syntax::ast::VarKeyword::Const,
            );
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
    fn visit_constructor(&mut self, node: ConstructorRef<'_>) {
        if let Some(named) = &node.constructor_name {
            self.check(named);
        }
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        self.check_declarators(
            &node.declarators,
            node.keyword == falcon_syntax::ast::VarKeyword::Const,
        );
        walk_field_decl(self, node);
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        self.check(&node.name);
        walk_formal_param(self, node);
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        self.check_declarators(
            &node.declarators,
            node.keyword == falcon_syntax::ast::VarKeyword::Const,
        );
        falcon_syntax::visitor::walk_local_var(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::LocalFunc(lf) => self.check(&lf.name),
            // Local declarations and pattern bindings use their dedicated hooks.
            Stmt::For(f) => {
                if let Some(ForInit::ForIn {
                    declares_variable: true,
                    name,
                    ..
                }) = &f.header.init
                {
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
        let falcon_dart_parser::ParseResult {
            program, syntax, ..
        } = parse(source);
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, source, &config, &syntax);
        NonConstantIdentifierNames.analyze(&program, &ctx).len()
    }

    #[test]
    fn descends_into_every_expression_kind() {
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
