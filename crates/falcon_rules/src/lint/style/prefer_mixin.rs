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

use falcon_analyze::resolve::TypeKind;
use falcon_analyze::{
    AnalyzeContext, ResolvedType, Rule, RulePreparation, SemanticModel, SignatureIndex,
    StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferMixin;

impl Rule for PreferMixin {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
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
impl StructuralRuleVisitor for Collector<'_> {
    fn visit_top_level_decl(&mut self, decl: &TopLevelDecl) -> StructuralTraversal {
        let ctx = self.ctx;
        let model = &self.model;
        let signatures = self.signatures;
        let diags = &mut self.diags;
        let with_clause = match decl {
            TopLevelDecl::Class(node) => &node.with_clause,
            TopLevelDecl::ClassTypeAlias(node) => &node.with_clause,
            TopLevelDecl::MixinClass(node) => &node.with_clause,
            TopLevelDecl::Enum(node) => &node.with_clause,
            _ => return StructuralTraversal::SkipChildren,
        };
        for mixin in with_clause {
            let is_plain_class = match model.resolve_type(mixin) {
                ResolvedType::Interface { identity, .. } => {
                    signatures.type_kind(&identity) == Some(TypeKind::Class)
                }
                _ => false,
            };
            if is_plain_class {
                diags.push(ctx.diagnostic(
                    "prefer-mixin",
                    "Only mixins should be mixed in.",
                    *mixin.span(),
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
    use falcon_analyze::{IdentityIndex, IdentitySource, LibrarySource, TypeIndex};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    /// Mixed-in types resolve through declaration identities across files.
    fn reports(source: &str, imported: &str) -> Vec<String> {
        let path = PathBuf::from("/project/lib/main.dart");
        let imported_path = PathBuf::from("/project/lib/imported.dart");
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
            ..
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let falcon_dart_parser::ParseResult {
            program: dependency,
            errors,
            ..
        } = parse(imported);
        assert!(errors.is_empty(), "{errors:?}");
        let sources = [
            IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            },
            IdentitySource {
                path: &imported_path,
                program: &dependency,
                has_parse_errors: false,
            },
        ];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_resolved_files(
            sources.iter().map(|source| {
                (
                    source.path,
                    LibrarySource {
                        program: source.program,
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        let signatures = falcon_analyze::SignatureIndex::from_project_files(
            &[
                (path.clone(), &program),
                (imported_path.clone(), &dependency),
            ],
            &identities,
            &types,
        );
        let config = FalconConfig::default();
        let context = AnalyzeContext::new(&path, source, &config, &syntax)
            .with_identities(&identities)
            .with_types(&types)
            .with_signatures(&signatures);
        PreferMixin
            .analyze(&program, &context)
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
