//! Borrowed member and initializer extraction and declaration-header positions
//! shared by declaration-agnostic lint scans. Declaration-specific policies
//! still inspect names, bases, and type parameters themselves.

use std::collections::HashSet;

use falcon_analyze::AnalyzeContext;
use falcon_syntax::ast::{
    Annotation, ClassMember, Expr, OperatorDecl, Program, Span, TopLevelDecl,
};

/// Comments after metadata take precedence over comments before annotations.
pub(crate) fn documented_node_span(
    span: Span,
    annotations: &[Annotation],
    ctx: &AnalyzeContext,
) -> Span {
    let syntax = ctx.syntax.syntax_for_span(span);
    let documentation_before = |start: usize| {
        let before = syntax.token_range(Span::new(start, start)).start.index();
        let gap_start = syntax.tokens()[..before]
            .iter()
            .rev()
            .take_while(|token| token.is_trivia())
            .last()
            .map_or(start, |token| token.offset());
        syntax.documentation_start(Span::new(gap_start, start), ctx.source)
    };
    let body_start = annotations.last().map_or(span.start, |annotation| {
        syntax
            .tokens_in(Span::new(annotation.span.end, span.end))
            .find(|token| !token.is_trivia())
            .map_or(span.start, |token| token.offset())
    });
    let documentation = documentation_before(body_start).or_else(|| {
        annotations
            .iter()
            .rev()
            .find_map(|annotation| documentation_before(annotation.span.start))
    });
    Span::new(
        documentation.map_or(span.start, |start| start.min(span.start)),
        span.end,
    )
}

/// Whether metadata has the given terminal name, for syntax-only consumers.
pub(crate) fn annotation(annotations: &[Annotation], name: &str) -> bool {
    annotations
        .iter()
        .any(|annotation| annotation.name.last().is_some_and(|part| part.name == name))
}

/// Resolve metadata rather than accepting an annotation from another library.
pub(crate) fn has_meta_annotation(
    annotations: &[Annotation],
    name: &str,
    model: &falcon_analyze::SemanticModel<'_>,
) -> bool {
    annotations.iter().any(|annotation| {
        let Some(qualified) = falcon_analyze::QualifiedName::from_identifiers(&annotation.name)
        else {
            return false;
        };
        model
            .resolve_value(qualified)
            .or_else(|| model.resolve_name(qualified))
            .or_else(|| model.resolve_imported_member(qualified))
            .is_some_and(|identity| identity.is_package("meta", name))
    })
}

/// Whether a file is in one of the analyzer's package-root test directories.
pub(crate) fn in_test_directory(path: &std::path::Path) -> bool {
    let Some(pubspec) = crate::cross_file::enclosing_pubspec(path) else {
        return false;
    };
    let Some(root) = pubspec.parent() else {
        return false;
    };
    falcon_analyze::canonical_or_lexical(path)
        .strip_prefix(root)
        .is_ok_and(|relative| {
            relative.components().next().is_some_and(|first| {
                matches!(
                    first.as_os_str().to_str(),
                    Some("test" | "integration_test" | "test_driver" | "testing")
                )
            })
        })
}

pub(crate) fn members_of(declaration: &TopLevelDecl) -> Option<&[ClassMember]> {
    match declaration {
        TopLevelDecl::Class(node) => Some(&node.members),
        TopLevelDecl::Mixin(node) => Some(&node.members),
        TopLevelDecl::MixinClass(node) => Some(&node.members),
        TopLevelDecl::Enum(node) => Some(&node.members),
        TopLevelDecl::Extension(node) => Some(&node.members),
        TopLevelDecl::ExtensionType(node) => Some(&node.members),
        _ => None,
    }
}

/// Top-level variable and field initializers, in declaration/declarator order.
/// Does not descend into expressions, local declarations, or constructor initializers.
pub(crate) fn declaration_initializers(program: &Program) -> impl Iterator<Item = &Expr> {
    program
        .declarations
        .iter()
        .flat_map(|declaration| {
            let variables = match declaration {
                TopLevelDecl::Variable(variable) => variable.declarators.as_slice(),
                _ => &[],
            };
            variables.iter().chain(
                members_of(declaration)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|member| match member {
                        ClassMember::Field(field) => Some(&field.declarators),
                        _ => None,
                    })
                    .flatten(),
            )
        })
        .filter_map(|declarator| declarator.initializer.as_ref())
}

/// Names of the fields, methods, getters, and setters `members` declare: the
/// names that shadow library declarations inside the declaration's body.
pub(crate) fn member_names(members: &[ClassMember]) -> HashSet<String> {
    let mut names = HashSet::new();
    for member in members {
        match member {
            ClassMember::Field(field) => {
                names.extend(
                    field
                        .declarators
                        .iter()
                        .map(|declarator| declarator.name.name.clone()),
                );
            }
            ClassMember::Method(method) => {
                names.insert(method.name.name.clone());
            }
            ClassMember::Getter(getter) => {
                names.insert(getter.name.name.clone());
            }
            ClassMember::Setter(setter) => {
                names.insert(setter.name.name.clone());
            }
            ClassMember::Constructor(_)
            | ClassMember::PrimaryConstructorBody(_)
            | ClassMember::Operator(_)
            | ClassMember::Error(_) => {}
        }
    }
    names
}

/// Identifies a top-level declaration by its address in the visited program,
/// for side tables keyed by declaration.
pub(crate) fn declaration_key(declaration: &TopLevelDecl) -> usize {
    std::ptr::from_ref(declaration).addr()
}

/// The span of the operator token (`==`, `[]=`) after the `operator` keyword
/// of `operator`; the declaration's span when the keyword is not found.
pub(crate) fn operator_token_span(
    operator: &OperatorDecl,
    syntax: &falcon_syntax::SyntaxStore,
) -> Span {
    let header_start = operator
        .annotations
        .last()
        .map_or(operator.span.start, |annotation| annotation.span.end);
    let mut tokens = syntax.tokens_in(Span::new(header_start, operator.params.span.start));
    tokens
        .find(|token| token.kind == falcon_syntax::TokenKind::Operator)
        .and_then(|_| tokens.find(|token| !token.is_trivia()))
        .map_or(operator.span, |token| {
            Span::new(token.offset(), token.offset() + operator.op.len())
        })
}

/// Offset of a declaration's first token after its annotations (the
/// analyzer's `firstTokenAfterCommentAndMetadata`), skipping whitespace and
/// comments; `span` is the declaration's span.
pub(crate) fn first_token_after_annotations(
    annotations: &[Annotation],
    span: Span,
    syntax: &falcon_syntax::SyntaxStore,
) -> usize {
    let offset = annotations
        .iter()
        .map(|annotation| annotation.span.end)
        .fold(span.start, usize::max);
    syntax
        .tokens_in(Span::new(offset, span.end))
        .find(|token| !token.is_trivia())
        .map_or(span.end, |token| token.offset())
}

#[cfg(test)]
mod tests {
    use falcon_analyze::RuleAnalysis;
    use falcon_dart_parser::parse;

    use super::*;

    #[test]
    fn extracts_members_from_every_member_bearing_declaration() {
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(
            "class C { void f() {} }
             mixin M { void f() {} }
             mixin class MC { void f() {} }
             enum E { value; void f() {} }
             extension X on int { void f() {} }
             extension type ET(int value) { void f() {} }
             void top() {}
             final global = 0;
             typedef Alias = int;",
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(program.declarations.len(), 9);
        for declaration in &program.declarations[..6] {
            let members = members_of(declaration).expect("member-bearing declaration");
            assert_eq!(members.len(), 1);
            assert!(matches!(&members[0], ClassMember::Method(method) if method.name.name == "f"));
        }
        for declaration in &program.declarations[6..] {
            assert!(members_of(declaration).is_none());
        }
    }

    #[test]
    fn preserves_empty_member_lists() {
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse("class C {} enum E { value } extension on int {}");
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(program.declarations.len(), 3);
        for declaration in &program.declarations {
            assert_eq!(members_of(declaration).map(<[ClassMember]>::len), Some(0));
        }
    }
    #[test]
    fn widened_container_dispatch_runs_each_rule() {
        use std::path::Path;

        use falcon_analyze::AnalyzeContext;
        use falcon_config::FalconConfig;

        use crate::meta::{RULE_METADATA, RuleKind};

        let cases: &[(&str, &[&str], &str)] = &[
            (
                "avoid-nested-if",
                &["ExtensionType"],
                "void f(bool a, bool b, bool c) { if (a) { if (b) { if (c) { print(a); } } } }",
            ),
            (
                "max-lines-for-function",
                &["ExtensionType"],
                "void f() {\n print(1);\n print(2);\n}",
            ),
            (
                "prefer-moving-to-variable",
                &["Enum", "Extension", "ExtensionType"],
                "void f(Object root) { final first = root.value; final second = root.value; }",
            ),
            (
                "correct-order-for-super-dispose",
                &["ExtensionType"],
                "void dispose() { super.dispose(); controller.dispose(); }",
            ),
            (
                "use-once-constructors-once-provider",
                &["Enum", "Extension", "ExtensionType"],
                "Object f() => OnceProvider((ref) => 1);",
            ),
            (
                "avoid-positional-fields-in-records",
                &["ExtensionType"],
                "void f() { final record = (1, 2); }",
            ),
            (
                "binary-expression-operand-order",
                &["Enum", "Extension", "ExtensionType"],
                "bool f(int value) => 5 == value;",
            ),
            (
                "boolean-prefixes",
                &["ExtensionType"],
                "bool get active => true;",
            ),
            (
                "newline-before-return",
                &["Enum", "Extension", "ExtensionType"],
                "int f() {\n final result = 1;\n return result;\n}",
            ),
            (
                "use-design-system-item",
                &["ExtensionType"],
                "Object f() => Container();",
            ),
            (
                "avoid-redundant-async",
                &["Enum", "Extension", "ExtensionType", "Mixin", "MixinClass"],
                "Future<int> f() async { return await compute(); }",
            ),
            (
                "prefer-conditional-expressions",
                &["Enum", "Extension", "ExtensionType"],
                "int f(bool flag) { if (flag) { return 1; } else { return 0; } }",
            ),
            (
                "prefer-immediate-return",
                &["Enum", "Extension", "ExtensionType", "MixinClass"],
                "int f() { final result = compute(); return result; }",
            ),
            (
                "unnecessary-getters-setters",
                &["Extension"],
                "static int _value = 0; static int get value => _value; static set value(int v) => _value = v;",
            ),
            (
                "avoid-global-state",
                &["Enum", "Extension", "ExtensionType"],
                "static var shared = 0;",
            ),
            (
                "avoid-returning-widgets",
                &["Enum", "Extension", "ExtensionType"],
                "Widget makeWidget() => Container();",
            ),
            (
                "hash-and-equals",
                &[
                    "Class",
                    "MixinClass",
                    "Mixin",
                    "Enum",
                    "Extension",
                    "ExtensionType",
                ],
                "int get hashCode => 42;",
            ),
            (
                "class-members-ordering",
                &["Enum", "Extension", "ExtensionType"],
                "void f() {} static const value = 1;",
            ),
            (
                "no-object-declaration",
                &["ExtensionType"],
                "Object f() => 0;",
            ),
            (
                "prefer-correct-identifier-length",
                &["ExtensionType"],
                "int get x => 0;",
            ),
            (
                "prefer-final-fields",
                &["Extension"],
                "static int _value = 1;",
            ),
            (
                "avoid-dynamic",
                &["ExtensionType"],
                "void f(dynamic value) {}",
            ),
            (
                "use-spacer-as-expanded-child",
                &["Enum", "Extension", "ExtensionType"],
                "Object f() => Expanded(child: SizedBox());",
            ),
        ];
        for &(name, kinds, member_source) in cases {
            let meta = RULE_METADATA.iter().find(|meta| meta.name == name).unwrap();
            let RuleKind::File { make, .. } = meta.kind else {
                panic!("expected a file rule: {name}");
            };
            let options = match name {
                "max-lines-for-function" => serde_json::json!({"max_lines": 2}),
                "use-design-system-item" => serde_json::json!({
                    "items": [{"class_name": "Container", "use_instead": "AppContainer"}]
                }),
                _ => serde_json::json!({}),
            };
            let config: FalconConfig = serde_json::from_value(serde_json::json!({
                "linter": {"rules": {
                    (meta.group.as_str()): {(name): {"level": "warn", "options": options}}
                }}
            }))
            .unwrap();
            let rule = make();
            for &kind in kinds {
                let header = match kind {
                    "Class" => "class Probe",
                    "Mixin" => "mixin Probe",
                    "MixinClass" => "mixin class Probe",
                    "Enum" => "enum Probe",
                    "Extension" => "extension Probe on int",
                    "ExtensionType" => "extension type Probe(int representation)",
                    _ => panic!("unexpected newly included kind: {kind}"),
                };
                let variants = if kind == "Enum" { "value;" } else { "" };
                let source = format!("{header} {{ {variants} {member_source} }}");
                let falcon_dart_parser::ParseResult {
                    program,
                    errors,
                    syntax,
                } = parse(&source);
                assert!(errors.is_empty(), "{name}/{kind}: {errors:?}");
                let ctx =
                    AnalyzeContext::new(Path::new("containers.dart"), &source, &config, &syntax);
                let diagnostics = rule.analyze(&program, &ctx);
                assert_eq!(
                    diagnostics.iter().any(|diagnostic| diagnostic.rule == name),
                    name != "hash-and-equals" || matches!(kind, "Class" | "MixinClass"),
                    "{name}/{kind} violated its declaration-kind contract: {diagnostics:?}",
                );
            }
        }
    }
}
