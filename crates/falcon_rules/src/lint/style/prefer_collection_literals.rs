//! Flags no-argument collection constructor calls that a literal would express.
//!
//! The empty `List()`, `Map()`, `Set()`, `LinkedHashMap()`, and `LinkedHashSet()`
//! constructors are more verbose than the equivalent `[]` or `{}` literals, which
//! are the idiomatic way to build a fresh collection and read more clearly. The
//! rule matches the unnamed and `.new` constructors written as a plain call
//! (`List()`, `Set.new()`), an explicit `new` expression (`new Map()`), or with
//! type arguments (`Map<K, V>()`). It deliberately spares named constructors such
//! as `List.filled` or `Map.of` and any invocation carrying arguments, since
//! those have no literal form. Adopted from package:lints
//! `prefer_collection_literals`.
//!
//! Two further facts withhold a diagnostic that would otherwise change semantics:
//!
//! * **Concrete context** — a `LinkedHashMap()` / `LinkedHashSet()` whose declared
//!   context type (a variable/field/top-level/return annotation) *is* that concrete
//!   type. A `{}` literal has static type `Map`/`Set`, which is not assignable back
//!   to `LinkedHashMap`/`LinkedHashSet`, so the constructor is required. This check
//!   is purely syntactic and applies without a type index. Local functions and
//!   operators use their own return annotations, never their enclosing function's.
//! * **Shadowed or unresolved identity** — a `List()`/`Map()`/`Set()`/… whose
//!   resolved declaration is not the SDK collection, or whose library/import
//!   graph cannot establish that identity. A literal could construct a different
//!   type. Without an identity index, the rule remains syntactic and such a
//!   constructor keeps firing.
//!
//! Everything else — including any `var`/unannotated context, and a concrete
//! constructor reached only through a nested argument position — keeps firing.

use std::collections::HashSet;

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferCollectionLiterals;

const NAMES: [&str; 5] = ["List", "Map", "Set", "LinkedHashMap", "LinkedHashSet"];

// A literal's Map/Set type cannot replace these concrete declared contexts.
const CONCRETE: [&str; 2] = ["LinkedHashMap", "LinkedHashSet"];

impl Rule for PreferCollectionLiterals {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            diags: Vec::new(),
            ctx,
            suppressed: HashSet::new(),
            expected: None,
            contexts: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    suppressed: HashSet<(usize, usize)>,
    expected: Option<&'static str>,
    contexts: Vec<Option<&'static str>>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        self.contexts.push(self.expected.take());
        if matches!(node, Expr::Paren { .. }) {
            return StructuralTraversal::Continue;
        }
        if let Some((span, name)) = default_collection_ctor(node) {
            let user_shadow = match self.ctx.identities {
                Some(identities) => identities
                    .file(self.ctx.file_path)
                    .and_then(|file| {
                        identities.resolve_declaration(identities.library(file), name.into())
                    })
                    .is_none_or(|identity| {
                        !identity.is_sdk("dart:core", name)
                            && !identity.is_sdk("dart:collection", name)
                    }),
                None => false,
            };
            let context_required = self.suppressed.contains(&(span.start, span.end));
            if !user_shadow && !context_required {
                self.diags.push(self.ctx.diagnostic(
                    "prefer-collection-literals",
                    "Use a collection literal instead of a constructor invocation.",
                    *span,
                ));
            }
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: &Expr) {
        self.expected = self.contexts.pop().unwrap();
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
        if let TopLevelDecl::Variable(v) = node {
            self.record_decls(&v.var_type, &v.declarators);
        }
        StructuralTraversal::Continue
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) -> StructuralTraversal {
        self.record_decls(&node.field_type, &node.declarators);
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) -> StructuralTraversal {
        self.record_decls(&node.var_type, &node.declarators);
        StructuralTraversal::Continue
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) -> StructuralTraversal {
        self.enter_return(node.return_type.as_ref());
        StructuralTraversal::Continue
    }

    fn exit_function_decl(&mut self, _node: &FunctionDecl) {
        self.expected = self.contexts.pop().unwrap();
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) -> StructuralTraversal {
        self.enter_return(node.return_type.as_ref());
        StructuralTraversal::Continue
    }

    fn exit_method_decl(&mut self, _node: &MethodDecl) {
        self.expected = self.contexts.pop().unwrap();
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) -> StructuralTraversal {
        self.enter_return(node.return_type.as_ref());
        StructuralTraversal::Continue
    }

    fn exit_getter_decl(&mut self, _node: &GetterDecl) {
        self.expected = self.contexts.pop().unwrap();
    }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        self.enter_return(match node {
            ClassMember::Operator(op) => op.return_type.as_ref(),
            _ => None,
        });
        StructuralTraversal::Continue
    }

    fn exit_class_member(&mut self, _node: &ClassMember) {
        self.expected = self.contexts.pop().unwrap();
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        self.contexts.push(self.expected);
        if let Stmt::LocalFunc(function) = node {
            self.expected = function.return_type.as_ref().and_then(concrete_context);
        } else if let Stmt::Return(ret) = node
            && let (Some(value), Some(expected)) = (&ret.value, self.expected)
        {
            self.record_if_matching(value, expected);
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, _node: &Stmt) {
        self.expected = self.contexts.pop().unwrap();
    }

    fn visit_function_body(&mut self, node: &FunctionBody) -> StructuralTraversal {
        if let FunctionBody::Arrow(expr, _) = node
            && let Some(expected) = self.expected
        {
            self.record_if_matching(expr, expected);
        }
        StructuralTraversal::Continue
    }
}
}

impl Collector<'_> {
    fn enter_return(&mut self, ty: Option<&DartType>) {
        self.contexts.push(self.expected);
        self.expected = ty.and_then(concrete_context);
    }
    fn record_decls(&mut self, ty: &Option<DartType>, decls: &[VarDeclarator]) {
        let Some(ty) = ty else { return };
        let Some(expected) = concrete_context(ty) else {
            return;
        };
        for d in decls {
            if let Some(init) = &d.initializer {
                self.record_if_matching(init, expected);
            }
        }
    }

    fn record_if_matching(&mut self, expr: &Expr, expected: &str) {
        if let Some((span, base)) = default_collection_ctor(expr)
            && base == expected
        {
            self.suppressed.insert((span.start, span.end));
        }
    }
}

fn concrete_context(ty: &DartType) -> Option<&'static str> {
    if let DartType::Named(nt) = ty
        && let Some(seg) = nt.segments.last()
        && CONCRETE.contains(&seg.name.as_str())
    {
        return CONCRETE.iter().copied().find(|name| *name == seg.name);
    }
    None
}

fn default_collection_ctor(expr: &Expr) -> Option<(&Span, &str)> {
    match expr.unparenthesized() {
        Expr::Call {
            callee, args, span, ..
        } => {
            if !args.positional.is_empty() || !args.named.is_empty() {
                return None;
            }
            match callee.unparenthesized() {
                Expr::Ident(id) if NAMES.contains(&id.name.as_str()) => {
                    Some((span, id.name.as_str()))
                }
                Expr::Field { object, field, .. } if field.name == "new" => {
                    match object.unparenthesized() {
                        Expr::Ident(id) if NAMES.contains(&id.name.as_str()) => {
                            Some((span, id.name.as_str()))
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        }
        Expr::New {
            dart_type: DartType::Named(nt),
            constructor_name,
            args,
            span,
            ..
        } => {
            if !args.positional.is_empty() || !args.named.is_empty() {
                return None;
            }
            let base = nt.segments.last()?;
            if !NAMES.contains(&base.name.as_str()) {
                return None;
            }
            match constructor_name {
                None => Some((span, base.name.as_str())),
                Some(c) if c.name == "new" => Some((span, base.name.as_str())),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_analyze::TypeIndex;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    fn count_no_types(source: &str) -> usize {
        let falcon_dart_parser::ParseResult {
            program, syntax, ..
        } = parse(source);
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, source, &config, &syntax);
        PreferCollectionLiterals.analyze(&program, &ctx).len()
    }

    fn count_with_types(source: &str) -> usize {
        let falcon_dart_parser::ParseResult {
            program, syntax, ..
        } = parse(source);
        let identities = falcon_analyze::IdentityIndex::from_project_files(
            &[falcon_analyze::IdentitySource {
                path: std::path::Path::new("t.dart"),
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_resolved_files(
            [(
                std::path::Path::new("t.dart"),
                falcon_analyze::LibrarySource {
                    program: &program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        );
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, source, &config, &syntax)
            .with_types(&types)
            .with_identities(&identities);
        PreferCollectionLiterals.analyze(&program, &ctx).len()
    }

    #[test]
    fn bare_constructors_fire_with_and_without_types() {
        let src = "import 'dart:collection'; \
                   void f() { var a = List(); var b = Map(); var c = LinkedHashMap(); }";
        assert_eq!(count_no_types(src), 3);
        assert_eq!(count_with_types(src), 3);
    }

    #[test]
    fn concrete_context_suppresses_only_relevant_ctor() {
        let src = "import 'dart:collection'; \
                   void f() { LinkedHashMap<int, int> m = LinkedHashMap(); }";
        assert_eq!(count_no_types(src), 0);
        assert_eq!(count_with_types(src), 0);
    }

    #[test]
    fn non_concrete_context_still_fires() {
        let src = "import 'dart:collection'; \
                   void f() { Map<int, int> m = LinkedHashMap(); }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn var_context_still_fires() {
        let src = "import 'dart:collection'; void f() { var m = LinkedHashMap(); }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn nested_arg_position_still_fires() {
        let src = "import 'dart:collection'; \
                   LinkedHashMap<int, int> id(LinkedHashMap<int, int> x) => x; \
                   void f() { LinkedHashMap<int, int> m = id(LinkedHashMap()); }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn concrete_return_type_suppresses() {
        let src = "import 'dart:collection'; \
                   LinkedHashSet<int> make() => LinkedHashSet(); \
                   LinkedHashMap<int, int> build() { return LinkedHashMap(); }";
        assert_eq!(count_with_types(src), 0);
    }

    #[test]
    fn concrete_field_suppresses() {
        let src = "import 'dart:collection'; \
                   class C { LinkedHashMap<int, int> m = LinkedHashMap(); }";
        assert_eq!(count_with_types(src), 0);
    }

    #[test]
    fn concrete_operator_return_suppresses() {
        let source = "import 'dart:collection';
            class C {
                LinkedHashMap operator +(C other) { return LinkedHashMap(); }
            }";
        assert_eq!(count_no_types(source), 0);
        assert_eq!(count_with_types(source), 0);
    }

    #[test]
    fn concrete_returns_follow_control_flow_without_crossing_function_boundaries() {
        for body in [
            "{ return LinkedHashMap(); }",
            "if (flag) return LinkedHashMap(); else return LinkedHashMap();",
            "while (flag) { return LinkedHashMap(); }",
            "do { return LinkedHashMap(); } while (flag);",
            "for (; flag;) { return LinkedHashMap(); }",
            "try { return LinkedHashMap(); } catch (e) { return LinkedHashMap(); } finally { return LinkedHashMap(); }",
            "switch (value) { case 0: return LinkedHashMap(); default: return LinkedHashMap(); }",
            "label: { return LinkedHashMap(); }",
        ] {
            let source = format!(
                "import 'dart:collection'; LinkedHashMap make(bool flag, int value) {{ {body} }}"
            );
            assert_eq!(count_no_types(&source), 0, "{body}");
            assert_eq!(count_with_types(&source), 0, "{body}");
        }
        let source = "import 'dart:collection';
            LinkedHashMap make() {
                Map local() { return LinkedHashMap(); }
                LinkedHashMap concrete() { return LinkedHashMap(); }
                var closure = () { return LinkedHashMap(); };
                return LinkedHashMap();
            }";
        assert_eq!(count_no_types(source), 2);
        assert_eq!(count_with_types(source), 2);
    }

    #[test]
    fn user_declared_shadow_suppresses_only_with_types() {
        let src = "class List {} void f() { var a = List(); }";
        assert_eq!(count_no_types(src), 1, "no type index → fire (baseline)");
        assert_eq!(count_with_types(src), 0, "user-declared List → suppress");
    }

    #[test]
    fn named_constructors_and_literals_never_fire() {
        let src = "void f() { var a = List.filled(3, 0); var b = <int>[]; var c = {}; }";
        assert_eq!(count_no_types(src), 0);
    }

    #[test]
    fn incomplete_resolved_library_does_not_replace_shadowed_collections() {
        use falcon_analyze::{IdentityIndex, IdentitySource, LibrarySource};
        for source in [
            "part 'missing.dart'; class Map {} void f() { var value = Map(); }",
            "part 'missing.dart'; class Duration {} void f() { var value = Map(); }",
        ] {
            let falcon_dart_parser::ParseResult {
                program,
                errors,
                syntax,
                ..
            } = parse(source);
            assert!(errors.is_empty());
            let path = PathBuf::from("/project/t.dart");
            let identities = IdentityIndex::from_project_files(
                &[IdentitySource {
                    path: &path,
                    program: &program,
                    has_parse_errors: false,
                }],
                &[],
            );
            let types = TypeIndex::from_resolved_files(
                [(
                    path.as_path(),
                    LibrarySource {
                        program: &program,
                        has_parse_errors: false,
                        has_unresolved_parts: true,
                    },
                )],
                &identities,
            );
            let config = FalconConfig::default();
            let ctx = AnalyzeContext::new(&path, source, &config, &syntax)
                .with_types(&types)
                .with_identities(&identities);
            assert!(
                PreferCollectionLiterals.analyze(&program, &ctx).is_empty(),
                "{source}"
            );
        }
    }
}
