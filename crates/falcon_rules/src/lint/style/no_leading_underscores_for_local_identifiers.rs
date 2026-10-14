//! Flags local variables and parameters whose names begin with an underscore.
//!
//! A leading underscore marks a declaration as library-private, but that
//! privacy only applies to top-level and class members — locals, parameters,
//! closure parameters, for-in variables, and catch clause bindings are already
//! confined to their scope. Prefixing them with an underscore implies a privacy
//! that does not exist and misleads the reader, so drop it. Wildcard names made
//! solely of underscores (e.g. `_`, `__`) are exempt, being the idiomatic way
//! to name a deliberately unused binding.

use crate::lint::naming::has_leading_underscore;
use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_formal_param, walk_pattern, walk_setter_decl, walk_stmt,
};

#[derive(Default)]
pub struct NoLeadingUnderscoresForLocalIdentifiers;

impl Rule for NoLeadingUnderscoresForLocalIdentifiers {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

const MESSAGE: &str = "Avoid leading underscores for local identifiers.";

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Collector<'_> {
    fn check(&mut self, name: &Identifier) {
        if has_leading_underscore(&name.name) {
            self.diags.push(self.ctx.diagnostic(
                "no-leading-underscores-for-local-identifiers",
                MESSAGE,
                name.span,
            ));
        }
    }
}

impl Visitor for Collector<'_> {
    /// Every formal parameter is a local binding: top-level and member
    /// functions, closures, operators, and function-typed parameter signatures.
    ///
    /// Initializing formals (`this.x`) and super formals (`super.x`) are not
    /// local identifiers — the underscore is the *field's* privacy and the
    /// spelling is forced — so upstream skips them ("These are not local
    /// identifiers.") and so do we. The default value is still walked, since it
    /// can host a closure with its own parameters.
    fn visit_formal_param(&mut self, node: &FormalParam) {
        if node.initializing.is_none() {
            self.check(&node.name);
        }
        walk_formal_param(self, node);
    }

    /// A class setter's value parameter is not part of a `FormalParamList`, so
    /// it needs its own check.
    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        self.check(&node.param);
        walk_setter_decl(self, node);
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        for declarator in &node.declarators {
            self.check(&declarator.name);
        }
        falcon_syntax::visitor::walk_local_var(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            // Local declarations and pattern bindings use their dedicated hooks.
            Stmt::For(f) => {
                if let Some(ForInit::ForIn { name, .. }) = &f.header.init {
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

    /// Declared variable patterns bind locals: `final (_a, _b) = pair`,
    /// `case final _x`, `for (final (_a, _b) in xs)`.
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
        NoLeadingUnderscoresForLocalIdentifiers
            .analyze(&program, &ctx)
            .len()
    }

    #[test]
    fn descends_into_every_expression_kind() {
        let cases = [
            "void f() { var l = [(int _a) => 0]; }",
            "void f() { var s = {(int _a) => 0}; }",
            "void f() { var m = {'k': (int _a) => 0}; }",
            "void f() { var r = (1, (int _a) => 0); }",
            "void f() { var n = Foo((int _a) => 0); }",
            "void f() { var n = new Foo((int _a) => 0); }",
            "void f(x) { x..m((int _a) => 0); }",
            "void f(x) { x[(int _a) => 0]; }",
            "void f(x) { x.g((int _a) => 0).h; }",
            "void f() { !(((int _a) => 0) is X); }",
            "void f(x) { x = ((int _a) => 0) as X; }",
            "void f(x) { x!((int _a) => 0); }",
            "void f() { var s = '${(int _a) => 0}'; }",
            "void f(x) { x++; var q = switch (x) { _ => (int _a) => 0 }; }",
        ];
        for src in cases {
            assert_eq!(run(src), 1, "closure param missed in: {src}");
        }
    }

    #[test]
    fn descends_into_assign_target_and_conditional_condition() {
        // `Assign.target` and `Conditional.condition` were never scanned.
        assert_eq!(run("void f(m) { m[(int _a) => 0] = 1; }"), 1);
        assert_eq!(run("void f(c) { ((int _a) => true)(1) ? 1 : 2; }"), 1);
    }

    #[test]
    fn scans_top_level_and_field_initializers() {
        // Defect 2: `scan_top` had no Variable arm and `scan_member` no Field arm.
        assert_eq!(run("final f = (int _private) { return 0; };"), 1);
        assert_eq!(run("class A { final f = (int _private) => 0; }"), 1);
    }

    #[test]
    fn checks_all_for_loop_init_forms() {
        // Defect 3: only `ForInit::ForIn` was handled.
        assert_eq!(run("void f() { for (var _i = 0; _i < 1; _i++) {} }"), 1);
        assert_eq!(run("void f(xs) { for (final (_a, _b) in xs) {} }"), 2);
        assert_eq!(run("void f(xs) { for (var _x in xs) {} }"), 1);
        // `Exprs` init declares nothing but may host a closure.
        assert_eq!(run("void f(xs) { for (xs((int _a) => 0);;) {} }"), 1);
    }

    #[test]
    fn checks_catch_clause_bindings() {
        assert_eq!(run("void f() { try {} catch (_e, _st) {} }"), 2);
        assert_eq!(run("void f() { try {} catch (_, __) {} }"), 0);
    }

    #[test]
    fn ignores_initializing_and_super_formals() {
        // Upstream `visitFormalParameterList` skips FieldFormalParameter and
        // SuperFormalParameter: the underscore is the field's privacy.
        assert_eq!(run("class C { final int _t; C(this._t); }"), 0);
        assert_eq!(run("class C { C({required this._t}); final int _t; }"), 0);
        assert_eq!(run("class D extends C { D(super._t); }"), 0);
        // Ordinary parameters alongside them are still reported.
        assert_eq!(run("class C { final int _t; C(this._t, int _other); }"), 1);
        // A function-typed field formal's own parameters are still local.
        assert_eq!(run("class C { final f; C(this.f(int _a)); }"), 1);
    }

    #[test]
    fn ignores_private_declarations_that_are_not_local() {
        assert_eq!(run("int _topLevel = 0;"), 0);
        assert_eq!(run("class A { int _field = 0; void _method() {} }"), 0);
        assert_eq!(run("void f() { print(_someRef); }"), 0);
    }
}
