//! Span-independent syntax retained for analyzer `defaultValueCode` equality.

use std::sync::Arc;

use falcon_dart_parser::lexer::Lexer;
#[cfg(test)]
use falcon_syntax::cst::TopLevelDecl;
use falcon_syntax::cst::{
    Annotation, ArgList, AssignOp, BinaryOp, DartType, Element, ElementKind, Expr, Identifier,
    IfCondition, NodeKind, PostfixIncDec, StringLitNode, UnaryOp,
    visit::{self as visitor, Visitor},
};

/// An exact syntax key, computed once when ingesting a parameter default.
///
/// Recovery and nonconstant-only forms compare only to clones of the same key:
/// missing syntax must never make unrelated defaults appear equivalent.
#[derive(Debug, Clone)]
pub struct DefaultSyntaxKey {
    tokens: Arc<[Token]>,
    complete: bool,
}

impl PartialEq for DefaultSyntaxKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.tokens, &other.tokens)
            || (self.complete && other.complete && self.tokens == other.tokens)
    }
}

impl Eq for DefaultSyntaxKey {}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Expr(NodeKind),
    Parenthesized,
    Annotation,
    Type(NodeKind),
    Element(NodeKind),
    MapElement(NodeKind),
    Text(Box<str>),
    Count(usize),
    Flag(bool),
    Unary(UnaryOp),
    Binary(BinaryOp),
    Assign(AssignOp),
    Postfix(PostfixIncDec),
    End,
}

pub fn default_syntax_key(expr: Expr<'_>) -> DefaultSyntaxKey {
    let mut key = SyntaxCollector {
        tokens: Vec::new(),
        complete: true,
    };
    key.visit_expr(expr);
    DefaultSyntaxKey {
        tokens: key.tokens.into(),
        complete: key.complete,
    }
}

struct SyntaxCollector {
    tokens: Vec<Token>,
    complete: bool,
}

impl SyntaxCollector {
    fn text(&mut self, text: &str) {
        self.tokens.push(Token::Text(text.into()));
    }

    fn count(&mut self, count: usize) {
        self.tokens.push(Token::Count(count));
    }

    fn flag(&mut self, flag: bool) {
        self.tokens.push(Token::Flag(flag));
    }

    fn name(&mut self, name: Option<Identifier<'_>>) {
        self.flag(name.is_some());
        if let Some(name) = name {
            self.text(name.name());
        }
    }

    fn args(&mut self, args: ArgList<'_>) {
        self.count(args.positional().len());
        self.count(args.named().len());
        for arg in args.named() {
            // Shared traversal groups arguments by kind; retain their source order.
            self.count(
                args.positional()
                    .take_while(|value| value.span().start < arg.span().start)
                    .count(),
            );
            self.text(arg.name().name());
        }
    }

    fn condition(&mut self, condition: IfCondition<'_>) {
        // Pattern conditions cannot occur in a constant collection default.
        if !matches!(condition, IfCondition::Expr(_)) {
            self.complete = false;
        }
    }

    fn enter_expr(&mut self, expr: Expr<'_>) -> bool {
        // Analyzer toSource omits record `const` and trailing commas, so a
        // one-positional-field record has the same spelling as parentheses.
        let parenthesized = matches!(expr, Expr::Paren(_))
            || matches!(expr, Expr::Record(record)
                if record.fields().len() == 1
                    && record.fields().next().unwrap().name().is_none());
        self.tokens.push(if parenthesized {
            Token::Parenthesized
        } else {
            Token::Expr(match expr {
                Expr::StringLit(_) => NodeKind::StringLitNode,
                _ => expr.syntax().kind(),
            })
        });
        match expr {
            Expr::IntLit(value) => self.text(value.value()),
            Expr::DoubleLit(value) => self.text(value.value()),
            Expr::BoolLit(value) => self.flag(value.value()),
            Expr::Unary(value) => self.tokens.push(Token::Unary(value.op())),
            Expr::PostfixIncDec(value) => self.tokens.push(Token::Postfix(value.op())),
            Expr::Binary(value) => self.tokens.push(Token::Binary(value.op())),
            Expr::Assign(value) => self.tokens.push(Token::Assign(value.op())),
            Expr::Is(value) => self.flag(value.negated()),
            Expr::Field(value) => {
                self.text(value.field().name());
                self.flag(value.is_null_safe());
            }
            Expr::Index(value) => self.flag(value.is_null_safe()),
            Expr::Call(value) => {
                self.count(value.type_args().len());
                self.args(value.args());
            }
            Expr::List(value) => {
                self.flag(value.is_const());
                self.flag(value.type_arg().is_some());
                self.count(value.elements().len());
            }
            Expr::Set(value) => {
                self.flag(value.is_const());
                self.flag(value.type_arg().is_some());
                self.count(value.elements().len());
            }
            Expr::Map(value) => {
                self.flag(value.is_const());
                self.count(value.type_args().len());
                self.count(value.elements().len());
            }
            Expr::Record(value) => {
                if !parenthesized {
                    self.count(value.fields().len());
                    for field in value.fields() {
                        self.name(field.name());
                    }
                }
            }
            Expr::New(value) => {
                self.flag(value.is_const());
                self.name(value.constructor_name());
                self.args(value.args());
            }
            Expr::DotShorthand(value) => {
                self.flag(value.is_const());
                self.text(value.name().name());
            }
            Expr::SymbolLit(value) => {
                let raw = value.raw();
                for token in Lexer::new(raw).tokenize() {
                    if !token.is_trivia() && !token.is_empty() {
                        self.text(token.text(raw));
                    }
                }
            }
            Expr::GenericInstantiation(value) => self.count(value.type_args().len()),
            Expr::Error(_) | Expr::FuncExpr(_) | Expr::Cascade(_) | Expr::Switch(_) => {
                self.complete = false;
                return false;
            }
            Expr::StringLit(_)
            | Expr::NullLit(_)
            | Expr::Ident(_)
            | Expr::Paren(_)
            | Expr::This(_)
            | Expr::Super(_)
            | Expr::Conditional(_)
            | Expr::As(_)
            | Expr::Await(_)
            | Expr::Throw(_)
            | Expr::NullAssert(_) => {}
        }
        true
    }
}

impl<'a> Visitor<'a> for SyntaxCollector {
    fn visit_annotation(&mut self, annotation: Annotation<'a>) {
        self.tokens.push(Token::Annotation);
        self.count(annotation.name().count());
        self.count(annotation.type_args().len());
        self.flag(annotation.constructor_name().is_some());
        self.flag(annotation.args().is_some());
        if let Some(args) = annotation.args() {
            self.args(args);
        }
        visitor::walk_annotation(self, annotation);
        self.tokens.push(Token::End);
    }

    fn visit_expr(&mut self, expr: Expr<'a>) {
        visitor::walk_expr_chain_with_exit(self, expr, Self::enter_expr, |this, expr| {
            // Unsupported forms were pruned without a closing token.
            if !matches!(
                expr,
                Expr::Error(_) | Expr::FuncExpr(_) | Expr::Cascade(_) | Expr::Switch(_)
            ) {
                this.tokens.push(Token::End);
            }
        });
    }

    fn visit_identifier(&mut self, identifier: Identifier<'a>) {
        self.text(identifier.name());
    }

    fn visit_string_lit(&mut self, string: StringLitNode<'a>) {
        // Retain each literal's spelling without trivia between adjacent literals.
        let spans = string.segment_spans();
        let single = spans.clone().next().is_none().then_some(string.span());
        self.count(spans.clone().count() + usize::from(single.is_some()));
        let source = string.syntax().source();
        for span in spans.chain(single) {
            let Some(raw) = source.get(span.start..span.end) else {
                self.complete = false;
                return;
            };
            let mut cursor = 0;
            for interpolation in string.interpolations() {
                let interpolation = interpolation.span();
                if interpolation.start < span.start || interpolation.end > span.end {
                    continue;
                }
                let start = interpolation.start - span.start;
                let end = interpolation.end - span.start;
                let Some(fragment) = raw.get(cursor..start) else {
                    self.complete = false;
                    return;
                };
                self.text(fragment);
                cursor = end;
            }
            let Some(fragment) = raw.get(cursor..) else {
                self.complete = false;
                return;
            };
            self.text(fragment);
        }
    }

    fn visit_dart_type(&mut self, dart_type: DartType<'a>) {
        self.tokens.push(Token::Type(match dart_type {
            DartType::Function(_) => NodeKind::FunctionType,
            _ => dart_type.syntax().kind(),
        }));
        match dart_type {
            DartType::Named(named) => {
                self.flag(named.is_nullable());
                self.count(named.segments().count());
                for segment in named.segments() {
                    self.text(segment.name());
                }
                self.count(named.type_args().len());
            }
            DartType::Function(function) => {
                self.flag(function.is_nullable());
                self.flag(function.return_type().is_some());
                self.count(function.type_params().len());
                for param in function.type_params() {
                    self.text(param.name().name());
                    self.flag(param.bound().is_some());
                    if !param.annotations().is_empty() {
                        self.complete = false;
                    }
                }
                self.count(function.params().len());
                for param in function.params() {
                    self.count(param.annotations().len());
                    self.name(param.name());
                    self.flag(param.is_named());
                    self.flag(param.is_required());
                }
            }
            DartType::Record(record) => {
                self.flag(record.is_nullable());
                self.count(record.positional().len());
                self.count(record.named().len());
                for name in record.positional_names() {
                    self.name(name);
                }
                for field in record.named() {
                    self.text(field.name().name());
                }
            }
            DartType::Void(_) | DartType::Dynamic(_) => {}
        }
        visitor::walk_dart_type(self, dart_type);
        self.tokens.push(Token::End);
    }

    fn visit_element<E: Element<'a>>(&mut self, element: E) {
        let kind = element.kind();
        self.tokens.push(if E::IS_MAP {
            Token::MapElement(element.syntax().kind())
        } else {
            Token::Element(match kind {
                ElementKind::Expression(_) => NodeKind::Identifier,
                _ => element.syntax().kind(),
            })
        });
        match kind {
            ElementKind::Entry(entry) => {
                self.flag(entry.key_null_aware());
                self.flag(entry.value_null_aware());
            }
            ElementKind::Spread { .. } => self.flag(element.spread_is_null_aware()),
            ElementKind::If {
                condition,
                else_element,
                ..
            } => {
                self.condition(condition);
                self.flag(else_element.is_some());
            }
            ElementKind::For { .. } => self.complete = false,
            ElementKind::Expression(_) | ElementKind::NullAware(_) => {}
        }
        visitor::walk_element(self, kind);
        self.tokens.push(Token::End);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_dart_parser::parse;

    fn key(source: &str) -> DefaultSyntaxKey {
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = Arc::new(crate::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let program = parsed.program();
        let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
            panic!("expected function");
        };
        default_syntax_key(
            function
                .params()
                .unwrap()
                .params()
                .next()
                .unwrap()
                .default_value()
                .unwrap(),
        )
    }

    fn default_key(expression: &str) -> DefaultSyntaxKey {
        key(&format!("void f([Object? value = {expression}]) {{}}"))
    }

    #[test]
    fn preserves_syntax_not_constant_values() {
        for (left, right) in [
            ("1 + 1", "2"),
            ("1", "(1)"),
            ("1", "0x1"),
            ("'text'", "\"text\""),
            ("'a' 'b'", "'ab'"),
            ("const [1]", "[1]"),
            ("(a: 1,)", "(b: 1,)"),
            ("const C(a: 1)", "const C(b: 1)"),
            ("const C<int>()", "const C<num>()"),
            ("const [1, ...[2]]", "const [1, [2]]"),
            ("'${1}'", "'$one'"),
            ("'$one'", "'${one}'"),
            ("#one", "#two"),
            ("const C(a: 1, 2)", "const C(2, a: 1)"),
            ("identity<(int a,)>", "identity<(int,)>"),
            (
                "identity<void Function(@deprecated int)>",
                "identity<void Function(int)>",
            ),
            (
                "identity<void Function(int)>",
                "identity<void Function([int])>",
            ),
            (
                "identity<void Function(int)>",
                "identity<void Function(int value)>",
            ),
            (
                "identity<void Function({int value})>",
                "identity<void Function({required int value})>",
            ),
        ] {
            assert_ne!(default_key(left), default_key(right), "{left} / {right}");
        }
    }

    #[test]
    fn matches_analyzer_record_source_normalization() {
        for (left, right) in [
            ("const (1, 2)", "(1, 2)"),
            ("(1,)", "(1)"),
            ("const (1,)", "(1)"),
        ] {
            assert_eq!(default_key(left), default_key(right), "{left} / {right}");
        }
    }

    #[test]
    fn ignores_trivia_and_source_offsets() {
        for (left, right) in [
            ("1+2", "1 /* comment */ + 2"),
            ("const C<int>(a: 1)", "const C < int > ( a : 1, )"),
            (
                "const C(a: 1, 2)",
                "const C ( a : 1 , /* before positional */ 2 )",
            ),
            (
                "identity<(int label,)>",
                "identity < ( int /* before name */ label , ) >",
            ),
            (
                "identity<void Function(@deprecated int)>",
                "identity < void Function ( @deprecated int ) >",
            ),
            (
                "const <String, int>{'a': 1}",
                "const < String, int > { 'a' : 1, }",
            ),
            ("(1, a: 2)", "( 1 , a : 2 , )"),
            ("'a' 'b'", "'a' /* between */ 'b'"),
            ("'${1+2}'", "'${ /* before */ 1 + 2 /* after */ }'"),
            ("'${'${1+2}'}'", "'${ '${ 1 + 2 }' }'"),
            ("'${1}' 'b'", "'${ 1 }' /* between */ 'b'"),
            ("#one.two", "#one /* comment */ . two"),
            (
                "identity<void Function(int)>",
                "identity < void Function ( int ) >",
            ),
        ] {
            assert_eq!(default_key(left), default_key(right), "{left} / {right}");
        }
        assert_eq!(
            key("void f([Object? v = 1]) {}"),
            key("\n\n void f([Object? v = 1]) {}")
        );
    }

    #[test]
    fn recovery_never_equates_separately_ingested_defaults() {
        let source = "void f([Object? value = ;]) {}";
        let parsed = parse(source);
        let parsed = Arc::new(crate::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let expression = parsed
            .tree
            .root(&parsed.source)
            .descendants()
            .find_map(|node| Expr::cast(node).filter(|expr| matches!(expr, Expr::Error(_))))
            .expect("expected recovered expression");
        let first = default_syntax_key(expression);
        assert_eq!(first, first.clone());
        assert_ne!(first, default_syntax_key(expression));
    }
}
