//! Flags a multi-line call, declaration, assert, or collection literal that is
//! missing a trailing comma.
//!
//! When the opening and closing delimiters of an argument list, parameter list,
//! `assert`, or list/set/map literal sit on different lines, the last element
//! must be followed by a comma so the formatter keeps one element per line:
//!
//! ```dart
//! foo(a, b);                    // fine: fits on one line
//! foo(
//!   a,
//!   b                           // flagged at `)`
//! );
//! void f({
//!   required int a              // flagged at `}`
//! }) {}
//! ```
//!
//! A positional last element that itself spans several lines is exempt when it
//! is a block-bodied function literal, a string literal, an immediately
//! invoked function literal whose arguments fit on one line, or a list, set, or
//! map literal — the classic `test('name', () { ... });` shape. Named
//! arguments and parameters never get the exemption. Record literals, type
//! argument lists, and extension type representation clauses are not checked.
//!
//! Delimiters inside string interpolations (`'${f(a, b)}'`) are matched like
//! any others.
//!
//! Like the official lint, the rule is off in libraries at language version
//! 3.7 or later, where the formatter's tall style owns trailing commas.
//!
//! Adopted from the official `require_trailing_commas` lint.

use falcon_analyze::{
    AnalyzeContext, Feature, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::token::{Token, TokenKind};

use crate::lint::tokens::significant_tokens;

#[derive(Default)]
pub struct RequireTrailingCommas;

impl Rule for RequireTrailingCommas {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if ctx.language_version().supports(Feature::TallStyle) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            tokens: significant_tokens(ctx).collect(),
            diags: Vec::new(),
        }))
    }
}

/// The last element of a checked list, as far as the exemptions care.
enum Last<'a> {
    /// A positional expression: argument, collection element, assert operand.
    Expr(&'a Expr),
    /// A named argument, map entry, spread, or control-flow element.
    Other,
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    /// Significant tokens in source order.
    tokens: Vec<&'a Token>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    /// Index of the first token of `kind` starting at or after `offset`.
    fn find_from(&self, offset: usize, kind: TokenKind) -> Option<usize> {
        let start = self.tokens.partition_point(|token| token.offset() < offset);
        self.tokens[start..]
            .iter()
            .position(|token| token.kind == kind)
            .map(|index| start + index)
    }

    fn matching_close(&self, open: usize) -> Option<usize> {
        self.matching_delimiter(open, true)
    }

    fn matching_open(&self, close: usize) -> Option<usize> {
        self.matching_delimiter(close, false)
    }

    fn matching_delimiter(&self, index: usize, opening: bool) -> Option<usize> {
        let token = self.tokens[index];
        let span = Span {
            start: token.offset(),
            end: self.end(index),
        };
        let syntax = self.ctx.syntax.syntax_for_span(span);
        let id = syntax.token_range(span).start;
        let paired = if opening {
            syntax.closing_delimiter(id)?
        } else {
            syntax.opening_delimiter(id)?
        };
        let offset = syntax.token(paired)?.offset();
        self.tokens
            .binary_search_by_key(&offset, |token| token.offset())
            .ok()
    }

    fn end(&self, index: usize) -> usize {
        self.tokens[index].offset() + self.tokens[index].len()
    }

    /// Whether the significant source of `span` sits on one line.
    fn on_one_line(&self, span: Span) -> bool {
        self.same_line(span.start, span.end)
    }

    fn same_line(&self, start: usize, end: usize) -> bool {
        !self.ctx.source[start..end.max(start)].contains('\n')
    }

    fn report(&mut self, index: usize) {
        let token = self.tokens[index];
        self.diags.push(self.ctx.diagnostic(
            "require-trailing-commas",
            "Missing a required trailing comma.",
            DiagSpan {
                start: token.offset(),
                end: token.offset() + token.len(),
            },
        ));
    }

    /// Checks an argument list, assert, or collection literal opened at `open`.
    fn check_elements(&mut self, open: Option<usize>, last: Last<'_>) {
        let Some(open) = open else { return };
        let Some(close) = self.matching_close(open) else {
            return;
        };
        let before_close = close - 1;
        if before_close <= open || self.tokens[before_close].kind == TokenKind::Comma {
            return;
        }
        if self.same_line(self.tokens[open].offset(), self.end(close)) {
            return;
        }
        if let Last::Expr(expr) = last
            && self.end(before_close) <= expr.span().end
            && self.is_exempt(expr)
        {
            return;
        }
        self.report(close);
    }

    /// Checks a formal parameter list opened at `open`; the error lands on the
    /// `}`/`]` closing an optional group, else on `)`.
    fn check_params(&mut self, open: Option<usize>) {
        let Some(open) = open else { return };
        let Some(close) = self.matching_close(open) else {
            return;
        };
        if close <= open + 1 {
            return;
        }
        let mut error = close;
        if matches!(
            self.tokens[close - 1].kind,
            TokenKind::RBrace | TokenKind::RBracket
        ) {
            error = close - 1;
            if error <= open + 1 {
                return;
            }
        }
        if self.tokens[error - 1].kind == TokenKind::Comma {
            return;
        }
        if self.same_line(self.tokens[open].offset(), self.end(close)) {
            return;
        }
        self.report(error);
    }

    fn check_param_list(&mut self, params: &FormalParamList) {
        let open = self.find_from(params.span.start, TokenKind::LParen);
        self.check_params(open);
    }

    fn check_args(&mut self, args: &ArgList) {
        let positional = args
            .positional
            .iter()
            .map(|arg| (arg.span().start, Last::Expr(arg)));
        let named = args.named.iter().map(|arg| (arg.span.start, Last::Other));
        let Some((_, last)) = positional.chain(named).max_by_key(|(start, _)| *start) else {
            return;
        };
        let open = self.find_from(args.span.start, TokenKind::LParen);
        self.check_elements(open, last);
    }

    fn check_assert(&mut self, span: &Span, condition: &Expr, message: Option<&Expr>) {
        let open = self.find_from(span.start, TokenKind::LParen);
        self.check_elements(open, Last::Expr(message.unwrap_or(condition)));
    }

    fn check_collection(&mut self, span: &Span, kind: TokenKind, last: Option<&CollectionElement>) {
        let Some(last) = last else { return };
        let last = match last {
            CollectionElement::Expr(expr) => Last::Expr(expr),
            CollectionElement::NullAware { .. }
            | CollectionElement::Spread { .. }
            | CollectionElement::If { .. }
            | CollectionElement::For { .. } => Last::Other,
        };
        let open = self.find_from(span.start, kind);
        self.check_elements(open, last);
    }

    /// The analyzer's exemptions for a multi-line positional last element.
    fn is_exempt(&self, expr: &Expr) -> bool {
        if self.on_one_line(*expr.span()) {
            return false;
        }
        match expr.unparenthesized() {
            Expr::FuncExpr { body, .. } => matches!(**body, FunctionBody::Block(_)),
            Expr::StringLit(_) | Expr::List { .. } | Expr::Set { .. } | Expr::Map { .. } => true,
            Expr::Call {
                callee, args, span, ..
            } if matches!((**callee).unparenthesized(), Expr::FuncExpr { .. })
                && callee.span().start == span.start =>
            {
                self.on_one_line(args.span)
            }
            _ => false,
        }
    }
}

structural_visitor! {
impl StructuralRuleVisitor for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        match node {
            Expr::Call { args, .. } | Expr::New { args, .. } => self.check_args(args),
            Expr::Cascade { sections, .. } => {
                for op in sections.iter().flat_map(|section| &section.ops) {
                    if let CascadeOp::Call(_, _, args) | CascadeOp::Invoke(_, args) = op {
                        self.check_args(args);
                    }
                }
            }
            Expr::List { elements, span, .. } => {
                self.check_collection(span, TokenKind::LBracket, elements.last());
            }
            Expr::Set { elements, span, .. } => {
                self.check_collection(span, TokenKind::LBrace, elements.last());
            }
            Expr::Map { elements, span, .. } => {
                if !elements.is_empty() {
                    let open = self.find_from(span.start, TokenKind::LBrace);
                    self.check_elements(open, Last::Other);
                }
            }
            Expr::FuncExpr { params, .. } => self.check_param_list(params),
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn visit_annotation(&mut self, node: &Annotation) -> StructuralTraversal {
        if let Some(args) = &node.args {
            self.check_args(args);
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'_>) -> StructuralTraversal {
        self.check_param_list(node.params);
        for initializer in node.initializers {
            match initializer {
                ConstructorInitializer::SuperCall { args, .. }
                | ConstructorInitializer::ThisCall { args, .. } => self.check_args(args),
                ConstructorInitializer::Assert {
                    condition,
                    message,
                    span,
                } => self.check_assert(span, condition, message.as_ref()),
                ConstructorInitializer::FieldInit { .. } => {}
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_enum_decl(&mut self, node: &EnumDecl) -> StructuralTraversal {
        for args in node
            .variants
            .iter()
            .filter_map(|variant| variant.args.as_ref())
        {
            self.check_args(args);
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        match node {
            Stmt::Assert(stmt) => {
                self.check_assert(&stmt.span, &stmt.condition, stmt.message.as_ref());
            }
            Stmt::LocalFunc(func) => self.check_param_list(&func.params),
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) -> StructuralTraversal {
        if let Some(params) = node.params() {
            self.check_param_list(params);
        }
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) -> StructuralTraversal {
        self.check_param_list(&node.params);
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        match node {
            ClassMember::Operator(operator) => self.check_param_list(&operator.params),
            ClassMember::Setter(setter) => {
                let open = self.find_from(setter.name.span.end, TokenKind::LParen);
                self.check_params(open);
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn visit_formal_param(&mut self, node: &FormalParam) -> StructuralTraversal {
        if let Some(params) = &node.function_params {
            self.check_param_list(params);
        }
        StructuralTraversal::Continue
    }

    fn visit_dart_type(&mut self, node: &DartType) -> StructuralTraversal {
        if let DartType::Function(function) = node
            && !function.params.is_empty()
        {
            let span = function.span;
            let end = self
                .tokens
                .partition_point(|token| token.offset() < span.end);
            let mut close = end.checked_sub(1);
            // A legacy `typedef void F(...);` type span runs through its `;`.
            if let Some(index) = close
                && self.tokens[index].kind == TokenKind::Semicolon
            {
                close = index.checked_sub(1);
            }
            if function.is_nullable
                && let Some(index) = close
                && self.tokens[index].kind == TokenKind::Qmark
            {
                close = index.checked_sub(1);
            }
            if let Some(close) = close
                && self.tokens[close].kind == TokenKind::RParen
                && self.tokens[close].offset() >= span.start
            {
                let open = self.matching_open(close);
                self.check_params(open);
            }
        }
        StructuralTraversal::Continue
    }
}
}
