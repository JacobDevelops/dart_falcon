//! Requires library-targeted metadata to attach to a `library` directive.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, Rule, SemanticModel};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct LibraryAnnotations;

impl Rule for LibraryAnnotations {
    fn name(&self) -> &'static str {
        "library-annotations"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        if program.part_of_directive.is_some() {
            return Vec::new();
        }
        let library_annotations = library_annotation_names(program, ctx);
        let declared_names = declared_names(program, ctx);
        let first_directive = first_non_library_directive(program);
        let mut diags = Vec::new();
        for (span, annotations) in directive_annotations(program) {
            let is_first = first_directive.is_some_and(|start| start == span.start);
            for annotation in annotations {
                if (is_first
                    && is_library_annotation(annotation, program, &library_annotations, ctx))
                    || late_trust_pragma(annotation, &declared_names, ctx)
                {
                    flag(annotation, ctx, &mut diags);
                }
            }
        }
        for declaration in &program.declarations {
            for annotation in declaration_annotations(declaration) {
                if late_trust_pragma(annotation, &declared_names, ctx) {
                    flag(annotation, ctx, &mut diags);
                }
            }
        }
        diags
    }
}

fn first_non_library_directive(program: &Program) -> Option<usize> {
    program
        .part_directives
        .iter()
        .map(|d| d.span.start)
        .chain(program.imports.iter().map(|d| d.span.start))
        .chain(program.exports.iter().map(|d| d.span.start))
        .min()
}

fn directive_annotations(program: &Program) -> Vec<(&Span, &[Annotation])> {
    program
        .part_directives
        .iter()
        .map(|d| (&d.span, d.annotations.as_slice()))
        .chain(
            program
                .imports
                .iter()
                .map(|d| (&d.span, d.annotations.as_slice())),
        )
        .chain(
            program
                .exports
                .iter()
                .map(|d| (&d.span, d.annotations.as_slice())),
        )
        .collect()
}

fn declaration_annotations(decl: &TopLevelDecl) -> &[Annotation] {
    match decl {
        TopLevelDecl::Class(d) => &d.annotations,
        TopLevelDecl::ClassTypeAlias(d) => &d.annotations,
        TopLevelDecl::Mixin(d) => &d.annotations,
        TopLevelDecl::MixinClass(d) => &d.annotations,
        TopLevelDecl::Enum(d) => &d.annotations,
        TopLevelDecl::Extension(d) => &d.annotations,
        TopLevelDecl::ExtensionType(d) => &d.annotations,
        TopLevelDecl::Function(d) => &d.annotations,
        TopLevelDecl::Variable(d) => &d.annotations,
        TopLevelDecl::TypeAlias(d) => &d.annotations,
        TopLevelDecl::Error(_) => &[],
    }
}

fn library_annotation_names(program: &Program, ctx: &AnalyzeContext) -> HashSet<String> {
    if ctx.identities.is_some() && ctx.signatures.is_some() {
        return HashSet::new();
    }
    let mut programs = vec![program];
    if let Some(library) = ctx.library {
        programs.extend(library.siblings().iter().copied());
    }
    let annotation_types = programs
        .iter()
        .flat_map(|program| &program.declarations)
        .filter_map(|decl| match decl {
            TopLevelDecl::Class(class) if class.annotations.iter().any(target_includes_library) => {
                Some(class.name.name.clone())
            }
            _ => None,
        })
        .collect::<HashSet<_>>();
    let mut names = annotation_types.clone();
    for program in programs {
        for decl in &program.declarations {
            let TopLevelDecl::Variable(variable) = decl else {
                continue;
            };
            if !variable.is_const {
                continue;
            }
            for declarator in &variable.declarators {
                if declarator
                    .initializer
                    .as_ref()
                    .and_then(constructed_type)
                    .is_some_and(|name| annotation_types.contains(name))
                {
                    names.insert(declarator.name.name.clone());
                }
            }
        }
    }
    names
}

fn target_includes_library(annotation: &Annotation) -> bool {
    if annotation
        .name
        .last()
        .is_none_or(|name| name.name != "Target")
    {
        return false;
    }
    let mut finder = LibraryTargetFinder(false);
    if let Some(args) = &annotation.args {
        for arg in &args.positional {
            finder.visit_expr(arg);
        }
        for arg in &args.named {
            finder.visit_expr(&arg.value);
        }
    }
    finder.0
}

struct LibraryTargetFinder(bool);

impl Visitor for LibraryTargetFinder {
    fn visit_expr(&mut self, node: &Expr) {
        if matches!(node, Expr::Field { object, field, .. }
            if field.name == "library"
                && matches!(object.as_ref(), Expr::Ident(name) if name.name == "TargetKind"))
        {
            self.0 = true;
        } else {
            walk_expr(self, node);
        }
    }
}

fn constructed_type(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::New { dart_type, .. } => type_name(dart_type),
        Expr::Call { callee, .. } => match callee.as_ref() {
            Expr::Ident(name) => Some(&name.name),
            Expr::Field { object, .. } => match object.as_ref() {
                Expr::Ident(name) => Some(&name.name),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn type_name(ty: &DartType) -> Option<&str> {
    match ty {
        DartType::Named(named) => named.segments.last().map(|name| name.name.as_str()),
        _ => None,
    }
}

fn is_library_annotation(
    annotation: &Annotation,
    program: &Program,
    local_names: &HashSet<String>,
    ctx: &AnalyzeContext,
) -> bool {
    if ctx.identities.is_some() && ctx.signatures.is_some() {
        return canonical_library_annotation(annotation, ctx);
    }
    let Some(last) = annotation.name.last() else {
        return false;
    };
    if local_names.contains(&last.name) {
        return annotation.name.len() == 1;
    }
    const TEST_LIBRARY_NAMES: &[&str] = &["TestOn", "Timeout", "Tags", "OnPlatform"];
    if !TEST_LIBRARY_NAMES.contains(&last.name.as_str()) {
        return false;
    }
    let prefix = (annotation.name.len() > 1).then(|| annotation.name[0].name.as_str());
    program.imports.iter().any(|import| {
        import.uri.value == "package:test/test.dart"
            && import.as_name.as_ref().map(|name| name.name.as_str()) == prefix
            && import_exposes(import, &last.name)
    })
}

fn canonical_library_annotation(annotation: &Annotation, ctx: &AnalyzeContext) -> bool {
    let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
        return false;
    };
    let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
    let segments = annotation
        .name
        .iter()
        .map(|segment| segment.name.clone())
        .collect::<Vec<_>>();
    let annotation_type = model
        .resolve_name(&segments)
        .or_else(|| model.resolve_imported_member(&segments))
        .or_else(|| {
            let value = model.resolve_value(&segments)?;
            signatures.constant_type(&value).cloned()
        });
    annotation_type
        .and_then(|identity| signatures.declaration(&identity))
        .is_some_and(|facts| facts.library_target)
}

fn import_exposes(import: &ImportDirective, name: &str) -> bool {
    if import.combinators.iter().any(|combinator| {
        matches!(combinator, ImportCombinator::Show(names, _) if !names.iter().any(|item| item.name == name))
    }) {
        return false;
    }
    !import.combinators.iter().any(|combinator| {
        matches!(combinator, ImportCombinator::Hide(names, _) if names.iter().any(|item| item.name == name))
    })
}

fn declared_names(program: &Program, ctx: &AnalyzeContext) -> HashSet<String> {
    let own = std::iter::once(program).chain(
        ctx.library
            .into_iter()
            .flat_map(|library| library.siblings().iter().copied()),
    );
    own.flat_map(|program| &program.declarations)
        .flat_map(|decl| match decl {
            TopLevelDecl::Class(decl) => vec![&decl.name.name],
            TopLevelDecl::ClassTypeAlias(decl) => vec![&decl.name.name],
            TopLevelDecl::Mixin(decl) => vec![&decl.name.name],
            TopLevelDecl::MixinClass(decl) => vec![&decl.name.name],
            TopLevelDecl::Enum(decl) => vec![&decl.name.name],
            TopLevelDecl::Extension(decl) => decl.name.iter().map(|name| &name.name).collect(),
            TopLevelDecl::ExtensionType(decl) => vec![&decl.name.name],
            TopLevelDecl::Function(decl) => vec![&decl.name.name],
            TopLevelDecl::Variable(decl) => decl.declarators.iter().map(|d| &d.name.name).collect(),
            TopLevelDecl::TypeAlias(decl) => vec![&decl.name.name],
            TopLevelDecl::Error(_) => vec![],
        })
        .cloned()
        .collect()
}

fn late_trust_pragma(
    annotation: &Annotation,
    declared_names: &HashSet<String>,
    ctx: &AnalyzeContext,
) -> bool {
    let segments = annotation
        .name
        .iter()
        .map(|segment| segment.name.clone())
        .collect::<Vec<_>>();
    let canonical = ctx.identities.is_some_and(|identities| {
        matches!(
            identities.resolve_sdk_member(ctx.file_path, &segments),
            Some(DeclarationIdentity::Sdk { library, name })
                if library == "dart:core" && name == "pragma"
        )
    });
    (canonical
        || (ctx.identities.is_none()
            && annotation.name.len() == 1
            && annotation.name[0].name == "pragma"
            && !declared_names.contains("pragma")))
        && annotation
            .args
            .as_ref()
            .and_then(|args| args.positional.first())
            .is_some_and(|arg| {
                matches!(arg, Expr::StringLit(string) if string.value == "dart2js:late:trust")
            })
}

fn flag(annotation: &Annotation, ctx: &AnalyzeContext, diags: &mut Vec<Diagnostic>) {
    diags.push(Diagnostic::new(
        "library-annotations",
        Severity::Warning,
        "Attach library annotations to a library directive.",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: annotation.span.start,
            end: annotation.span.end,
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    /// Without a resolver the rule falls back to `declared_names` to decide whether
    /// `pragma` is shadowed, so every declarator of a multi-name variable counts.
    #[test]
    fn shadowing_pragma_suppresses_late_trust_without_a_resolver() {
        let source = "var other, pragma;\n@pragma('dart2js:late:trust')\nimport 'a.dart';\n";
        let (program, _) = parse(source);
        let config = FalconConfig::default();
        let path = PathBuf::from("shadowed.dart");
        let ctx = AnalyzeContext::new(&path, source, &config);
        assert!(LibraryAnnotations.analyze(&program, &ctx).is_empty());
    }
}
