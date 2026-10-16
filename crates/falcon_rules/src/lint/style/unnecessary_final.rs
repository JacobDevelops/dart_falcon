//! Don't use `final` for local variables and parameters.
//!
//! Some codebases prefer `var` for locals and plain parameters, reserving
//! `final` for fields and top-level declarations. This flags the `final`
//! keyword on parameters, local variable statements, for-each loop variables
//! of `for` statements (including destructuring loops), pattern variable
//! declarations, and variable patterns (`case final x`). C-style `for`
//! initializers are not variable declaration statements and are left alone;
//! neither are collection `for` elements (`[for (final x in xs) x]`), which the
//! analyzer does not visit.
//! Parameters are checked only before Dart 3.13: with primary constructors a
//! parameter's `final` declares a field instead.
//! Local keyword spans come from the AST, not matching words in documentation
//! or annotation arguments.
//!
//! ```dart
//! void f(final int x) { // flagged
//!   final y = x + 1; // flagged
//!   var z = y; // ok
//! }
//! ```
//!
//! Adopted from the official `unnecessary_final` lint.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Feature;
use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;
use falcon_syntax::visitor::{
    Visitor, walk_formal_param, walk_pattern, walk_setter_decl, walk_stmt,
};

const RULE: &str = "unnecessary-final";
const MESSAGE: &str = "Local variables should not be marked as 'final'.";

#[derive(Default)]
pub struct UnnecessaryFinal;

impl Rule for UnnecessaryFinal {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            ctx,
            // With primary constructors `final` on a parameter declares a
            // field, so no parameter's `final` is unnecessary.
            check_params: !ctx
                .language_version()
                .supports(Feature::PrimaryConstructors),
            reported: HashSet::new(),
            diagnostics: Vec::new(),
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    check_params: bool,
    /// Keyword offsets already reported: every variable of `final (a, b) = r;`
    /// shares the one keyword.
    reported: HashSet<usize>,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    fn report_final_in(&mut self, start: usize, end: usize) {
        if let Some(token) = self
            .ctx
            .syntax
            .keywords_in(Span::new(start, end), TokenKind::Final)
            .next()
        {
            self.report_final(Span::new(token.offset(), token.offset() + token.len()));
        }
    }

    fn report_final(&mut self, keyword: Span) {
        if self.reported.insert(keyword.start) {
            self.diagnostics
                .push(self.ctx.diagnostic(RULE, MESSAGE, keyword));
        }
    }

    fn check_for_init(&mut self, init: &ForInit, start: usize) {
        match init {
            ForInit::ForIn {
                keyword: VarKeyword::Final,
                declares_variable: true,
                var_type,
                name,
                ..
            } => {
                let anchor = var_type
                    .as_ref()
                    .map_or(name.span.start, |ty| ty.span().start);
                self.report_final_in(start, anchor);
            }
            ForInit::PatternForIn {
                keyword: VarKeyword::Final,
                pattern,
                ..
            } => {
                self.report_final_in(start, pattern.span().start);
            }
            ForInit::ForIn { .. }
            | ForInit::PatternForIn { .. }
            | ForInit::VarDecl(_)
            | ForInit::Exprs(_) => {}
        }
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_formal_param(&mut self, node: &FormalParam) {
        if self.check_params && node.is_final {
            let start = node
                .annotations
                .last()
                .map_or(node.span.start, |annotation| annotation.span.end);
            self.report_final_in(start, node.name.span.start);
        }
        walk_formal_param(self, node);
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        let start = node
            .param_annotations
            .last()
            .map_or(node.name.span.end, |annotation| annotation.span.end);
        if self.check_params {
            self.report_final_in(start, node.param.span.start);
        }
        walk_setter_decl(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalVar(declaration) = node
            && declaration.keyword == VarKeyword::Final
            && let Some(keyword) = declaration.keyword_span
        {
            self.report_final(keyword);
        } else if let Stmt::PatternDecl(declaration) = node
            && declaration.keyword == VarKeyword::Final
        {
            self.report_final(declaration.keyword_span);
        } else if let Stmt::For(statement) = node
            && let Some(init) = &statement.header.init
        {
            self.check_for_init(init, statement.span.start);
        }
        walk_stmt(self, node);
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        if let Pattern::Variable { name, span, .. } = node
            && let Some(keyword) = self
                .ctx
                .syntax
                .keywords_in(Span::new(span.start, name.span.start), TokenKind::Final)
                .next()
            && keyword.offset() == span.start
        {
            self.report_final(Span::new(
                keyword.offset(),
                keyword.offset() + keyword.len(),
            ));
        }
        walk_pattern(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    #[test]
    fn local_final_span_ignores_keyword_words_in_documentation_and_metadata() {
        let source = r#"void f() {
  /// final belongs to this documentation, not the declaration keyword.
  @pragma('final')
  late final value = 1;
  /// final belongs to this pattern documentation.
  @pragma('final')
  final (first, second) = (value, 2);
  print(first + second);
}"#;
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config, &syntax);
        let diagnostics = UnnecessaryFinal.analyze(&program, &ctx);
        assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
        let local = source.find("final value").unwrap();
        let pattern = source.find("final (first").unwrap();
        assert_eq!(diagnostics[0].span, Span::new(local, local + 5).into());
        assert_eq!(diagnostics[1].span, Span::new(pattern, pattern + 5).into());
    }
}
