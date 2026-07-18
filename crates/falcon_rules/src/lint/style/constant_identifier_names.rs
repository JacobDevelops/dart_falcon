//! Flags constant identifiers that are not lowerCamelCase.
//!
//! Dart names constants in `lowerCamelCase`, not the `SCREAMING_CAPS` common in
//! other languages, so `maxCount` rather than `MAX_COUNT`. Following the
//! convention keeps constants visually consistent with every other identifier.
//! The rule covers `const` declarations at every scope — top-level, static and
//! instance fields, and locals — as well as enum values. All-underscore
//! wildcard names are permitted.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_enum_decl, walk_field_decl, walk_stmt, walk_top_level_decl,
};

pub struct ConstantIdentifierNames;

impl Rule for ConstantIdentifierNames {
    fn name(&self) -> &'static str {
        "constant-identifier-names"
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

const MESSAGE: &str = "Name constant identifiers using lowerCamelCase.";

/// lowerCamelCase per the analyzer's `isLowerCamelCase`: an all-underscore name
/// (a wildcard) is allowed, leading underscores are ignored, then the remainder
/// must start with a lowercase letter or `$` and contain no further underscores.
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
                "constant-identifier-names",
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

    fn check_declarators(&mut self, declarators: &[VarDeclarator]) {
        for d in declarators {
            self.check(&d.name);
        }
    }
}

impl Visitor for Collector {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(v) = node
            && v.is_const
        {
            self.check_declarators(&v.declarators);
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        if node.is_const {
            self.check_declarators(&node.declarators);
        }
        walk_field_decl(self, node);
    }

    fn visit_enum_decl(&mut self, node: &EnumDecl) {
        for variant in &node.variants {
            self.check(&variant.name);
        }
        walk_enum_decl(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalVar(lv) = node
            && lv.is_const
        {
            self.check_declarators(&lv.declarators);
        }
        walk_stmt(self, node);
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
        ConstantIdentifierNames.analyze(&program, &ctx).len()
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
