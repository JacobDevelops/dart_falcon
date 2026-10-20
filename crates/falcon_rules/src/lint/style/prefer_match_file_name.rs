//! Match a file's sole class or enum name to its snake-case basename.
//!
//! Other declarations may coexist with the type. Files with multiple classes or
//! enums, generated inner-dot suffixes, and part-of files are left alone. Naming
//! convention violations themselves belong to the existing naming rules.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferMatchFileName;

impl Rule for PreferMatchFileName {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let stem = ctx
            .file_path()
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".dart"));
        let Some(stem) = stem.filter(|stem| !stem.is_empty() && !stem.contains('.')) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        if crate::lint::declarations::has_test_file_suffix(ctx.file_path())
            || crate::lint::declarations::in_test_directory(ctx.file_path())
        {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            stem,
            candidate: None,
            count: 0,
            is_part: false,
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    stem: &'a str,
    candidate: Option<Identifier<'a>>,
    count: usize,
    is_part: bool,
}

/// Preserve acronym runs (`HTTPClient` -> `http_client`) and digit suffixes (`Point3D` -> `point3d`).
fn file_stem(name: &str) -> String {
    let name = name.trim_start_matches('_');
    let mut stem = String::with_capacity(name.len());
    let mut chars = name.chars().peekable();
    let mut previous = None::<char>;
    while let Some(character) = chars.next() {
        if character.is_uppercase()
            && previous.is_some_and(|previous| {
                previous.is_lowercase()
                    || ((previous.is_ascii_digit() || previous.is_uppercase())
                        && chars.peek().is_some_and(|next| next.is_lowercase()))
            })
        {
            stem.push('_');
        }
        stem.extend(character.to_lowercase());
        previous = Some(character);
    }
    stem
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let name = match declaration {
            TopLevelDecl::Class(class) => Some(class.name()),
            TopLevelDecl::Enum(enumeration) => Some(enumeration.name()),
            TopLevelDecl::Mixin(_)
            | TopLevelDecl::MixinClass(_)
            | TopLevelDecl::Extension(_)
            | TopLevelDecl::ExtensionType(_)
            | TopLevelDecl::ClassTypeAlias(_) => {
                self.is_part = true;
                None
            }
            _ => None,
        };
        if let Some(name) = name {
            self.count += 1;
            self.candidate = Some(name);
        }
        StructuralTraversal::SkipChildren
    }

    fn visit_part_of_directive(&mut self, _node: PartOfDirective<'a>) -> StructuralTraversal {
        self.is_part = true;
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        if self.is_part || self.count != 1 {
            return Vec::new();
        }
        let Some(name) = self.candidate else {
            return Vec::new();
        };
        if !crate::lint::naming::is_upper_camel_case(name.name()) {
            return Vec::new();
        }
        if name.name().chars().filter(|character| *character != '_').flat_map(char::to_lowercase)
            .eq(self.stem.chars().filter(|character| *character != '_').flat_map(char::to_lowercase))
        {
            return Vec::new();
        }
        let expected = file_stem(name.name());
        vec![self.ctx.diagnostic(
            "prefer-match-file-name",
            format!(
                "The sole class or enum '{}' does not match this filename. Rename the file to '{}.dart' or rename the declaration to match the file.",
                name.name(), expected,
            ),
            name.span(),
        )]
    }
}
}

#[cfg(test)]
mod tests {
    use super::file_stem;

    #[test]
    fn preserves_digit_suffixes_and_word_boundaries() {
        for (name, expected) in [
            ("Point3D", "point3d"),
            ("Base64Encoder", "base64_encoder"),
            ("HTTPClient", "http_client"),
            ("Vector2DList", "vector2d_list"),
        ] {
            assert_eq!(file_stem(name), expected, "{name}");
        }
    }
}
