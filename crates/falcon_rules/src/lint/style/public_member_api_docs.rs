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

use falcon_analyze::{
    AnalyzeContext, MemberKind, Rule, RulePreparation, SemanticModel, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::{TokenKind, cst::*};

use crate::lint::declarations::has_meta_annotation;

#[derive(Default)]
pub struct PublicMemberApiDocs;

pub(crate) struct ApiDeclaration<'a> {
    pub name: &'a str,
    pub span: Span,
    pub start: usize,
    pub annotations: AstChildren<'a, Annotation<'a>>,
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

pub(crate) fn documented(tokens: &[SyntaxToken<'_>], start: usize) -> bool {
    let end = tokens.partition_point(|token| token.offset() < start);
    crate::lint::tokens::preceding_trivia(tokens[..end].iter().copied(), start)
        // The lexer leaves `/**/` as ordinary trivia; analyzer still attaches
        // that empty block as a declaration's documentation comment.
        .any(|t| {
            t.kind() == TokenKind::DocComment
                || (t.kind() == TokenKind::BlockComment && t.len() == 4)
        })
}

impl ApiDeclaration<'_> {
    /// Whether a doc comment precedes the declaration, its metadata, or any
    /// annotation within it.
    pub(crate) fn documented(&self, tokens: &[SyntaxToken<'_>]) -> bool {
        let Some(last) = self.annotations.clone().last() else {
            return documented(tokens, self.start);
        };
        let after = tokens.partition_point(|token| token.offset() < last.span().end);
        let keyword = tokens[after..]
            .iter()
            .find(|t| !t.is_trivia())
            .map_or(last.span().end, |t| t.offset());
        documented(tokens, self.start)
            || self
                .annotations
                .clone()
                .any(|a| documented(tokens, a.span().start))
            || documented(tokens, keyword)
    }
}

fn declaration_candidates<'a>(
    decl: TopLevelDecl<'a>,
    model: Option<&SemanticModel<'_>>,
    mut emit: impl FnMut(ApiDeclaration<'a>),
) {
    let is_internal = |annotations: AstChildren<'a, Annotation<'a>>| {
        model.is_some_and(|model| has_meta_annotation(annotations, "internal", model))
    };
    let (name, annotations) = match decl {
        TopLevelDecl::Class(n) => (Some(n.name()), n.annotations()),
        TopLevelDecl::ClassTypeAlias(n) => (Some(n.name()), n.annotations()),
        TopLevelDecl::Mixin(n) => (Some(n.name()), n.annotations()),
        TopLevelDecl::MixinClass(n) => (Some(n.name()), n.annotations()),
        TopLevelDecl::Enum(n) => (Some(n.name()), n.annotations()),
        TopLevelDecl::Extension(n) => (n.name(), n.annotations()),
        TopLevelDecl::ExtensionType(n) => (Some(n.name()), n.annotations()),
        TopLevelDecl::Function(n) => (Some(n.name()), n.annotations()),
        TopLevelDecl::TypeAlias(n) => (Some(n.name()), n.annotations()),
        TopLevelDecl::Variable(n) => {
            let internal = is_internal(n.annotations());
            for v in n.declarators() {
                emit(ApiDeclaration {
                    internal,
                    ..candidate(v.name(), n.span().start, n.annotations(), None, None, true)
                });
            }
            return;
        }
        TopLevelDecl::Error(_) => return,
    };
    let Some(name) = name.filter(|n| !n.name().starts_with('_')) else {
        return;
    };
    let mut top = candidate(
        name,
        decl.span().start,
        annotations.clone(),
        None,
        None,
        true,
    );
    top.internal = is_internal(annotations);
    if let TopLevelDecl::Function(n) = decl {
        top.is_top_level_function = true;
        top.kind = match n.kind() {
            FunctionKind::Getter => Some(MemberKind::Getter),
            FunctionKind::Setter(_) => Some(MemberKind::Setter),
            FunctionKind::Function(_) => None,
        };
    }
    let internal = top.internal;
    emit(top);
    let elements = match decl {
        TopLevelDecl::Class(n) => Some(n.element_members()),
        TopLevelDecl::MixinClass(n) => Some(n.element_members()),
        _ => None,
    };
    if let Some(elements) = elements
        && let Some(primary) = elements.primary
    {
        let constructor = elements.constructors().find(|c| c.is_primary).unwrap();
        let body = elements.declared.clone().find_map(|member| match member {
            ClassMember::PrimaryConstructorBody(body) => Some(body),
            _ => None,
        });
        let mut c = candidate(
            primary.constructor_name().unwrap_or(name),
            body.map_or(primary.span().start, |body| body.span().start),
            constructor.annotations,
            Some(name.name()),
            None,
            true,
        );
        if let Some(body) = body {
            c.span = body.this_span();
            c.internal = internal;
        }
        emit(c);
    }
}

fn member_candidates<'a>(
    member: ClassMember<'a>,
    name: &'a str,
    internal: bool,
    closed: bool,
    extension: bool,
    mut emit: impl FnMut(ApiDeclaration<'a>),
) {
    let mut report = |mut declaration: ApiDeclaration<'a>| {
        declaration.internal = internal;
        if extension {
            declaration.is_static = true;
        }
        emit(declaration);
    };
    match member {
        ClassMember::Field(n) => {
            for v in n.declarators() {
                report(candidate(
                    v.name(),
                    n.span().start,
                    n.annotations(),
                    Some(name),
                    Some(MemberKind::field(n, v)),
                    n.is_static(),
                ));
            }
        }
        ClassMember::Method(n) => report(candidate(
            n.name(),
            n.span().start,
            n.annotations(),
            Some(name),
            Some(MemberKind::Method),
            n.is_static(),
        )),
        ClassMember::Getter(n) => report(candidate(
            n.name(),
            n.span().start,
            n.annotations(),
            Some(name),
            Some(MemberKind::Getter),
            n.is_static(),
        )),
        ClassMember::Setter(n) => report(candidate(
            n.name(),
            n.span().start,
            n.annotations(),
            Some(name),
            Some(MemberKind::Setter),
            n.is_static(),
        )),
        ClassMember::Constructor(n) => {
            let mut c = candidate(
                n.constructor_name().unwrap_or(n.name()),
                n.span().start,
                n.annotations(),
                Some(name),
                None,
                true,
            );
            c.exempt_constructor = closed;
            report(c);
        }
        ClassMember::Operator(n) => report(ApiDeclaration {
            name: n.lookup_name(),
            span: crate::lint::declarations::operator_token_span(n),
            start: n
                .annotations()
                .first()
                .map_or(n.span().start, |a| a.span().start),
            annotations: n.annotations(),
            owner: Some(name),
            kind: Some(MemberKind::Operator),
            is_static: false,
            is_top_level_function: false,
            exempt_constructor: false,
            internal,
        }),
        ClassMember::PrimaryConstructorBody(_) | ClassMember::Error(_) => {}
    }
}

fn candidate<'a>(
    name: Identifier<'a>,
    start: usize,
    annotations: AstChildren<'a, Annotation<'a>>,
    owner: Option<&'a str>,
    kind: Option<MemberKind>,
    is_static: bool,
) -> ApiDeclaration<'a> {
    ApiDeclaration {
        name: name.name(),
        span: name.span(),
        start: annotations
            .first()
            .map_or(start, |a| a.span().start.min(start)),
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
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if crate::cross_file::lib_dir_package(ctx.file_path()).is_none() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let model = ctx.identities().map(|identities| {
            SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types())
        });
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model,
            tokens: ctx.tokens().collect(),
            owner: None,
            pending: Vec::new(),
        }))
    }
}

#[derive(Clone, Copy)]
struct Owner<'a> {
    name: &'a str,
    internal: bool,
    closed: bool,
    extension: bool,
}

struct PendingDoc<'a> {
    name: &'a str,
    owner: Option<&'a str>,
    kind: Option<MemberKind>,
    span: Span,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: Option<SemanticModel<'a>>,
    tokens: Vec<SyntaxToken<'a>>,
    owner: Option<Owner<'a>>,
    pending: Vec<PendingDoc<'a>>,
}

impl Collector<'_> {
    fn missing(&self, d: &ApiDeclaration<'_>) -> bool {
        let ctx = self.ctx;
        let model = self.model.as_ref();
        if d.name.starts_with('_')
            || d.internal
            || d.exempt_constructor
            || (d.is_top_level_function && d.name == "main")
            || d.documented(&self.tokens)
        {
            return false;
        }
        if !d.is_static
            && let (Some(owner), Some(kind)) = (d.owner, d.kind)
        {
            let (Some(model), Some(signatures)) = (model, ctx.signatures()) else {
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
    }

    fn record(&mut self, d: &ApiDeclaration<'_>) {
        if self.missing(d) {
            self.pending.push(PendingDoc {
                name: &self.ctx.source()[d.span.start..d.span.end],
                owner: d
                    .owner
                    .map(|_| self.owner.as_ref().expect("member owner").name),
                kind: d.kind,
                span: d.span,
            });
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        self.owner = None;
        let model = self.model;
        declaration_candidates(decl, model.as_ref(), |declaration| {
            if declaration.owner.is_none() {
                self.owner = Some(Owner {
                    name: &self.ctx.source()[declaration.span.start..declaration.span.end],
                    internal: declaration.internal,
                    closed: match decl {
                        TopLevelDecl::Class(n) => match n.modifiers().restriction {
                            Some(ClassRestriction::Sealed) => true,
                            Some(ClassRestriction::Final | ClassRestriction::Interface) => {
                                n.modifiers().is_abstract
                            }
                            _ => false,
                        },
                        TopLevelDecl::Enum(_) => true,
                        _ => false,
                    },
                    extension: matches!(decl, TopLevelDecl::Extension(_)),
                });
            }
            self.record(&declaration);
        });
        if self.owner.is_none() {
            return StructuralTraversal::SkipChildren;
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, member: ClassMember<'a>) -> StructuralTraversal {
        if let Some(owner) = self.owner {
            member_candidates(
                member,
                owner.name,
                owner.internal,
                owner.closed,
                owner.extension,
                |declaration| self.record(&declaration),
            );
        }
        StructuralTraversal::SkipChildren
    }

    fn visit_enum_variant(&mut self, variant: EnumVariant<'a>) -> StructuralTraversal {
        if let Some(owner) = self.owner {
            let mut declaration = candidate(
                variant.name(),
                variant.span().start,
                variant.annotations(),
                Some(owner.name),
                None,
                true,
            );
            declaration.internal = owner.internal;
            self.record(&declaration);
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        let mut diagnostics: Vec<_> = self.pending.iter()
            .filter(|d| {
                d.kind != Some(MemberKind::Setter)
                    || self.pending.iter().any(|getter| {
                        getter.owner == d.owner
                            && getter.name == d.name
                            && getter.kind == Some(MemberKind::Getter)
                    })
            })
            .map(|d| self.ctx.diagnostic(
                "public-member-api-docs",
                "Missing documentation for a public member.",
                d.span,
            ))
            .collect();
        // A type and its unnamed primary header are consecutive declarations
        // at the same token. Analyzer reports that diagnostic only once.
        diagnostics.dedup_by_key(|diagnostic| (diagnostic.span.start, diagnostic.span.end));
        diagnostics
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;

    #[test]
    fn carriage_return_line_comments_do_not_hide_public_declarations() {
        let source = "// ordinary comment\rvoid visible() {}";
        let falcon_dart_parser::ParseResult { tree, errors } = falcon_dart_parser::parse(source);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        assert!(errors.is_empty(), "{errors:?}");
        let config = falcon_config::FalconConfig::default();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/public-member-api-docs/project/lib/cr_only.dart");
        let ctx = AnalyzeContext::new(&path, &config, &parsed);
        let diagnostics = PublicMemberApiDocs.analyze(program, &ctx);
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
        let falcon_dart_parser::ParseResult { tree, errors } = falcon_dart_parser::parse(source);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        assert!(errors.is_empty(), "{errors:?}");
        let mut declarations = Vec::with_capacity(program.declarations().len() * 2);
        for decl in program.declarations() {
            declaration_candidates(decl, None, |declaration| declarations.push(declaration));
        }
        let tokens: Vec<_> = parsed.tree.tokens(&parsed.source).collect();
        let constructor = declarations.iter().find(|d| d.name == "named").unwrap();
        assert_eq!(
            &source[constructor.span.start..constructor.span.end],
            "this"
        );
        assert_eq!(constructor.owner, Some("Public"));
        assert!(!constructor.documented(&tokens));
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
        let falcon_dart_parser::ParseResult { tree, errors } = falcon_dart_parser::parse(source);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        assert!(errors.is_empty(), "{errors:?}");
        let config = falcon_config::FalconConfig::default();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/public-member-api-docs/project/lib/primary.dart");
        let ctx = AnalyzeContext::new(&path, &config, &parsed);
        let names: Vec<_> = PublicMemberApiDocs
            .analyze(program, &ctx)
            .into_iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(names, ["A", "B", "named", "C", "M"]);
    }

    #[test]
    fn reports_paired_accessors_in_declaration_order() {
        use falcon_analyze::{IdentityIndex, IdentitySource, SignatureIndex};

        let source = r"
/// Documented class.
class Accessors {
  set value(int next) {}
  int get value => 0;
}
set top(int next) {}
int get top => 0;
";
        let falcon_dart_parser::ParseResult { tree, errors } = falcon_dart_parser::parse(source);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        assert!(errors.is_empty(), "{errors:?}");
        let config = falcon_config::FalconConfig::default();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/public-member-api-docs/project/lib/accessor_order.dart");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                parsed: &parsed,
                has_parse_errors: false,
            }],
            &[],
        );
        let model = SemanticModel::new(&path, &parsed, &identities, None);
        let signatures = SignatureIndex::from_program(program, &model);
        let ctx = AnalyzeContext::new(&path, &config, &parsed)
            .with_identities(&identities)
            .with_signatures(&signatures);
        let diagnostics = PublicMemberApiDocs.analyze(program, &ctx);
        let spans: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.span.start, diagnostic.span.end))
            .collect();
        let expected = ["value(int", "value =>", "top(int", "top =>"].map(|needle| {
            let start = source.find(needle).unwrap();
            (start, start + needle.find(['(', ' ']).unwrap())
        });
        assert_eq!(spans, expected);
    }
}
