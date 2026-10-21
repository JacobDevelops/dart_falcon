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
//! Local keyword spans come from the syntax tree, not matching words in documentation
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

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Feature;
use falcon_syntax::cst::*;
use falcon_syntax::token::TokenKind;

const MESSAGE: &str = "Local variables should not be marked as 'final'.";

#[derive(Default)]
pub struct UnnecessaryFinal;

impl Rule for UnnecessaryFinal {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            ctx,
            // With primary constructors `final` on a parameter declares a
            // field, so no parameter's `final` is unnecessary.
            check_params: !ctx
                .language_version()
                .supports(Feature::PrimaryConstructors),
            reported: HashSet::new(),
            diagnostics: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
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
            .tree()
            .keywords_in(self.ctx.source(), Span::new(start, end), TokenKind::Final)
            .next()
        {
            self.report_final(Span::new(token.offset(), token.offset() + token.len()));
        }
    }

    fn report_final(&mut self, keyword: Span) {
        if self.reported.insert(keyword.start) {
            self.diagnostics
                .push(self.ctx.diagnostic(MESSAGE, keyword));
        }
    }

    fn check_for_init(&mut self, init: ForInit<'_>, start: usize) {
        match init {
            ForInit::ForIn(init)
                if init.keyword() == VarKeyword::Final && init.declares_variable() =>
            {
                let anchor = init
                    .var_type()
                    .map_or(init.name().span().start, |ty| ty.span().start);
                self.report_final_in(start, anchor);
            }
            ForInit::PatternForIn(init) if init.keyword() == VarKeyword::Final => {
                self.report_final_in(start, init.pattern().span().start);
            }
            _ => {}
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'_, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }

    fn visit_formal_param(&mut self, node: FormalParam<'a>) -> StructuralTraversal {
        if self.check_params && node.is_final() {
            let start = node
                .annotations()
                .last()
                .map_or(node.span().start, |annotation| annotation.span().end);
            self.report_final_in(start, node.name().span().start);
        }
        StructuralTraversal::Continue
    }

    fn visit_setter_decl(&mut self, node: SetterDecl<'a>) -> StructuralTraversal {
        let start = node
            .param_annotations()
            .last()
            .map_or(node.name().span().end, |annotation| annotation.span().end);
        if self.check_params {
            self.report_final_in(start, node.param().span().start);
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalVar(declaration) = node
            && declaration.keyword() == VarKeyword::Final
            && let Some(keyword) = declaration.keyword_span()
        {
            self.report_final(keyword);
        } else if let Stmt::PatternDecl(declaration) = node
            && declaration.keyword() == VarKeyword::Final
        {
            self.report_final(declaration.keyword_span());
        } else if let Stmt::For(statement) = node
            && let Some(init) = statement.header().init()
        {
            self.check_for_init(init, statement.span().start);
        }
        StructuralTraversal::Continue
    }

    fn visit_pattern(&mut self, node: Pattern<'a>) -> StructuralTraversal {
        if let Pattern::Variable(variable) = node
            && let Some(keyword) = self
                .ctx
                .tree()
                .keywords_in(self.ctx.source(), Span::new(variable.span().start, variable.name().span().start), TokenKind::Final)
                .next()
            && keyword.offset() == variable.span().start
        {
            self.report_final(Span::new(
                keyword.offset(),
                keyword.offset() + keyword.len(),
            ));
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
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let program = parsed.program();
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
        let diagnostics = UnnecessaryFinal.analyze(UnnecessaryFinal::NAME, program, &ctx);
        assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
        let local = source.find("final value").unwrap();
        let pattern = source.find("final (first").unwrap();
        assert_eq!(diagnostics[0].span, Span::new(local, local + 5).into());
        assert_eq!(diagnostics[1].span, Span::new(pattern, pattern + 5).into());
    }
}
