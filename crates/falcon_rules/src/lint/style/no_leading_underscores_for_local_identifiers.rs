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
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct NoLeadingUnderscoresForLocalIdentifiers;

impl Rule for NoLeadingUnderscoresForLocalIdentifiers {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
            primary_params: None,
            representation_params: None,
            field_shortcuts: Vec::new(),
            shortcut_lengths: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

const MESSAGE: &str = "Avoid leading underscores for local identifiers.";

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    primary_params: Option<usize>,
    representation_params: Option<usize>,
    field_shortcuts: Vec<usize>,
    shortcut_lengths: Vec<usize>,
}

impl Collector<'_> {
    fn check(&mut self, name: Identifier<'_>) {
        if has_leading_underscore(name.name()) {
            self.diags.push(self.ctx.diagnostic(
                "no-leading-underscores-for-local-identifiers",
                MESSAGE,
                name.span(),
            ));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.representation_params = match node {
            TopLevelDecl::ExtensionType(extension) => {
                Some(extension.primary_constructor().params().span().start)
            }
            _ => None,
        };
        StructuralTraversal::Continue
    }

    fn visit_primary_constructor(&mut self, node: PrimaryConstructor<'a>) -> StructuralTraversal {
        self.primary_params = Some(node.params().span().start);
        StructuralTraversal::Continue
    }

    fn exit_primary_constructor(&mut self, _node: PrimaryConstructor<'a>) {
        self.primary_params = None;
    }

    fn visit_formal_param_list(&mut self, node: FormalParamList<'a>) -> StructuralTraversal {
        if self.representation_params != Some(node.span().start) {
            for parameter in node.params() {
                if parameter.initializing().is_some()
                    || (self.primary_params == Some(node.span().start)
                        && (parameter.is_var() || parameter.is_final()))
                {
                    break;
                }
                if !matches!(parameter.kind(), ParamKind::Named { .. }) {
                    self.check(parameter.name());
                }
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_dart_type(&mut self, node: DartType<'a>) -> StructuralTraversal {
        if let DartType::Function(function) = node {
            for parameter in function.params() {
                if !parameter.is_named()
                    && let Some(name) = parameter.name()
                {
                    self.check(name);
                }
            }
        }
        StructuralTraversal::Continue
    }

    /// A class setter's value parameter is not part of a `FormalParamList`, so
    /// it needs its own check.
    fn visit_setter_decl(&mut self, node: SetterDecl<'a>) -> StructuralTraversal {
        self.check(node.param());
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) -> StructuralTraversal {
        for declarator in node.declarators() {
            self.check(declarator.name());
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        match node {
            // Local declarations and pattern bindings use their dedicated hooks.
            Stmt::LocalFunc(function) => self.check(function.name()),
            Stmt::For(f) => {
                if let Some(ForInit::ForIn(init)) = f.header().init()
                    && init.declares_variable()
                {
                    self.check(init.name());
                }
            }
            Stmt::TryCatch(tc) => {
                for catch in tc.catches() {
                    if let Some(v) = catch.exception_var() {
                        self.check(v);
                    }
                    if let Some(v) = catch.stack_trace_var() {
                        self.check(v);
                    }
                }
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    /// Declared variable patterns bind locals: `final (_a, _b) = pair`,
    /// `case final _x`, `for (final (_a, _b) in xs)`.
    fn visit_pattern(&mut self, node: Pattern<'a>) -> StructuralTraversal {
        self.shortcut_lengths.push(self.field_shortcuts.len());
        if let Pattern::Object(object) = node {
            for field in object.fields() {
                if self.ctx.source().as_bytes().get(field.span().start) == Some(&b':') {
                    self.field_shortcuts.push(field.name().span().start);
                }
            }
        }
        if let Pattern::Variable(variable) = node
            && !self.field_shortcuts.contains(&variable.name().span().start)
        {
            self.check(variable.name());
        }
        StructuralTraversal::Continue
    }

    fn exit_pattern(&mut self, _node: Pattern<'a>) {
        self.field_shortcuts.truncate(self.shortcut_lengths.pop().unwrap());
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    fn run(source: &str) -> usize {
        let falcon_dart_parser::ParseResult { tree, .. } = parse(source);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, &config, &parsed);
        NoLeadingUnderscoresForLocalIdentifiers
            .analyze(program, &ctx)
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
        // Upstream stops checking this list at its first field formal.
        assert_eq!(run("class C { final int _t; C(this._t, int _other); }"), 0);
        // A function-typed field formal's own parameters are still local.
        assert_eq!(run("class C { final f; C(this.f(int _a)); }"), 1);
        assert_eq!(run("class C { final int t; C(int _other, this.t); }"), 1);
    }

    #[test]
    fn distinguishes_private_fields_from_local_bindings() {
        assert_eq!(run("class C(final int _field);"), 0);
        assert_eq!(run("class C(var int _field);"), 0);
        assert_eq!(run("class C(int _argument);"), 1);
        assert_eq!(run("extension type C(int _representation) {}"), 0);
        assert_eq!(run("void f() { void _local() {} }"), 1);
        assert_eq!(run("void f(x) { if (x case C(:var _field)) {} }"), 0);
        assert_eq!(run("void f(x) { if (x case C(_field: var _local)) {} }"), 1);
    }

    #[test]
    fn ignores_private_declarations_that_are_not_local() {
        assert_eq!(run("int _topLevel = 0;"), 0);
        assert_eq!(run("class A { int _field = 0; void _method() {} }"), 0);
        assert_eq!(run("void f() { print(_someRef); }"), 0);
    }
}
