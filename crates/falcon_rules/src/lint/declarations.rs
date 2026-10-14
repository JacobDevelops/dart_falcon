//! Borrowed member extraction shared by declaration-agnostic lint scans.
//! Declaration-specific policies still inspect names, bases, and type parameters themselves.

use falcon_syntax::ast::{ClassMember, TopLevelDecl};

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

#[cfg(test)]
mod tests {
    use falcon_dart_parser::parse;

    use super::*;

    #[test]
    fn extracts_members_from_every_member_bearing_declaration() {
        let (program, errors) = parse(
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
        let (program, errors) = parse("class C {} enum E { value } extension on int {}");
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
                &["Enum", "Extension", "ExtensionType"],
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
                    "Mixin" => "mixin Probe",
                    "MixinClass" => "mixin class Probe",
                    "Enum" => "enum Probe",
                    "Extension" => "extension Probe on int",
                    "ExtensionType" => "extension type Probe(int representation)",
                    _ => panic!("unexpected newly included kind: {kind}"),
                };
                let variants = if kind == "Enum" { "value;" } else { "" };
                let source = format!("{header} {{ {variants} {member_source} }}");
                let (program, errors) = parse(&source);
                assert!(errors.is_empty(), "{name}/{kind}: {errors:?}");
                let ctx = AnalyzeContext::new(Path::new("containers.dart"), &source, &config);
                let diagnostics = rule.analyze(&program, &ctx);
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic.rule == name),
                    "{name} did not inspect newly included {kind} members: {diagnostics:?}",
                );
            }
        }
    }
}
