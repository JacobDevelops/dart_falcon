//! Flags `with` clauses that mix in a plain class instead of a mixin.
//!
//! Dart 3 only lets `mixin` and `mixin class` declarations be mixed in from
//! other libraries; mixing in an ordinary class is a legacy pattern that also
//! couples the class's constructor-free shape to every user. Declare the
//! reusable behavior as a `mixin`.
//!
//! ```dart
//! class Logger { void log(String m) {} }
//! class Service with Logger {}  // flagged
//!
//! mixin Logging { void log(String m) {} }
//! class Worker with Logging {}  // preferred
//! ```
//!
//! Resolved plain classes are reported regardless of their declaring library.
//!
//! Adopted from the official `prefer_mixin` lint.

use falcon_analyze::TypeKind;
use falcon_analyze::{
    AnalyzeContext, ResolvedTypeView, Rule, RulePreparation, SemanticModel, SignatureIndex,
    StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferMixin;

impl Rule for PreferMixin {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model,
            signatures,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        let ctx = self.ctx;
        let model = &self.model;
        let signatures = self.signatures;
        let diags = &mut self.diags;
        let with_clause = match decl {
            TopLevelDecl::Class(node) => node.with_clause(),
            TopLevelDecl::ClassTypeAlias(node) => node.with_clause(),
            TopLevelDecl::MixinClass(node) => node.with_clause(),
            TopLevelDecl::Enum(node) => node.with_clause(),
            _ => return StructuralTraversal::SkipChildren,
        };
        for mixin in with_clause {
            let is_plain_class = match model.resolve_type(mixin).view() {
                ResolvedTypeView::Interface { identity, .. } => {
                    signatures.type_kind(identity) == Some(TypeKind::Class)
                }
                _ => false,
            };
            if is_plain_class {
                diags.push(ctx.diagnostic(
                    "Only mixins should be mixed in.",
                    mixin.span(),
                ));
            }
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
    use std::path::PathBuf;

    /// Mixed-in types resolve through declaration identities across files.
    fn reports(source: &str, imported: &str) -> Vec<String> {
        let path = PathBuf::from("/project/lib/main.dart");
        let imported_path = PathBuf::from("/project/lib/imported.dart");
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        assert!(errors.is_empty(), "{errors:?}");
        let falcon_dart_parser::ParseResult { tree, errors } = parse(imported);
        let dependency = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: imported.into(),
            tree,
        });
        assert!(errors.is_empty(), "{errors:?}");
        let indexes = crate::lint::test_support::TestIndexes::new(&[(&path, &parsed), (&imported_path, &dependency)]);
        let config = FalconConfig::default();
        let context = indexes.context(&path, &config, &parsed);
        PreferMixin.analyze(PreferMixin::NAME, program, &context)
            .into_iter()
            .map(|diagnostic| {
                source[diagnostic.span.start..diagnostic.span.end]
                    .trim()
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn imported_plain_class_is_reported_by_identity() {
        let imported = "class Logger {}\nmixin Logging {}\nmixin class Tracing {}\n";
        let source =
            "import 'imported.dart';\nclass A with Logger {}\nclass B with Logging, Tracing {}\n";
        assert_eq!(reports(source, imported), ["Logger"]);
    }

    #[test]
    fn prefixed_import_resolves_to_the_declaring_library() {
        let imported = "class Logger {}\n";
        let source =
            "import 'imported.dart' as lib;\nmixin Logger {}\nclass A with lib.Logger, Logger {}\n";
        assert_eq!(reports(source, imported), ["lib.Logger"]);
    }
}
