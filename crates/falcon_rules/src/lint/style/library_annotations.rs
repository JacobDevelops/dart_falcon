//! Requires library-targeted metadata to attach to a `library` directive.

use std::collections::HashSet;

use falcon_analyze::{
    AnalyzeContext, QualifiedName, Rule, RulePreparation, SemanticModel, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{Visitor, walk_expr};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct LibraryAnnotations;

impl Rule for LibraryAnnotations {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if program.part_of_directive().is_some() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let library_annotations = library_annotation_names(program, ctx);
        let declared_names = declared_names(program, ctx);
        let first_directive = first_non_library_directive(program);
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            program,
            library_annotations,
            declared_names,
            first_directive,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    program: Program<'a>,
    library_annotations: HashSet<&'a str>,
    declared_names: HashSet<&'a str>,
    first_directive: Option<usize>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Collector<'a> {
    fn directive(&mut self, span: Span, annotations: AstChildren<'a, Annotation<'a>>) {
        let is_first = self.first_directive == Some(span.start);
        for annotation in annotations {
            if (is_first
                && is_library_annotation(
                    annotation,
                    self.program,
                    &self.library_annotations,
                    self.ctx,
                ))
                || late_trust_pragma(annotation, &self.declared_names, self.ctx)
            {
                self.diagnostics.push(self.ctx.diagnostic(
                    "library-annotations",
                    "Attach library annotations to a library directive.",
                    annotation.span(),
                ));
            }
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_part_directive(&mut self, node: PartDirective<'a>) -> StructuralTraversal {
        self.directive(node.span(), node.annotations());
        StructuralTraversal::SkipChildren
    }

    fn visit_import(&mut self, node: ImportDirective<'a>) -> StructuralTraversal {
        self.directive(node.span(), node.annotations());
        StructuralTraversal::SkipChildren
    }

    fn visit_export(&mut self, node: ExportDirective<'a>) -> StructuralTraversal {
        self.directive(node.span(), node.annotations());
        StructuralTraversal::SkipChildren
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        for annotation in declaration_annotations(node) {
            if late_trust_pragma(annotation, &self.declared_names, self.ctx) {
                self.diagnostics.push(self.ctx.diagnostic(
                    "library-annotations",
                    "Attach library annotations to a library directive.",
                    annotation.span(),
                ));
            }
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}

fn first_non_library_directive(program: Program<'_>) -> Option<usize> {
    program
        .part_directives()
        .map(|d| d.span().start)
        .chain(program.imports().map(|d| d.span().start))
        .chain(program.exports().map(|d| d.span().start))
        .min()
}

fn declaration_annotations<'a>(decl: TopLevelDecl<'a>) -> impl Iterator<Item = Annotation<'a>> {
    AstChildren::new(decl.syntax()).filter(move |_| !matches!(decl, TopLevelDecl::Error(_)))
}

fn library_annotation_names<'a>(
    program: Program<'a>,
    ctx: &AnalyzeContext<'a>,
) -> HashSet<&'a str> {
    if ctx.identities().is_some() && ctx.signatures().is_some() {
        return HashSet::new();
    }
    let mut programs = vec![program];
    if let Some(library) = ctx.library() {
        programs.extend(library.siblings().iter().copied());
    }
    let annotation_types = programs
        .iter()
        .flat_map(|program| program.declarations())
        .filter_map(|decl| match decl {
            TopLevelDecl::Class(class) if class.annotations().any(target_includes_library) => {
                Some(class.name().name())
            }
            _ => None,
        })
        .collect::<HashSet<_>>();
    let mut names = annotation_types.clone();
    for program in programs {
        for decl in program.declarations() {
            let TopLevelDecl::Variable(variable) = decl else {
                continue;
            };
            if variable.keyword() != VarKeyword::Const {
                continue;
            }
            for declarator in variable.declarators() {
                if declarator
                    .initializer()
                    .and_then(constructed_type)
                    .is_some_and(|name| annotation_types.contains(name))
                {
                    names.insert(declarator.name().name());
                }
            }
        }
    }
    names
}

fn target_includes_library(annotation: Annotation<'_>) -> bool {
    if annotation
        .name()
        .next_back()
        .is_none_or(|name| name.name() != "Target")
    {
        return false;
    }
    let mut finder = LibraryTargetFinder(false);
    if let Some(args) = annotation.args() {
        for arg in args.positional() {
            finder.visit_expr(arg);
        }
        for arg in args.named() {
            finder.visit_expr(arg.value());
        }
    }
    finder.0
}

struct LibraryTargetFinder(bool);

impl<'a> Visitor<'a> for LibraryTargetFinder {
    fn visit_expr(&mut self, node: Expr<'a>) {
        if matches!(node, Expr::Field(field)
            if field.field().name() == "library"
                && matches!(field.object(), Expr::Ident(name) if name.name() == "TargetKind"))
        {
            self.0 = true;
        } else {
            walk_expr(self, node);
        }
    }
}

fn constructed_type<'a>(expr: Expr<'a>) -> Option<&'a str> {
    match expr.unparenthesized() {
        Expr::New(new) => type_name(new.dart_type()),
        Expr::Call(call) => match call.callee().unparenthesized() {
            Expr::Ident(name) => Some(name.name()),
            Expr::Field(field) => match field.object().unparenthesized() {
                Expr::Ident(name) => Some(name.name()),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn type_name<'a>(ty: DartType<'a>) -> Option<&'a str> {
    match ty {
        DartType::Named(named) => named.segments().next_back().map(|name| name.name()),
        _ => None,
    }
}

fn is_library_annotation(
    annotation: Annotation<'_>,
    program: Program<'_>,
    local_names: &HashSet<&str>,
    ctx: &AnalyzeContext,
) -> bool {
    if ctx.identities().is_some() && ctx.signatures().is_some() {
        return canonical_library_annotation(annotation, ctx);
    }
    let Some(last) = annotation.name().next_back() else {
        return false;
    };
    if local_names.contains(last.name()) {
        return annotation.name().count() == 1;
    }
    const TEST_LIBRARY_NAMES: &[&str] = &["TestOn", "Timeout", "Tags", "OnPlatform"];
    if !TEST_LIBRARY_NAMES.contains(&last.name()) {
        return false;
    }
    let prefix = (annotation.name().count() > 1).then(|| annotation.name().next().unwrap().name());
    program.imports().any(|import| {
        import.uri().value() == "package:test/test.dart"
            && import.as_name().map(|name| name.name()) == prefix
            && import_exposes(import, last.name())
    })
}

fn canonical_library_annotation(annotation: Annotation<'_>, ctx: &AnalyzeContext) -> bool {
    let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
        return false;
    };
    let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
    let Some(name) = QualifiedName::from_identifiers(annotation.name()) else {
        return false;
    };
    let annotation_type = model
        .resolve_name(name)
        .or_else(|| model.resolve_imported_member(name))
        .or_else(|| {
            let value = model.resolve_value(name)?;
            signatures.constant_type(&value).cloned()
        });
    annotation_type
        .and_then(|identity| signatures.declaration(&identity))
        .is_some_and(|facts| facts.library_target)
}

fn import_exposes(import: ImportDirective<'_>, name: &str) -> bool {
    if import.combinators().any(|combinator| {
        matches!(combinator, ImportCombinator::Show(names) if !names.names().any(|item| item.name() == name))
    }) {
        return false;
    }
    !import.combinators().any(|combinator| {
        matches!(combinator, ImportCombinator::Hide(names) if names.names().any(|item| item.name() == name))
    })
}

fn declared_names<'a>(program: Program<'a>, ctx: &AnalyzeContext<'a>) -> HashSet<&'a str> {
    let programs = std::iter::once(program).chain(
        ctx.library()
            .into_iter()
            .flat_map(|library| library.siblings().iter().copied()),
    );
    let mut names = HashSet::new();
    for decl in programs.flat_map(|program| program.declarations()) {
        let name = match decl {
            TopLevelDecl::Class(decl) => decl.name(),
            TopLevelDecl::ClassTypeAlias(decl) => decl.name(),
            TopLevelDecl::Mixin(decl) => decl.name(),
            TopLevelDecl::MixinClass(decl) => decl.name(),
            TopLevelDecl::Enum(decl) => decl.name(),
            TopLevelDecl::Extension(decl) => {
                if let Some(name) = decl.name() {
                    names.insert(name.name());
                }
                continue;
            }
            TopLevelDecl::ExtensionType(decl) => decl.name(),
            TopLevelDecl::Function(decl) => decl.name(),
            TopLevelDecl::Variable(decl) => {
                names.extend(decl.declarators().map(|d| d.name().name()));
                continue;
            }
            TopLevelDecl::TypeAlias(decl) => decl.name(),
            TopLevelDecl::Error(_) => continue,
        };
        names.insert(name.name());
    }
    names
}

fn late_trust_pragma(
    annotation: Annotation<'_>,
    declared_names: &HashSet<&str>,
    ctx: &AnalyzeContext,
) -> bool {
    let canonical = ctx.identities().is_some_and(|identities| {
        QualifiedName::from_identifiers(annotation.name())
            .and_then(|name| {
                SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types())
                    .resolve_sdk_member(name)
            })
            .is_some_and(|identity| identity.is_sdk("dart:core", "pragma"))
    });
    (canonical
        || (ctx.identities().is_none()
            && annotation.name().count() == 1
            && annotation.name().next().unwrap().name() == "pragma"
            && !declared_names.contains("pragma")))
        && annotation
            .args()
            .and_then(|args| args.positional().next())
            .is_some_and(|arg| {
                matches!(arg.unparenthesized(), Expr::StringLit(string) if string.value() == "dart2js:late:trust")
            })
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    /// Without a resolver the rule falls back to `declared_names` to decide whether
    /// `pragma` is shadowed, so every declarator of a multi-name variable counts.
    #[test]
    fn shadowing_pragma_suppresses_late_trust_without_a_resolver() {
        let source = "var other, pragma;\n@pragma('dart2js:late:trust')\nimport 'a.dart';\n";
        let parsed = parse(source);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let config = FalconConfig::default();
        let path = PathBuf::from("shadowed.dart");
        let ctx = AnalyzeContext::new(&path, &config, &parsed);
        assert!(
            LibraryAnnotations
                .analyze(parsed.tree.program(source), &ctx)
                .is_empty()
        );
    }
}
