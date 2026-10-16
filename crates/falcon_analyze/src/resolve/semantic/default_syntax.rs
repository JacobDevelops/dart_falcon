//! Span-independent syntax retained for analyzer `defaultValueCode` equality.

use std::{
    mem::{Discriminant, discriminant},
    sync::Arc,
};

use falcon_dart_parser::lexer::Lexer;
use falcon_syntax::{
    ast::*,
    visitor::{self, Visitor},
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
    Expr(Discriminant<Expr>),
    Parenthesized,
    Annotation,
    Type(Discriminant<DartType>),
    Element(Discriminant<CollectionElement>),
    MapElement(Discriminant<MapElement>),
    Text(Box<str>),
    Count(usize),
    Flag(bool),
    Unary(UnaryOp),
    Binary(BinaryOp),
    Assign(AssignOp),
    Postfix(PostfixIncDec),
    End,
}

pub fn default_syntax_key(expr: &Expr) -> DefaultSyntaxKey {
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

    fn name(&mut self, name: Option<&Identifier>) {
        self.flag(name.is_some());
        if let Some(name) = name {
            self.text(&name.name);
        }
    }

    fn args(&mut self, args: &ArgList) {
        self.count(args.positional.len());
        self.count(args.named.len());
        for arg in &args.named {
            // Shared traversal groups arguments by kind; retain their source order.
            self.count(
                args.positional
                    .partition_point(|value| value.span().start < arg.span.start),
            );
            self.text(&arg.name.name);
        }
    }

    fn condition(&mut self, condition: &IfCondition) {
        // Pattern conditions cannot occur in a constant collection default.
        if !matches!(condition, IfCondition::Expr(_)) {
            self.complete = false;
        }
    }
}

impl Visitor for SyntaxCollector {
    fn visit_annotation(&mut self, annotation: &Annotation) {
        self.tokens.push(Token::Annotation);
        self.count(annotation.name.len());
        self.count(annotation.type_args.len());
        self.flag(annotation.constructor_name.is_some());
        self.flag(annotation.args.is_some());
        if let Some(args) = &annotation.args {
            self.args(args);
        }
        visitor::walk_annotation(self, annotation);
        self.tokens.push(Token::End);
    }

    fn visit_expr(&mut self, expr: &Expr) {
        // Analyzer toSource omits record `const` and trailing commas, so a
        // one-positional-field record has the same spelling as parentheses.
        let parenthesized = matches!(expr, Expr::Paren { .. })
            || matches!(expr, Expr::Record { fields, .. }
                if fields.len() == 1 && fields[0].name.is_none());
        self.tokens.push(if parenthesized {
            Token::Parenthesized
        } else {
            Token::Expr(discriminant(expr))
        });
        match expr {
            Expr::IntLit { value, .. } | Expr::DoubleLit { value, .. } => self.text(value),
            Expr::BoolLit { value, .. } => self.flag(*value),
            Expr::Unary { op, .. } => self.tokens.push(Token::Unary(*op)),
            Expr::PostfixIncDec { op, .. } => self.tokens.push(Token::Postfix(*op)),
            Expr::Binary { op, .. } => self.tokens.push(Token::Binary(*op)),
            Expr::Assign { op, .. } => self.tokens.push(Token::Assign(*op)),
            Expr::Is { negated, .. } => self.flag(*negated),
            Expr::Field {
                field,
                is_null_safe,
                ..
            } => {
                self.text(&field.name);
                self.flag(*is_null_safe);
            }
            Expr::Index { is_null_safe, .. } => self.flag(*is_null_safe),
            Expr::Call {
                type_args, args, ..
            } => {
                self.count(type_args.len());
                self.args(args);
            }
            Expr::List {
                is_const,
                type_arg,
                elements,
                ..
            }
            | Expr::Set {
                is_const,
                type_arg,
                elements,
                ..
            } => {
                self.flag(*is_const);
                self.flag(type_arg.is_some());
                self.count(elements.len());
            }
            Expr::Map {
                is_const,
                type_args,
                elements,
                ..
            } => {
                self.flag(*is_const);
                self.count(type_args.len());
                self.count(elements.len());
            }
            Expr::Record { fields, .. } => {
                if !parenthesized {
                    self.count(fields.len());
                    for field in fields {
                        self.name(field.name.as_ref());
                    }
                }
            }
            Expr::New {
                is_const,
                constructor_name,
                args,
                ..
            } => {
                self.flag(*is_const);
                self.name(constructor_name.as_ref());
                self.args(args);
            }
            Expr::DotShorthand { is_const, name, .. } => {
                self.flag(*is_const);
                self.text(&name.name);
            }
            Expr::SymbolLit { raw, .. } => {
                for token in Lexer::new(raw).tokenize() {
                    if !token.is_trivia() && !token.is_empty() {
                        self.text(token.text(raw));
                    }
                }
            }
            Expr::GenericInstantiation { type_args, .. } => self.count(type_args.len()),
            Expr::Error { .. }
            | Expr::FuncExpr { .. }
            | Expr::Cascade { .. }
            | Expr::Switch { .. } => {
                self.complete = false;
                return;
            }
            Expr::StringLit(_)
            | Expr::NullLit { .. }
            | Expr::Ident(_)
            | Expr::Paren { .. }
            | Expr::This { .. }
            | Expr::Super { .. }
            | Expr::Conditional { .. }
            | Expr::As { .. }
            | Expr::Await { .. }
            | Expr::Throw { .. }
            | Expr::NullAssert { .. } => {}
        }
        visitor::walk_expr(self, expr);
        self.tokens.push(Token::End);
    }

    fn visit_identifier(&mut self, identifier: &Identifier) {
        self.text(&identifier.name);
    }

    fn visit_string_lit(&mut self, string: &StringLitNode) {
        // Adjacent segment spans map absolute interpolation positions into the
        // concatenated raw spelling without retaining trivia between literals.
        let mut raw_offset = 0;
        let spans = if string.segment_spans.is_empty() {
            std::slice::from_ref(&string.span)
        } else {
            &string.segment_spans
        };
        self.count(spans.len());
        for span in spans {
            let length = span.end - span.start;
            let Some(raw) = string.raw.get(raw_offset..raw_offset + length) else {
                self.complete = false;
                return;
            };
            let mut cursor = 0;
            for interpolation in &string.interpolations {
                if interpolation.span.start < span.start || interpolation.span.end > span.end {
                    continue;
                }
                let start = interpolation.span.start - span.start;
                let end = interpolation.span.end - span.start;
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
            raw_offset += length;
        }
    }

    fn visit_dart_type(&mut self, dart_type: &DartType) {
        self.tokens.push(Token::Type(discriminant(dart_type)));
        match dart_type {
            DartType::Named(named) => {
                self.flag(named.is_nullable);
                self.count(named.segments.len());
                for segment in &named.segments {
                    self.text(&segment.name);
                }
                self.count(named.type_args.len());
            }
            DartType::Function(function) => {
                self.flag(function.is_nullable);
                self.flag(function.return_type.is_some());
                self.count(function.type_params.len());
                for param in &function.type_params {
                    self.text(&param.name.name);
                    self.flag(param.bound.is_some());
                    if !param.annotations.is_empty() {
                        self.complete = false;
                    }
                }
                self.count(function.params.len());
                for param in &function.params {
                    self.count(param.annotations.len());
                    self.name(param.name.as_ref());
                    self.flag(param.is_named);
                    self.flag(param.is_required);
                }
            }
            DartType::Record(record) => {
                self.flag(record.is_nullable);
                self.count(record.positional.len());
                self.count(record.named.len());
                for name in &record.positional_names {
                    self.name(name.as_ref());
                }
                for field in &record.named {
                    self.text(&field.name.name);
                }
            }
            DartType::Void { .. } | DartType::Dynamic { .. } => {}
        }
        visitor::walk_dart_type(self, dart_type);
        self.tokens.push(Token::End);
    }

    fn visit_collection_element(&mut self, element: &CollectionElement) {
        self.tokens.push(Token::Element(discriminant(element)));
        match element {
            CollectionElement::Spread { is_null_aware, .. } => self.flag(*is_null_aware),
            CollectionElement::If {
                condition,
                else_elem,
                ..
            } => {
                self.condition(condition);
                self.flag(else_elem.is_some());
            }
            CollectionElement::For { .. } => self.complete = false,
            CollectionElement::Expr(_) | CollectionElement::NullAware { .. } => {}
        }
        visitor::walk_collection_element(self, element);
        self.tokens.push(Token::End);
    }

    fn visit_map_element(&mut self, element: &MapElement) {
        self.tokens.push(Token::MapElement(discriminant(element)));
        match element {
            MapElement::Entry(entry) => {
                self.flag(entry.key_null_aware);
                self.flag(entry.value_null_aware);
            }
            MapElement::Spread { is_null_aware, .. } => self.flag(*is_null_aware),
            MapElement::If {
                condition,
                else_entry,
                ..
            } => {
                self.condition(condition);
                self.flag(else_entry.is_some());
            }
            MapElement::For { .. } => self.complete = false,
        }
        visitor::walk_map_element(self, element);
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
        let program = parsed.program;
        let TopLevelDecl::Function(function) = &program.declarations[0] else {
            panic!("expected function");
        };
        default_syntax_key(
            function.params().unwrap().params[0]
                .default_value
                .as_ref()
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
        let expression = Expr::Error {
            span: Span::new(0, 0),
        };
        let first = default_syntax_key(&expression);
        assert_eq!(first, first.clone());
        assert_ne!(first, default_syntax_key(&expression));
    }
}
