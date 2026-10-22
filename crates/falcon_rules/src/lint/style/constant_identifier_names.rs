//! Flags constant identifiers that are not lowerCamelCase.
//!
//! Dart names constants in `lowerCamelCase`, not the `SCREAMING_CAPS` common in
//! other languages, so `maxCount` rather than `MAX_COUNT`. Following the
//! convention keeps constants visually consistent with every other identifier.
//! The rule covers `const` declarations at every scope — top-level, static and
//! instance fields, and locals — as well as enum values. The wildcard `_` is
//! permitted.

use crate::lint::naming::is_lower_camel_case;
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct ConstantIdentifierNames;

impl Rule for ConstantIdentifierNames {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

const MESSAGE: &str = "Name constant identifiers using `lowerCamelCase`.";

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Collector<'_> {
    fn check(&mut self, name: Identifier<'_>) {
        if !is_lower_camel_case(name.name()) {
            self.diags.push(
                self.ctx.diagnostic(MESSAGE, name.span()),
            );
        }
    }

    fn check_declarators(&mut self, declarators: AstChildren<'_, VarDeclarator<'_>>) {
        for d in declarators {
            self.check(d.name());
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Variable(v) = node
            && v.keyword() == VarKeyword::Const
        {
            self.check_declarators(v.declarators());
        }
        StructuralTraversal::Continue
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) -> StructuralTraversal {
        if node.keyword() == VarKeyword::Const {
            self.check_declarators(node.declarators());
        }
        StructuralTraversal::Continue
    }

    fn visit_enum_decl(&mut self, node: EnumDecl<'a>) -> StructuralTraversal {
        for variant in node.variants() {
            self.check(variant.name());
        }
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) -> StructuralTraversal {
        if node.keyword() == VarKeyword::Const {
            self.check_declarators(node.declarators());
        }
        StructuralTraversal::Continue
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
        let parsed = parse(source);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let program = parsed.tree.program(source);
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, &config, &parsed);
        ConstantIdentifierNames.analyze(ConstantIdentifierNames::NAME, program, &ctx).len()
    }

    #[test]
    fn const_inside_closure_body_is_reported() {
        // Closures live in expression position — only expression descent reaches
        // this declaration.
        let src = "void f() { final g = () { const MAX_LEN = 3; return MAX_LEN; }; }";
        assert_eq!(run(src), 1, "const in closure body must fire");
    }

    #[test]
    fn const_in_c_style_for_initializer_is_reported() {
        // The initializer decl is only visited via `ForInit::VarDecl`.
        let src = "void f() { for (const MAX_LEN = 3;;) {} }";
        assert_eq!(run(src), 1, "const in for initializer must fire");
    }

    #[test]
    fn const_reached_through_return_and_expression_statements_is_reported() {
        // One closure behind a `return`, one behind an expression statement.
        let src = "Function f() => () { const A_B = 1; return A_B; };\n\
                   void g() { run(() { const C_D = 2; }); }";
        assert_eq!(run(src), 2, "return and expr-stmt paths must fire");
    }

    #[test]
    fn non_const_locals_are_ignored() {
        let src = "void f() { var MAX_LEN = 3; final OTHER = 4; }";
        assert_eq!(run(src), 0, "only const declarations are checked");
    }

    #[test]
    fn lower_camel_case_constants_are_clean() {
        let src =
            "const maxLen = 3;\nclass A { static const otherLen = 4; }\nenum E { first, second }";
        assert_eq!(run(src), 0, "lowerCamelCase constants must not fire");
    }
}
