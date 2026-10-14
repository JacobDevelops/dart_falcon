//! Documents public declarations and members in package libraries.
//!
//! Adopted from the official `public_member_api_docs` lint. Internal and private
//! declarations, top-level main, enum constructors, and overriding members are
//! exempt. Setters are checked only when their paired getter is reported.
//! Unknown inheritance stays silent. The AST does not model class primary
//! constructors. Extension-type representation fields are not field declarations
//! visited by the upstream rule.

use falcon_analyze::{AnalyzeContext, MemberKind, Rule, SemanticModel};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::{TokenKind, ast::*};

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
    pub enum_constructor: bool,
    pub internal: bool,
}

pub(crate) fn annotation(annotations: &[Annotation], name: &str) -> bool {
    annotations
        .iter()
        .any(|a| a.name.last().is_some_and(|n| n.name == name))
}

pub(crate) fn documented(tokens: &[falcon_syntax::Token], start: usize) -> bool {
    let end = tokens.partition_point(|t| t.offset < start);
    tokens[..end]
        .iter()
        .rev()
        .take_while(|t| t.is_trivia())
        .any(|t| t.kind == TokenKind::DocComment)
}

pub(crate) fn api_declarations<'a>(program: &'a Program, source: &str) -> Vec<ApiDeclaration<'a>> {
    let mut result = Vec::new();
    let mut candidates = Vec::new();
    for decl in &program.declarations {
        let (name, annotations, members, is_enum) = match decl {
            TopLevelDecl::Class(n) => (
                Some(&n.name),
                n.annotations.as_slice(),
                n.members.as_slice(),
                false,
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
                    result.push(candidate(
                        &v.name,
                        n.span.start,
                        &n.annotations,
                        None,
                        None,
                        true,
                    ));
                }
                continue;
            }
            TopLevelDecl::Error(_) => continue,
        };
        let Some(name) = name.filter(|n| !n.name.starts_with('_')) else {
            continue;
        };
        let mut top = candidate(name, decl.span().start, annotations, None, None, true);
        if let TopLevelDecl::Function(n) = decl {
            top.kind = match n.kind {
                FunctionKind::Getter => Some(MemberKind::Getter),
                FunctionKind::Setter(_) => Some(MemberKind::Setter),
                FunctionKind::Function(_) => None,
            };
        }
        result.push(top);
        let internal = annotation(annotations, "internal");
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
                c.internal |= internal;
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
                            Some(MemberKind::field(n)),
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
                    c.enum_constructor = is_enum;
                    candidates.push(c);
                }
                ClassMember::Operator(n) => {
                    if let Some(keyword) = source[n.span.start..n.span.end].find("operator") {
                        let start = n.span.start + keyword + "operator".len();
                        let suffix = &source[start..n.span.end];
                        let offset = start + suffix.len() - suffix.trim_start().len();
                        candidates.push(ApiDeclaration {
                            name: &n.op,
                            span: Span {
                                start: offset,
                                end: offset + n.op.len(),
                            },
                            start: n.annotations.first().map_or(n.span.start, |a| a.span.start),
                            annotations: &n.annotations,
                            owner: Some(&name.name),
                            kind: Some(MemberKind::Operator),
                            is_static: false,
                            enum_constructor: false,
                            internal: annotation(&n.annotations, "internal"),
                        });
                    }
                }
                ClassMember::Error(_) => {}
            }
            for mut c in candidates.drain(..) {
                c.internal |= internal;
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
        enum_constructor: false,
        internal: annotation(annotations, "internal"),
    }
}

impl Rule for PublicMemberApiDocs {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        if crate::cross_file::lib_dir_package(ctx.file_path).is_none() {
            return Vec::new();
        }
        let declarations = api_declarations(program, ctx.source);
        let tokens = falcon_dart_parser::lexer::Lexer::new(ctx.source).tokenize();
        let missing = |d: &ApiDeclaration<'_>| {
            if d.name.starts_with('_')
                || d.internal
                || d.enum_constructor
                || (d.owner.is_none() && d.name == "main")
                || documented(&tokens, d.start)
                || annotation(d.annotations, "override")
            {
                return false;
            }
            if !d.is_static
                && let (Some(owner), Some(kind)) = (d.owner, d.kind)
            {
                let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
                    return false;
                };
                let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
                let Some(identity) = model.resolve_name(owner) else {
                    return false;
                };
                let Some(facts) = signatures.inherited_member_facts(&identity, d.name) else {
                    return false;
                };
                if facts.iter().any(|f| {
                    if matches!(kind, MemberKind::Method | MemberKind::Operator) {
                        f.kind == kind
                    } else {
                        (kind.has_getter() && f.kind.has_getter())
                            || (kind.has_setter() && f.kind.has_setter())
                    }
                }) {
                    return false;
                }
            }
            true
        };
        declarations
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
            .collect()
    }
}
