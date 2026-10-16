//! Don't use constant patterns whose expressions have the core `Type` type.
//!
//! Adopted from the official `type_literal_in_constant_pattern` lint. This
//! includes constant variables and static fields typed as `Type`, not just
//! direct type names. Patterns explicitly wrapped in `const (...)` and patterns
//! matched against `Type` values are excluded.
//! Qualified constants report the complete constant expression, including their
//! import or class prefix but excluding an enclosing null-check pattern's `?`.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct TypeLiteralInConstantPattern;
impl Rule for TypeLiteralInConstantPattern {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Semantic(Box::new(collector))
    }
}
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}
impl SemanticRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_pattern(
        &mut self,
        pattern: &Pattern,
        matched: Option<&ResolvedType>,
        state: &SemanticState<'_>,
    ) {
        let Pattern::Const(constant) = pattern else {
            return;
        };
        let ConstPatternValue::Name(name) = &constant.value else {
            return;
        };
        let Some((first, rest)) = name.split_first() else {
            return;
        };
        // Raw type literals are fine when the matched value is a `Type`.
        if matched.is_some_and(|matched| is_type_subtype(matched, state)) {
            return;
        }
        // `dynamic` is a type-literal keyword, not a resolvable declaration.
        let is_dynamic = rest.is_empty() && first.name == "dynamic";
        let expression = rest
            .iter()
            .fold(Expr::Ident(first.clone()), |object, field| {
                let span = Span::new(object.span().start, field.span.end);
                Expr::Field {
                    object: Box::new(object),
                    field: field.clone(),
                    is_null_safe: false,
                    span,
                }
            });
        if is_dynamic
            || matches!(state.infer(&expression),
            ResolvedType::Interface { ref identity, .. }
                if *identity == DeclarationIdentity::sdk("dart:core", "Type"))
        {
            self.diags.push(self.ctx.diagnostic(
                "type-literal-in-constant-pattern",
                "Use a type pattern or wrap the type literal in 'const (...)'.",
                *expression.span(),
            ));
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
                    .is_subtype_of(ty, &type_identity, &state.model)
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
    use super::*;
    use falcon_analyze::{
        IdentityIndex, IdentitySource, RuleAnalysis, SemanticModel, SignatureIndex,
    };
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    #[test]
    fn qualified_constant_patterns_report_the_complete_pattern_span() {
        let source = "class A { static const ty = String; }\nvoid f(Object o, Type type) {\n  if (o case A.ty) {}\n  if (o case const (String)) {}\n  if (type case A.ty) {}\n  if (o case int?) {}\n}";
        let path = Path::new("/project/lib/test.dart");
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let model = SemanticModel::new(path, &identities, None);
        let signatures = SignatureIndex::from_program(&program, &model);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, source, &config, &syntax)
            .with_identities(&identities)
            .with_signatures(&signatures);
        let diagnostics = TypeLiteralInConstantPattern.analyze(&program, &ctx);
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
