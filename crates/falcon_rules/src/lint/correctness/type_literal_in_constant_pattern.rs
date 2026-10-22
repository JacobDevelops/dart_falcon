//! Don't use constant patterns whose expressions have the core `Type` type.
//!
//! Adopted from the official `type_literal_in_constant_pattern` lint. This
//! includes constant variables and static fields typed as `Type`, not just
//! direct type names. Patterns explicitly wrapped in `const (...)` and patterns
//! matched against `Type` values are excluded.
//! Qualified constants report the complete constant expression, including their
//! import or class prefix but excluding an enclosing null-check pattern's `?`.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, Truth, TypeRelation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct TypeLiteralInConstantPattern;
impl Rule for TypeLiteralInConstantPattern {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diags) }
}
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}
impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_pattern(
        &mut self,
        pattern: Pattern<'a>,
        matched: Option<&ResolvedType>,
        state: &SemanticState<'_>,
    ) {
        let Pattern::Const(constant) = pattern else {
            return;
        };
        let ConstPatternValue::Name(name) = constant.value() else {
            return;
        };
        let Some(first) = name.first() else {
            return;
        };
        // Raw type literals are fine when the matched value is a `Type`.
        if matched.is_some_and(|matched| is_type_subtype(matched, state)) {
            return;
        }
        // `dynamic` is a type-literal keyword, not a resolvable declaration.
        let is_dynamic = name.len() == 1 && first.name() == "dynamic";
        let span = Span::new(
            first.span().start,
            PatternNames::last(&name).unwrap().span().end,
        );
        if is_dynamic
            || matches!(state.infer_constant_name(name),
            ResolvedType::Interface { ref identity, .. }
                if *identity == DeclarationIdentity::sdk("dart:core", "Type"))
        {
            self.diags.push(self.ctx.diagnostic("Use a type pattern or wrap the type literal in `const (...)`.", span));
        }
    }
}

/// Whether `ty` is a subtype of `dart:core`'s `Type`.
fn is_type_subtype(ty: &ResolvedType, state: &SemanticState<'_>) -> bool {
    let type_identity = DeclarationIdentity::sdk("dart:core", "Type");
    match ty {
        ResolvedType::Never => true,
        ResolvedType::Interface {
            identity,
            nullable: false,
            ..
        } => {
            *identity == type_identity
                || state
                    .signatures
                    .relation(ty, TypeRelation::SubtypeOfDeclaration(&type_identity), &state.model)
                    == Truth::Yes
        }
        ResolvedType::TypeParameter {
            bound,
            nullable: false,
            ..
        } => is_type_subtype(bound, state),
        ResolvedType::NonNullTypeParameter { bound, .. } => {
            is_type_subtype(&bound.non_null(), state)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use falcon_analyze::RuleAnalysis;
    use super::*;

    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    #[test]
    fn qualified_constant_patterns_report_the_complete_pattern_span() {
        let source = "class A { static const ty = String; }\nvoid f(Object o, Type type) {\n  if (o case A.ty) {}\n  if (o case const (String)) {}\n  if (type case A.ty) {}\n  if (o case int?) {}\n}";
        let path = Path::new("/project/lib/test.dart");
        let falcon_dart_parser::ParseResult { errors, tree } = parse(source);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        assert!(errors.is_empty(), "{errors:?}");
        let indexes = crate::lint::test_support::TestIndexes::single(path, &parsed, false);
        let config = FalconConfig::default();
        let ctx = indexes.context(path, &config, &parsed);
        let diagnostics = TypeLiteralInConstantPattern.analyze(TypeLiteralInConstantPattern::NAME, program, &ctx);
        let start = source.find("A.ty").unwrap();
        let nullable_start = source.find("int?").unwrap();
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| (diagnostic.span.start, diagnostic.span.end))
                .collect::<Vec<_>>(),
            vec![
                (start, start + "A.ty".len()),
                (nullable_start, nullable_start + 3)
            ],
            "{diagnostics:?}",
        );
    }
}
