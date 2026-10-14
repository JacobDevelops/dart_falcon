//! Declaration bodies stay separate from nested functions; statement lists include all closures.

use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_expr, walk_function_decl, walk_stmt,
};

pub(super) fn for_each_declared_body(
    program: &Program,
    source: &str,
    f: &mut impl FnMut(&FunctionBody, &Span, &Span),
) {
    struct Bodies<'a, F> {
        source: &'a str,
        callback: &'a mut F,
    }
    impl<F: FnMut(&FunctionBody, &Span, &Span)> Visitor for Bodies<'_, F> {
        fn visit_function_decl(&mut self, node: &FunctionDecl) {
            if let Some(body) = &node.body {
                (self.callback)(body, &node.name.span, &node.span);
            }
        }

        fn visit_class_member(&mut self, node: &ClassMember) {
            if let ClassMember::Operator(operator) = node {
                if let Some(body) = &operator.body {
                    let name = operator_header_span(operator, self.source);
                    (self.callback)(body, &name, &operator.span);
                }
                return;
            }
            let (body, name, span) = match node {
                ClassMember::Method(n) => (&n.body, &n.name.span, &n.span),
                ClassMember::Constructor(n) => (&n.body, &n.name.span, &n.span),
                ClassMember::Getter(n) => (&n.body, &n.name.span, &n.span),
                ClassMember::Setter(n) => (&n.body, &n.name.span, &n.span),
                ClassMember::Field(_) | ClassMember::Operator(_) | ClassMember::Error(_) => return,
            };
            if let Some(body) = body {
                (self.callback)(body, name, span);
            }
        }
    }
    Bodies {
        source,
        callback: f,
    }
    .visit_program(program);
}

// Annotations and comments must not shift operator diagnostic suppression.
fn operator_header_span(node: &OperatorDecl, source: &str) -> Span {
    let start = node
        .annotations
        .last()
        .map_or(node.span.start, |a| a.span.end);
    let header = &source[start..node.params.span.start];
    let mut offset = 0;
    let mut keyword = start;
    while offset < header.len() {
        let rest = &header[offset..];
        if rest.starts_with("//") {
            offset += rest.find('\n').unwrap_or(rest.len());
        } else if rest.starts_with("/*") {
            offset += 2;
            let mut depth = 1;
            while offset < header.len() && depth > 0 {
                if header[offset..].starts_with("/*") {
                    depth += 1;
                    offset += 2;
                } else if header[offset..].starts_with("*/") {
                    depth -= 1;
                    offset += 2;
                } else {
                    offset += header[offset..].chars().next().unwrap().len_utf8();
                }
            }
        } else {
            let ch = rest.chars().next().unwrap();
            if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                let end = rest
                    .char_indices()
                    .find(|(_, c)| !c.is_alphanumeric() && *c != '_' && *c != '$')
                    .map_or(rest.len(), |(i, _)| i);
                if &rest[..end] == "operator" {
                    keyword = start + offset;
                }
                offset += end;
            } else {
                offset += ch.len_utf8();
            }
        }
    }
    Span {
        start: keyword,
        end: (keyword + "operator".len()).min(node.params.span.start),
    }
}

pub(super) fn for_each_stmt_list(program: &Program, f: &mut impl FnMut(&[Stmt])) {
    struct Lists<'a, F>(&'a mut F);
    impl<F: FnMut(&[Stmt])> Lists<'_, F> {
        fn body(&mut self, body: &FunctionBody) {
            if let FunctionBody::Block(block) = body {
                (self.0)(&block.stmts);
            }
        }
    }
    impl<F: FnMut(&[Stmt])> Visitor for Lists<'_, F> {
        fn visit_function_decl(&mut self, node: &FunctionDecl) {
            if let Some(body) = &node.body {
                self.body(body);
            }
            walk_function_decl(self, node);
        }

        fn visit_class_member(&mut self, node: &ClassMember) {
            let body = match node {
                ClassMember::Method(n) => n.body.as_ref(),
                ClassMember::Constructor(n) => n.body.as_ref(),
                ClassMember::Getter(n) => n.body.as_ref(),
                ClassMember::Setter(n) => n.body.as_ref(),
                ClassMember::Operator(n) => n.body.as_ref(),
                ClassMember::Field(_) | ClassMember::Error(_) => None,
            };
            if let Some(body) = body {
                self.body(body);
            }
            walk_class_member(self, node);
        }

        fn visit_stmt(&mut self, node: &Stmt) {
            match node {
                Stmt::Block(block) => (self.0)(&block.stmts),
                Stmt::LocalFunc(n) => self.body(&n.body),
                Stmt::TryCatch(n) => {
                    (self.0)(&n.body.stmts);
                    for catch in &n.catches {
                        (self.0)(&catch.body.stmts);
                    }
                    if let Some(block) = &n.finally {
                        (self.0)(&block.stmts);
                    }
                }
                Stmt::Switch(n) => {
                    for case in &n.cases {
                        (self.0)(&case.body);
                    }
                }
                _ => {}
            }
            walk_stmt(self, node);
        }

        fn visit_expr(&mut self, node: &Expr) {
            if let Expr::FuncExpr { body, .. } = node {
                self.body(body);
            }
            walk_expr(self, node);
        }
    }
    Lists(f).visit_program(program);
}
