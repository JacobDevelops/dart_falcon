//! Documents public declarations and members in package libraries.
//!
//! Adopted from the official `public_member_api_docs` lint.
//! Private declarations, top-level functions named `main`, and overriding members are
//! exempt; like the analyzer, `package:meta`'s `@internal` exempts a top-level
//! declaration with its members, but not a member on its own. Ordinary constructors
//! of enums and of sealed, `abstract final` and `abstract interface` classes are exempt.
//! A doc comment
//! before, between or after the metadata documents the declaration, including
//! the analyzer's empty `/**/` documentation. `@override` alone does not exempt
//! a member. Fields and getters override inherited getters, methods and fields
//! (not setters);
//! setters override inherited setters and are checked only when their paired
//! getter is reported. Extension-type
//! representation fields are not field declarations visited by the upstream
//! rule.
//! A class primary constructor is checked separately from its type; header
//! constructors are reported at their name without closed-type exemptions,
//! and implementations at `this`. Header parameters are not field declarations.
//! An override exemption requires a combined inherited member, not merely a
//! same-named declaration among incompatible inherited interface signatures.

use falcon_analyze::{AnalyzeContext, MemberKind, Rule, SemanticModel};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::{TokenKind, ast::*};

use crate::lint::declarations::has_meta_annotation;

#[derive(Default)]
pub struct PublicMemberApiDocs;

pub(crate) struct ApiDeclaration<'a> {
    pub name: &'a str,
    pub span: Span,
    pub start: usize,
    pub annotations: &'a [Annotation],
    pub owner: Option<&'a str>,
    pub kind: Option<MemberKind>,
    pub is_static: bool,
    pub is_top_level_function: bool,
    /// Constructors of enums and of sealed, `abstract final` and `abstract
    /// interface` classes are not public API.
    pub exempt_constructor: bool,
    /// The declaration's own `@internal` for top-level declarations, the
    /// enclosing declaration's `@internal` for members.
    pub internal: bool,
}

pub(crate) fn documented(tokens: &[falcon_syntax::Token], start: usize) -> bool {
    let end = tokens.partition_point(|t| t.offset() < start);
    tokens[..end]
        .iter()
        .rev()
        .take_while(|t| t.is_trivia())
        // The lexer leaves `/**/` as ordinary trivia; analyzer still attaches
        // that empty block as a declaration's documentation comment.
        .any(|t| {
            t.kind == TokenKind::DocComment || (t.kind == TokenKind::BlockComment && t.len() == 4)
        })
}

impl ApiDeclaration<'_> {
    /// Whether a doc comment precedes the declaration, its metadata, or any
    /// annotation within it.
    pub(crate) fn documented(&self, tokens: &[falcon_syntax::Token]) -> bool {
        let Some(last) = self.annotations.last() else {
            return documented(tokens, self.start);
        };
        let after = tokens.partition_point(|t| t.offset() < last.span.end);
        let keyword = tokens[after..]
            .iter()
            .find(|t| !t.is_trivia())
            .map_or(last.span.end, |t| t.offset());
        documented(tokens, self.start)
            || self
                .annotations
                .iter()
                .any(|a| documented(tokens, a.span.start))
            || documented(tokens, keyword)
    }
}

pub(crate) fn api_declarations<'a>(
    program: &'a Program,
    syntax: &falcon_syntax::SyntaxStore,
    model: Option<&SemanticModel<'_>>,
) -> Vec<ApiDeclaration<'a>> {
    let is_internal = |annotations: &[Annotation]| {
        model.is_some_and(|model| has_meta_annotation(annotations, "internal", model))
    };
    let mut result = Vec::new();
    let mut candidates = Vec::new();
    for decl in &program.declarations {
        let (name, annotations, members, closed) = match decl {
            TopLevelDecl::Class(n) => (
                Some(&n.name),
                n.annotations.as_slice(),
                n.members.as_slice(),
                match n.modifiers.restriction {
                    Some(ClassRestriction::Sealed) => true,
                    Some(ClassRestriction::Final | ClassRestriction::Interface) => {
                        n.modifiers.is_abstract
                    }
                    _ => false,
                },
            ),
            TopLevelDecl::ClassTypeAlias(n) => {
                (Some(&n.name), n.annotations.as_slice(), &[][..], false)
            }
            TopLevelDecl::Mixin(n) => (
                Some(&n.name),
                n.annotations.as_slice(),
                n.members.as_slice(),
                false,
            ),
            TopLevelDecl::MixinClass(n) => (
                Some(&n.name),
                n.annotations.as_slice(),
                n.members.as_slice(),
                false,
            ),
            TopLevelDecl::Enum(n) => (
                Some(&n.name),
                n.annotations.as_slice(),
                n.members.as_slice(),
                true,
            ),
            TopLevelDecl::Extension(n) => (
                n.name.as_ref(),
                n.annotations.as_slice(),
                n.members.as_slice(),
                false,
            ),
            TopLevelDecl::ExtensionType(n) => (
                Some(&n.name),
                n.annotations.as_slice(),
                n.members.as_slice(),
                false,
            ),
            TopLevelDecl::Function(n) => (Some(&n.name), n.annotations.as_slice(), &[][..], false),
            TopLevelDecl::TypeAlias(n) => (Some(&n.name), n.annotations.as_slice(), &[][..], false),
            TopLevelDecl::Variable(n) => {
                for v in &n.declarators {
                    result.push(ApiDeclaration {
                        internal: is_internal(&n.annotations),
                        ..candidate(&v.name, n.span.start, &n.annotations, None, None, true)
                    });
                }
                continue;
            }
            TopLevelDecl::Error(_) => continue,
        };
        let Some(name) = name.filter(|n| !n.name.starts_with('_')) else {
            continue;
        };
        let mut top = candidate(name, decl.span().start, annotations, None, None, true);
        top.internal = is_internal(annotations);
        if let TopLevelDecl::Function(n) = decl {
            top.is_top_level_function = true;
            top.kind = match n.kind {
                FunctionKind::Getter => Some(MemberKind::Getter),
                FunctionKind::Setter(_) => Some(MemberKind::Setter),
                FunctionKind::Function(_) => None,
            };
        }
        let internal = top.internal;
        result.push(top);
        let elements = match decl {
            TopLevelDecl::Class(n) => Some(n.element_members()),
            TopLevelDecl::MixinClass(n) => Some(n.element_members()),
            _ => None,
        };
        if let Some(elements) = elements
            && let Some(primary) = elements.primary
        {
            let constructor = elements.constructors().find(|c| c.is_primary).unwrap();
            let body = members.iter().find_map(|member| match member {
                ClassMember::PrimaryConstructorBody(body) => Some(body),
                _ => None,
            });
            let mut c = candidate(
                primary.constructor_name.as_ref().unwrap_or(name),
                body.map_or(primary.span.start, |body| body.span.start),
                constructor.annotations,
                Some(&name.name),
                None,
                true,
            );
            if let Some(body) = body {
                c.span = body.this_span;
                c.internal = internal;
            }
            result.push(c);
        }
        if let TopLevelDecl::Enum(n) = decl {
            for v in &n.variants {
                let mut c = candidate(
                    &v.name,
                    v.span.start,
                    &v.annotations,
                    Some(&name.name),
                    None,
                    true,
                );
                c.internal = internal;
                result.push(c);
            }
        }
        for member in members {
            candidates.clear();
            match member {
                ClassMember::Field(n) => {
                    for v in &n.declarators {
                        candidates.push(candidate(
                            &v.name,
                            n.span.start,
                            &n.annotations,
                            Some(&name.name),
                            Some(MemberKind::field(n, v)),
                            n.is_static,
                        ));
                    }
                }
                ClassMember::Method(n) => candidates.push(candidate(
                    &n.name,
                    n.span.start,
                    &n.annotations,
                    Some(&name.name),
                    Some(MemberKind::Method),
                    n.is_static,
                )),
                ClassMember::Getter(n) => candidates.push(candidate(
                    &n.name,
                    n.span.start,
                    &n.annotations,
                    Some(&name.name),
                    Some(MemberKind::Getter),
                    n.is_static,
                )),
                ClassMember::Setter(n) => candidates.push(candidate(
                    &n.name,
                    n.span.start,
                    &n.annotations,
                    Some(&name.name),
                    Some(MemberKind::Setter),
                    n.is_static,
                )),
                ClassMember::Constructor(n) => {
                    let mut c = candidate(
                        n.constructor_name.as_ref().unwrap_or(&n.name),
                        n.span.start,
                        &n.annotations,
                        Some(&name.name),
                        None,
                        true,
                    );
                    c.exempt_constructor = closed;
                    candidates.push(c);
                }
                ClassMember::Operator(n) => candidates.push(ApiDeclaration {
                    name: n.lookup_name(),
                    span: crate::lint::declarations::operator_token_span(n, syntax),
                    start: n.annotations.first().map_or(n.span.start, |a| a.span.start),
                    annotations: &n.annotations,
                    owner: Some(&name.name),
                    kind: Some(MemberKind::Operator),
                    is_static: false,
                    is_top_level_function: false,
                    exempt_constructor: false,
                    internal,
                }),
                ClassMember::PrimaryConstructorBody(_) | ClassMember::Error(_) => {}
            }
            for mut c in candidates.drain(..) {
                c.internal = internal;
                if matches!(decl, TopLevelDecl::Extension(_)) {
                    c.is_static = true;
                }
                result.push(c);
            }
        }
    }
    result
}

fn candidate<'a>(
    name: &'a Identifier,
    start: usize,
    annotations: &'a [Annotation],
    owner: Option<&'a str>,
    kind: Option<MemberKind>,
    is_static: bool,
) -> ApiDeclaration<'a> {
    ApiDeclaration {
        name: &name.name,
        span: name.span,
        start: annotations
            .first()
            .map_or(start, |a| a.span.start.min(start)),
        annotations,
        owner,
        kind,
        is_static,
        is_top_level_function: false,
        exempt_constructor: false,
        internal: false,
    }
}

impl Rule for PublicMemberApiDocs {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        if crate::cross_file::lib_dir_package(ctx.file_path).is_none() {
            return Vec::new();
        }
        let model = ctx
            .identities
            .map(|identities| SemanticModel::new(ctx.file_path, identities, ctx.types));
        let declarations = api_declarations(program, ctx.syntax, model.as_ref());
        let tokens = ctx.tokens();
        let missing = |d: &ApiDeclaration<'_>| {
            if d.name.starts_with('_')
                || d.internal
                || d.exempt_constructor
                || (d.is_top_level_function && d.name == "main")
                || d.documented(tokens)
            {
                return false;
            }
            if !d.is_static
                && let (Some(owner), Some(kind)) = (d.owner, d.kind)
            {
                let (Some(model), Some(signatures)) = (model.as_ref(), ctx.signatures) else {
                    return false;
                };
                let Some(identity) = model.resolve_name(owner) else {
                    return false;
                };
                let Some(facts) = signatures.inherited_member_facts(&identity, d.name) else {
                    return false;
                };
                // Like the analyzer's `overriddenMember`, a setter overrides
                // inherited setters; anything else overrides the inherited
                // member of its name, so a field never counts as overriding a
                // setter.
                if facts.iter().any(|f| {
                    let matches = if kind == MemberKind::Setter {
                        f.kind.has_setter()
                    } else {
                        f.kind != MemberKind::Setter
                    };
                    matches
                        && signatures.has_combined_inherited_member(
                            &identity,
                            d.name,
                            if kind == MemberKind::Setter {
                                MemberKind::Setter
                            } else {
                                f.kind
                            },
                            model,
                        )
                }) {
                    return false;
                }
            }
            true
        };
        let mut diagnostics: Vec<_> = declarations
            .iter()
            .filter(|d| missing(d))
            .filter(|d| {
                d.kind != Some(MemberKind::Setter)
                    || declarations.iter().any(|getter| {
                        getter.owner == d.owner
                            && getter.name == d.name
                            && getter.kind == Some(MemberKind::Getter)
                            && missing(getter)
                    })
            })
            .map(|d| {
                ctx.diagnostic(
                    "public-member-api-docs",
                    "Missing documentation for a public member.",
                    d.span,
                )
            })
            .collect();
        // A type and its unnamed primary header are consecutive declarations
        // at the same token. Analyzer reports that diagnostic only once.
        diagnostics.dedup_by_key(|diagnostic| (diagnostic.span.start, diagnostic.span.end));
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carriage_return_line_comments_do_not_hide_public_declarations() {
        let source = "// ordinary comment\rvoid visible() {}";
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
        } = falcon_dart_parser::parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let config = falcon_config::FalconConfig::default();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/public-member-api-docs/project/lib/cr_only.dart");
        let ctx = AnalyzeContext::new(&path, source, &config, &syntax);
        let diagnostics = PublicMemberApiDocs.analyze(&program, &ctx);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(
            &source[diagnostics[0].span.start..diagnostics[0].span.end],
            "visible"
        );
    }

    #[test]
    fn primary_constructor_has_its_own_identity_and_documentation() {
        let source = "// @dart = 3.13\n\
            /// Class documentation.\n\
            @deprecated\n\
            class Public.named(final int value) { this {} }\n\
            /// Closed class.\n\
            sealed class Closed();\n\
            class _Private();\n\
            extension type Wrapper(int value) {}";
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
        } = falcon_dart_parser::parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let declarations = api_declarations(&program, &syntax, None);
        let tokens = syntax.tokens();
        let constructor = declarations.iter().find(|d| d.name == "named").unwrap();
        assert_eq!(
            &source[constructor.span.start..constructor.span.end],
            "this"
        );
        assert_eq!(constructor.owner, Some("Public"));
        assert!(!constructor.documented(tokens));
        assert!(constructor.annotations.is_empty());
        assert_eq!(
            declarations
                .iter()
                .filter(|d| d.owner == Some("Public"))
                .count(),
            1
        );
        assert!(
            declarations
                .iter()
                .any(|d| d.owner == Some("Closed") && !d.exempt_constructor)
        );
        assert!(
            !declarations
                .iter()
                .any(|d| d.owner == Some("_Private") || d.owner == Some("Wrapper"))
        );
    }

    #[test]
    fn coincident_type_and_primary_diagnostics_are_reported_once() {
        let source = "// @dart = 3.13\n\
            class A();\n\
            class B.named();\n\
            /// Documents only the class.\n\
            class C();\n\
            /// Documents only the mixin class.\n\
            mixin class M();";
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
        } = falcon_dart_parser::parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let config = falcon_config::FalconConfig::default();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/public-member-api-docs/project/lib/primary.dart");
        let ctx = AnalyzeContext::new(&path, source, &config, &syntax);
        let names: Vec<_> = PublicMemberApiDocs
            .analyze(&program, &ctx)
            .into_iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(names, ["A", "B", "named", "C", "M"]);
    }
}
