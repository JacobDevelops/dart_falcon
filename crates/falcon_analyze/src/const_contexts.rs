//! Dart constant-context roots shared by all rules analyzing a file.

use falcon_syntax::cst::visit::{
    Visitor, walk_annotation, walk_enum_decl, walk_expr_chain, walk_field_decl, walk_local_var,
    walk_program, walk_switch_case,
};
use falcon_syntax::cst::*;
use falcon_syntax::token::TokenKind;

#[derive(Default)]
struct Ranges {
    spans: Vec<Span>,
    max_end: Vec<usize>,
}

impl Ranges {
    fn finish(&mut self) {
        self.spans
            .sort_unstable_by_key(|span| (span.start, span.end));
        self.spans.dedup();
        self.max_end = self
            .spans
            .iter()
            .scan(0usize, |max, span| {
                *max = (*max).max(span.end);
                Some(*max)
            })
            .collect();
    }

    fn contains(&self, span: &Span, inclusive: bool) -> bool {
        let before = self
            .spans
            .partition_point(|context| context.start < span.start);
        if before > 0 && self.max_end[before - 1] >= span.end {
            return true;
        }
        let through = self
            .spans
            .partition_point(|context| context.start <= span.start);
        through > before
            && if inclusive {
                self.spans[through - 1].end >= span.end
            } else {
                self.spans[through - 1].end > span.end
            }
    }
}

/// Constant initializers, metadata, enum arguments and legacy switch cases are
/// implicit contexts including their root. Explicit `const` expressions create
/// a context for their operands, not for the expression itself. Constructor
/// assertion initializers are tracked separately: they are potentially constant,
/// not implicit constant contexts.
#[derive(Default)]
pub struct ConstContexts {
    implicit: Ranges,
    explicit: Ranges,
    constructor_asserts: Ranges,
}

impl ConstContexts {
    #[must_use]
    pub fn new(program: Program<'_>) -> Self {
        let mut contexts = Self::default();
        contexts.visit_program(program);
        contexts.implicit.finish();
        contexts.explicit.finish();
        contexts.constructor_asserts.finish();
        contexts
    }

    #[must_use]
    pub fn contains(&self, span: &Span) -> bool {
        self.implicit.contains(span, true) || self.explicit.contains(span, false)
    }

    /// Includes an explicitly constant expression's own root.
    #[must_use]
    pub fn contains_including_explicit(&self, span: &Span) -> bool {
        self.implicit.contains(span, true) || self.explicit.contains(span, true)
    }

    #[must_use]
    pub fn in_const_constructor_assert(&self, span: &Span) -> bool {
        self.constructor_asserts.contains(span, true)
    }

    fn initializers<'a>(
        &mut self,
        keyword: VarKeyword,
        declarators: AstChildren<'a, VarDeclarator<'a>>,
    ) {
        if keyword == VarKeyword::Const {
            self.implicit.spans.extend(
                declarators
                    .filter_map(falcon_syntax::cst::VarDeclarator::initializer)
                    .map(falcon_syntax::cst::Expr::span),
            );
        }
    }
}

impl<'a> Visitor<'a> for ConstContexts {
    fn visit_program(&mut self, node: Program<'a>) {
        for declaration in node.declarations() {
            if let TopLevelDecl::Variable(variable) = declaration {
                self.initializers(variable.keyword(), variable.declarators());
            }
        }
        walk_program(self, node);
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) {
        self.initializers(node.keyword(), node.declarators());
        walk_field_decl(self, node);
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) {
        self.initializers(node.keyword(), node.declarators());
        walk_local_var(self, node);
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'a>) {
        if node.is_const {
            for initializer in node.initializers {
                if let ConstructorInitializer::Assert(assertion) = initializer {
                    self.constructor_asserts.spans.push(assertion.span());
                }
            }
        }
    }

    fn visit_enum_decl(&mut self, node: EnumDecl<'a>) {
        self.implicit.spans.extend(
            node.variants()
                .filter_map(falcon_syntax::cst::EnumVariant::args)
                .map(falcon_syntax::cst::ArgList::span),
        );
        walk_enum_decl(self, node);
    }

    fn visit_annotation(&mut self, node: Annotation<'a>) {
        if let Some(args) = node.args() {
            self.implicit.spans.push(args.span());
        }
        walk_annotation(self, node);
    }

    fn visit_switch_case(&mut self, node: SwitchCase<'a>) {
        for case in node.cases() {
            if let SwitchCaseKind::Expr(expression) = case {
                self.implicit.spans.push(expression.span());
            }
        }
        walk_switch_case(self, node);
    }

    fn visit_expr(&mut self, node: Expr<'a>) {
        walk_expr_chain(self, node, |visitor, node| {
            let explicit = match node {
                Expr::New(node) => node.is_const(),
                Expr::List(node) => node.is_const(),
                Expr::Map(node) => node.is_const(),
                Expr::Set(node) => node.is_const(),
                Expr::Record(node) => node.is_const(),
                Expr::Paren(node) => node.syntax().child_token(TokenKind::Const).is_some(),
                Expr::DotShorthand(node) => node.is_const(),
                Expr::Call(call) => {
                    matches!(call.callee().unparenthesized(), Expr::DotShorthand(shorthand) if shorthand.is_const())
                }
                _ => false,
            };
            if explicit {
                visitor.explicit.spans.push(node.span());
            }
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnalyzeContext, ParsedFile};
    use std::path::Path;
    use std::sync::Arc;

    fn span(source: &str, text: &str) -> Span {
        let start = source.find(text).unwrap();
        Span::new(start, start + text.len())
    }

    #[test]
    fn long_initializer_chains_keep_nested_constant_contexts_on_the_normal_stack() {
        let source = format!(
            "void f(int a) {{ final value = a{} + const [Object()].length; }}",
            " + a".repeat(99_998)
        );
        let parsed = falcon_dart_parser::parse(&source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let contexts = ConstContexts::new(parsed.tree.program(&source));
        assert!(!contexts.contains(&span(&source, "a + a")));
        assert!(!contexts.contains(&span(&source, "const [Object()]")));
        assert!(contexts.contains_including_explicit(&span(&source, "const [Object()]")));
        assert!(contexts.contains(&span(&source, "Object()")));
    }

    #[test]
    fn distinguishes_explicit_roots_from_implicit_contexts() {
        let source = "const value = [Object()]; final other = const [Object()];";
        let parsed = falcon_dart_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let contexts = ConstContexts::new(parsed.tree.program(source));
        assert!(contexts.contains(&span(source, "[Object()]")));
        let explicit = span(source, "const [Object()]");
        assert!(!contexts.contains(&explicit));
        assert!(contexts.contains_including_explicit(&explicit));
        let start = source.rfind("Object()").unwrap();
        assert!(contexts.contains(&Span::new(start, start + "Object()".len())));
    }

    #[test]
    fn constant_patterns_include_inner_constructors_but_not_explicit_roots() {
        let source = "class C { const C(this.value); final int value; } void f(Object v) { switch (v) { case const (C(1)): break; } if (v case const (C(2))) {} }";
        let parsed = falcon_dart_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let contexts = ConstContexts::new(parsed.tree.program(source));
        for (pattern, constructor) in [("const (C(1))", "C(1)"), ("const (C(2))", "C(2)")] {
            let explicit = span(source, pattern);
            assert!(!contexts.contains(&explicit));
            assert!(contexts.contains_including_explicit(&explicit));
            assert!(contexts.contains(&span(source, constructor)));
        }
    }

    #[test]
    fn excludes_required_constant_positions_without_implicit_const() {
        let source = "class C { const C() : assert(1 == 1); } void f(Object value, [Object x = const C()]) { switch (value) { case const C(): break; case {const C(): _}: break; case == const C(): break; } }";
        let parsed = falcon_dart_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let contexts = ConstContexts::new(parsed.tree.program(source));
        let start = source.find("x = const C()").unwrap() + "x = ".len();
        let default = Span::new(start, start + "const C()".len());
        assert!(!contexts.contains(&default));
        assert!(contexts.contains_including_explicit(&default));
        assert!(contexts.in_const_constructor_assert(&span(source, "1 == 1")));
        assert!(!contexts.contains(&span(source, "1 == 1")));
        let start = source.rfind("const C()").unwrap();
        let relational_operand = Span::new(start, start + "const C()".len());
        assert!(!contexts.contains(&relational_operand));
        assert!(contexts.contains_including_explicit(&relational_operand));
        let start = source.find("{const C()").unwrap() + 1;
        assert!(!contexts.contains(&Span::new(start, start + "const C()".len())));
    }

    #[test]
    fn caches_indexes_per_file_and_rebinds_to_fresh_facts() {
        let source = "void f() { const x = 1; print(x); }";
        let parsed = falcon_dart_parser::parse(source);
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let config = falcon_config::FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("first.dart"), &config, &parsed);
        let program = parsed.program();
        assert!(std::ptr::eq(
            ctx.function_bodies(program),
            ctx.function_bodies(program)
        ));
        assert!(std::ptr::eq(
            ctx.const_contexts(program),
            ctx.const_contexts(program)
        ));
        assert!(std::ptr::eq(
            ctx.local_declarations(program),
            ctx.local_declarations(program)
        ));
        assert!(ctx.const_contexts(program).contains(&span(source, "1")));
        assert!(
            ctx.function_bodies(program)
                .enclosing(&span(source, "print(x)"))
                .is_some()
        );
        let other_source = "void other() {}";
        let other = falcon_dart_parser::parse(other_source);
        let other = Arc::new(ParsedFile {
            source: other_source.into(),
            tree: other.tree,
        });
        let rebound = ctx.for_file(Path::new("other.dart"), &other);
        assert!(rebound.local_declarations(other.program()).is_empty());
        assert!(
            !rebound
                .const_contexts(other.program())
                .contains(&span(source, "1"))
        );
        assert!(
            rebound
                .function_bodies(other.program())
                .enclosing(&span(source, "print(x)"))
                .is_none()
        );
    }
}
