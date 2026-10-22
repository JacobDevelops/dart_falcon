//! Flags `==` and `hashCode` overrides in classes not marked `@immutable`.
//!
//! A value-based `==` and `hashCode` on a mutable object is a trap: once the
//! object is stored in a `Set` or used as a `Map` key, mutating it changes its
//! hash code and the collection silently loses track of it. Only classes that
//! are (transitively) annotated with package:meta's `@immutable` should
//! override them.
//! The diagnostic covers the first declaration token after metadata.
//!
//! ```dart
//! class Point {                 // not @immutable
//!   int x;
//!   Point(this.x);
//!   @override
//!   bool operator ==(Object other) => other is Point && other.x == x; // flagged
//!   @override
//!   int get hashCode => x.hashCode; // flagged
//! }
//! ```
//!
//! The annotation may sit on the class itself or on any supertype (superclass,
//! mixin, or interface). Unresolved supertypes leave immutability unknown.
//!
//! Adopted from the official `avoid_equals_and_hash_code_on_mutable_classes`
//! lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, SemanticModel, StructuralRuleVisitor,
    StructuralTraversal, Truth, has_immutable_annotation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::declarations::first_token_after_annotations;

#[derive(Default)]
pub struct AvoidEqualsAndHashCodeOnMutableClasses;

impl Rule for AvoidEqualsAndHashCodeOnMutableClasses {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            program,
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        let ctx = self.ctx;
            let (name, annotations, members, has_supertypes) = match decl {
                TopLevelDecl::Class(class) => (
                    class.name().name(),
                    class.annotations(),
                    class.members(),
                    class.extends().is_some()
                        || !class.with_clause().is_empty()
                        || !class.implements().is_empty(),
                ),
                TopLevelDecl::MixinClass(class) => (
                    class.name().name(),
                    class.annotations(),
                    class.members(),
                    class.extends().is_some()
                        || !class.with_clause().is_empty()
                        || !class.implements().is_empty(),
                ),
                _ => return StructuralTraversal::SkipChildren,
            };
            let immutable = match (ctx.identities(), ctx.signatures()) {
                (Some(identities), Some(signatures)) => {
                    SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types())
                        .resolve_name(name)
                        .map_or(Truth::Unknown, |identity| {
                            signatures.has_immutable_supertype(&identity, false)
                        })
                }
                _ if has_immutable_annotation(annotations, self.program) => Truth::Yes,
                _ if !has_supertypes => Truth::No,
                _ => Truth::Unknown,
            };
            if immutable != Truth::No {
                return StructuralTraversal::SkipChildren;
            }
            for member in members {
                let (member_name, annotations, span) = match member {
                    ClassMember::Operator(op) if op.op() == "==" => ("==", op.annotations(), op.span()),
                    ClassMember::Getter(getter) if getter.name().name() == "hashCode" => {
                        ("hashCode", getter.annotations(), getter.span())
                    }
                    ClassMember::Method(method) if method.name().name() == "hashCode" => {
                        ("hashCode", method.annotations(), method.span())
                    }
                    _ => continue,
                };
                let start = first_token_after_annotations(annotations, span, ctx.tree(), ctx.source());
                self.diags.push(ctx.diagnostic(
            format!(
                "The method `{member_name}` should not be overridden in classes not annotated with `@immutable`."
            ),
            Span::new(
                start,
                ctx.tree()
                    .tokens_in(ctx.source(), Span::new(start, span.end))
                    .next()
                    .map_or(start, |token| token.offset() + token.len()),
            ),
        ));
            }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
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
    fn reports_first_token_after_metadata_not_the_member_body() {
        let source = r"
class Mutable {
  @override
  /* metadata gap */ bool operator ==(Object other) => true;
  @override
  int get hashCode => 0;
}
";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
        let diagnostics =
            AvoidEqualsAndHashCodeOnMutableClasses.analyze(AvoidEqualsAndHashCodeOnMutableClasses::NAME, parsed.tree.program(source), &ctx);
        assert_eq!(diagnostics.len(), 2);
        for (diagnostic, expected) in diagnostics.iter().zip(["bool", "int"]) {
            assert_eq!(diagnostic.span.start, source.find(expected).unwrap());
            assert_eq!(
                &source[diagnostic.span.start..diagnostic.span.end],
                expected
            );
        }
    }
}
