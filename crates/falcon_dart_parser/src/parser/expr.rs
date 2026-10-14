use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;

use super::Parser;

#[derive(Clone, Copy)]
pub(super) enum Brackets {
    ParenOnly,
    BraceOnly,
    Mixed,
}

impl<'src> Parser<'src> {
    pub(super) fn matching_close(
        &self,
        open: usize,
        brackets: Brackets,
        bail: fn(TokenKind) -> bool,
        max_scan: Option<usize>,
    ) -> Option<usize> {
        let mut depth = 0i32;
        for (offset, token) in self.tokens.get(open..)?.iter().enumerate() {
            if max_scan.is_some_and(|limit| offset >= limit) || token.kind == TokenKind::Eof {
                return None;
            }
            let delta = match (brackets, token.kind) {
                (Brackets::ParenOnly, TokenKind::LParen)
                | (Brackets::BraceOnly, TokenKind::LBrace)
                | (Brackets::Mixed, TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace) => {
                    1
                }
                (Brackets::ParenOnly, TokenKind::RParen)
                | (Brackets::BraceOnly, TokenKind::RBrace)
                | (Brackets::Mixed, TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace) => {
                    -1
                }
                _ => {
                    if bail(token.kind) {
                        return None;
                    }
                    continue;
                }
            };
            depth += delta;
            if depth == 0 {
                return Some(open + offset);
            }
        }
        None
    }

    pub(super) fn matching_angle_close(
        &self,
        open: usize,
        allowed: fn(TokenKind) -> bool,
        max_scan: Option<usize>,
    ) -> Option<usize> {
        let mut depth = 0i32;
        for (offset, token) in self.tokens.get(open..)?.iter().enumerate() {
            if max_scan.is_some_and(|limit| offset >= limit) || token.kind == TokenKind::Eof {
                return None;
            }
            match token.kind {
                TokenKind::Lt => depth += 1,
                TokenKind::Gt => depth -= 1,
                TokenKind::GtGt => depth -= 2,
                TokenKind::GtGtGt => depth -= 3,
                kind if allowed(kind) => {}
                _ => return None,
            }
            if depth <= 0 {
                return Some(open + offset);
            }
        }
        None
    }

    pub(super) fn parse_adjacent_strings(&mut self) -> StringLitNode {
        let mut node = self.parse_string_lit();
        while self.at(TokenKind::StringLit) {
            let next = self.parse_string_lit();
            node.is_adjacent = true;
            node.has_raw_segment |= next.has_raw_segment;
            node.raw.push_str(&next.raw);
            node.value.push_str(&next.value);
            node.span = node.span.merge(&next.span);
            node.interpolations.extend(next.interpolations);
        }
        node
    }

    fn parse_switch_expr_arms(&mut self) -> Vec<SwitchExprArm> {
        self.expect(TokenKind::LBrace);
        let mut arms = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let start = self.cur().offset;
            let pattern = self.parse_pattern();
            let guard = self.eat(TokenKind::When).map(|_| self.parse_expr());
            self.expect(TokenKind::Arrow);
            let body = self.parse_expr();
            arms.push(SwitchExprArm {
                pattern,
                guard,
                body,
                span: self.span_from(start),
            });
            self.eat(TokenKind::Comma);
        }
        self.expect(TokenKind::RBrace);
        arms
    }

    fn parse_typed_collection(&mut self, is_const: bool, start: usize) -> Expr {
        let type_args = self.parse_type_args();
        match self.cur().kind {
            TokenKind::LBracket => {
                self.parse_list_literal(is_const, type_args.into_iter().next(), start)
            }
            TokenKind::LBrace => self.parse_map_or_set_literal(is_const, type_args, start),
            _ => {
                self.error("expected '[' or '{' after type arguments in collection literal");
                Expr::Error {
                    span: self.span_from(start),
                }
            }
        }
    }

    pub(super) fn parse_expr(&mut self) -> Expr {
        self.parse_assign()
    }

    fn parse_assign(&mut self) -> Expr {
        let start = self.cur().offset;
        let lhs = self.parse_cascade();

        let Some(op) = self.try_parse_assign_op() else {
            return lhs;
        };
        let rhs = self.parse_assign(); // right-associative
        let span = self.span_from(start);
        Expr::Assign {
            target: Box::new(lhs),
            op,
            value: Box::new(rhs),
            span,
        }
    }

    /// Cascade binds looser than every operator except assignment, so it wraps a
    /// whole conditional/relational/cast subject: `x as T ..m()` is `(x as T)..m()`
    /// and `a = b..c` is `a = (b..c)`. Parsed here — above the operator ladder —
    /// rather than in `parse_postfix`, which only ever sees a primary target.
    fn parse_cascade(&mut self) -> Expr {
        let start = self.cur().offset;
        let object = self.parse_conditional();
        if matches!(self.cur().kind, TokenKind::DotDot | TokenKind::DotDotQmark) {
            let is_null_aware = self.at(TokenKind::DotDotQmark);
            let sections = self.parse_cascade_sections();
            return Expr::Cascade {
                object: Box::new(object),
                sections,
                is_null_aware,
                span: self.span_from(start),
            };
        }
        object
    }

    fn parse_conditional(&mut self) -> Expr {
        let start = self.cur().offset;
        let cond = self.parse_null_coalesce();
        if self.eat(TokenKind::Qmark).is_some() {
            // Both branches are `expressionWithoutCascade` in the Dart grammar,
            // which *includes* assignment — `a ? b = c : d = e` is valid, with each
            // branch a right-associative assignment. (Cascades are excluded; a
            // cascade in a branch must be parenthesised.)
            let then_expr = self.parse_expr_without_cascade();
            self.expect(TokenKind::Colon);
            let else_expr = self.parse_expr_without_cascade();
            let span = self.span_from(start);
            return Expr::Conditional {
                condition: Box::new(cond),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
                span,
            };
        }
        cond
    }

    /// Parse an `expressionWithoutCascade`: an assignment whose left-hand side is a
    /// conditional (cascade-excluding) subject. Used for the branches of a ternary,
    /// where Dart permits assignment but not a bare cascade.
    fn parse_expr_without_cascade(&mut self) -> Expr {
        let start = self.cur().offset;
        let lhs = self.parse_conditional();
        let Some(op) = self.try_parse_assign_op() else {
            return lhs;
        };
        let rhs = self.parse_expr_without_cascade(); // right-associative
        Expr::Assign {
            target: Box::new(lhs),
            op,
            value: Box::new(rhs),
            span: self.span_from(start),
        }
    }

    #[inline]
    fn parse_left_assoc(
        &mut self,
        next: fn(&mut Self) -> Expr,
        operator: fn(TokenKind) -> Option<BinaryOp>,
    ) -> Expr {
        let start = self.cur().offset;
        let mut lhs = next(self);
        while let Some(op) = operator(self.cur().kind) {
            self.advance();
            let rhs = next(self);
            lhs = Expr::Binary {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
                span: self.span_from(start),
            };
        }
        lhs
    }

    fn parse_null_coalesce(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_or, |kind| match kind {
            TokenKind::QmarkQmark => Some(BinaryOp::NullCoalesce),
            _ => None,
        })
    }

    fn parse_or(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_and, |kind| match kind {
            TokenKind::PipePipe => Some(BinaryOp::Or),
            _ => None,
        })
    }

    fn parse_and(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_equality, |kind| match kind {
            TokenKind::AmpAmp => Some(BinaryOp::And),
            _ => None,
        })
    }

    fn parse_equality(&mut self) -> Expr {
        let start = self.cur().offset;
        let lhs = self.parse_relational();
        let op = match self.cur().kind {
            TokenKind::EqEq => BinaryOp::EqEq,
            TokenKind::BangEq => BinaryOp::NotEq,
            _ => return lhs,
        };
        self.advance();
        let rhs = self.parse_relational();
        Expr::Binary {
            op,
            left: Box::new(lhs),
            right: Box::new(rhs),
            span: self.span_from(start),
        }
    }

    /// Scan a balanced `<...>` starting at the cursor (which must be `<`),
    /// returning the token index immediately *after* the closing `>` when the
    /// span contains only type-ish tokens, or `None` when some token means the
    /// `<` is really a comparison operator. Shared by generic-invocation and
    /// generic-instantiation detection.
    fn after_generic_type_args(&self) -> Option<usize> {
        use TokenKind::*;
        debug_assert_eq!(self.cur().kind, Lt);
        let mut depth = 0i32;
        // Angle depth captured at each open `(`/`[`/`{`. Records and function types
        // nest inside type arguments (`Foo<(int, Bar<T>)>`, `Foo<int Function()>`),
        // and their own generics must be tracked — but a `>`/`>>` may never close an
        // angle opened *outside* the current bracket group. That would be an
        // expression `>`/shift (`a < b ? (d >> e) : 0`), not a type-arg close.
        let mut bracket_stack: Vec<i32> = Vec::new();
        let mut i = self.pos;
        while let Some(tok) = self.tokens.get(i) {
            match &tok.kind {
                Lt => depth += 1,
                Gt | GtGt | GtGtGt => {
                    let dec = match tok.kind {
                        Gt => 1,
                        GtGt => 2,
                        _ => 3,
                    };
                    let base = bracket_stack.last().copied().unwrap_or(0);
                    if depth - dec < base {
                        return None;
                    }
                    depth -= dec;
                    if bracket_stack.is_empty() && depth <= 0 {
                        return Some(i + 1);
                    }
                }
                LParen | LBracket | LBrace => bracket_stack.push(depth),
                RParen | RBracket | RBrace => match bracket_stack.pop() {
                    // A type-arg list's brackets are angle-balanced within: the
                    // depth on close must equal the depth captured on open.
                    Some(base) if base == depth => {}
                    _ => return None,
                },
                // Tokens that legitimately appear inside a type-argument list
                // (named/qualified types, records, function types, nullability).
                Dot | Comma | Qmark | Void => {}
                k if k.is_identifier_like() => {}
                // Anything else (operators, literals, `=>`, `:`, `;`, …) means
                // this `<` is a comparison, not type arguments.
                _ => return None,
            }
            i += 1;
            if i - self.pos > 512 {
                return None;
            }
        }
        None
    }

    /// True when the `<` at the cursor opens a generic-invocation type-argument
    /// list — a balanced `<...>` closed immediately before `(`, `.`, or `?.`.
    /// Distinguishes `f<T>()` from the comparison in `a < b ? () => c() : null`.
    fn is_generic_invocation(&self) -> bool {
        self.after_generic_type_args()
            .is_some_and(|idx| self.after_type_args_is_call(idx))
    }

    /// True when the `<` at the cursor opens a bare generic-instantiation
    /// tear-off (`identity<int>`) — a balanced `<...>` *not* followed by a call
    /// token, whose following token cannot continue a `<` comparison (Dart's own
    /// disambiguation rule, per the constructor-tearoffs spec).
    fn is_generic_instantiation(&self) -> bool {
        match self.after_generic_type_args() {
            Some(idx) if !self.after_type_args_is_call(idx) => {
                self.token_ends_generic_instantiation(idx)
            }
            _ => false,
        }
    }

    fn after_type_args_is_call(&self, idx: usize) -> bool {
        matches!(
            self.tokens.get(idx).map(|t| &t.kind),
            Some(TokenKind::LParen | TokenKind::Dot | TokenKind::QmarkDot)
        )
    }

    /// True when the token at `idx` (following a balanced `<...>`) cannot begin
    /// or continue the right-hand side of a `<` comparison, so the `<...>` must
    /// be a generic-instantiation type-argument list. `f(a < b, c > d)` stays a
    /// comparison because an identifier follows `>`, which is *not* in this set.
    fn token_ends_generic_instantiation(&self, idx: usize) -> bool {
        use TokenKind::*;
        matches!(
            self.tokens.get(idx).map(|t| &t.kind),
            Some(
                RParen
                    | RBracket
                    | RBrace
                    | Colon
                    | Semicolon
                    | Comma
                    | Qmark
                    | QmarkQmark
                    | Eq
                    | EqEq
                    | BangEq
                    | DotDot
                    | DotDotQmark
                    | AmpAmp
                    | PipePipe
                    | Eof
            ) | None
        )
    }

    /// Parse the type after `is`/`as`, leaving a `?` that begins a conditional
    /// (`x is T ? a : b`) for the enclosing ternary rather than eating it as a
    /// nullable-type suffix.
    fn parse_type_after_is_as(&mut self) -> DartType {
        self.with_ctx(
            |ctx| ctx.suppress_conditional_qmark = true,
            Self::parse_type,
        )
    }

    fn parse_relational(&mut self) -> Expr {
        let start = self.cur().offset;
        let lhs = self.parse_cast();
        let op = match self.cur().kind {
            TokenKind::Lt => BinaryOp::Lt,
            TokenKind::Gt => BinaryOp::Gt,
            TokenKind::LtEq => BinaryOp::LtEq,
            TokenKind::GtEq => BinaryOp::GtEq,
            _ => return lhs,
        };
        self.advance();
        let r = self.parse_cast();
        Expr::Binary {
            op,
            left: Box::new(lhs),
            right: Box::new(r),
            span: self.span_from(start),
        }
    }

    /// Type test / cast level (`is`, `is!`, `as`), which binds tighter than the
    /// relational comparison operators — so `a < b as C` is `a < (b as C)` and
    /// `x as int <= y` is `(x as int) <= y`. Loops to allow chains (`a as B as C`).
    fn parse_cast(&mut self) -> Expr {
        let start = self.cur().offset;
        let mut lhs = self.parse_bitwise_or();
        loop {
            match self.cur().kind {
                TokenKind::Is => {
                    self.advance();
                    let negated = self.eat(TokenKind::Bang).is_some();
                    let dart_type = self.parse_type_after_is_as();
                    lhs = Expr::Is {
                        expr: Box::new(lhs),
                        dart_type,
                        negated,
                        span: self.span_from(start),
                    };
                }
                TokenKind::As => {
                    self.advance();
                    let dart_type = self.parse_type_after_is_as();
                    lhs = Expr::As {
                        expr: Box::new(lhs),
                        dart_type,
                        span: self.span_from(start),
                    };
                }
                _ => break,
            }
        }
        lhs
    }

    pub(super) fn parse_bitwise_or(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_bitwise_xor, |kind| match kind {
            TokenKind::Pipe => Some(BinaryOp::BitOr),
            _ => None,
        })
    }

    fn parse_bitwise_xor(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_bitwise_and, |kind| match kind {
            TokenKind::Caret => Some(BinaryOp::BitXor),
            _ => None,
        })
    }

    fn parse_bitwise_and(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_shift, |kind| match kind {
            TokenKind::Amp => Some(BinaryOp::BitAnd),
            _ => None,
        })
    }

    fn parse_shift(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_additive, |kind| match kind {
            TokenKind::LtLt => Some(BinaryOp::Shl),
            TokenKind::GtGt => Some(BinaryOp::Shr),
            TokenKind::GtGtGt => Some(BinaryOp::UShr),
            _ => None,
        })
    }

    fn parse_additive(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_multiplicative, |kind| match kind {
            TokenKind::Plus => Some(BinaryOp::Add),
            TokenKind::Minus => Some(BinaryOp::Sub),
            _ => None,
        })
    }

    fn parse_multiplicative(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_unary, |kind| match kind {
            TokenKind::Star => Some(BinaryOp::Mul),
            TokenKind::Slash => Some(BinaryOp::Div),
            TokenKind::Percent => Some(BinaryOp::Mod),
            TokenKind::TildeSlash => Some(BinaryOp::IntDiv),
            _ => None,
        })
    }

    fn parse_unary(&mut self) -> Expr {
        self.guarded(
            |p| Expr::Error { span: p.cur_span() },
            |p| {
                let start = p.cur().offset;
                let op = match p.cur().kind {
                    TokenKind::Minus => Some(UnaryOp::Minus),
                    TokenKind::Bang => Some(UnaryOp::Bang),
                    TokenKind::Tilde => Some(UnaryOp::Tilde),
                    TokenKind::PlusPlus => Some(UnaryOp::PlusPlus),
                    TokenKind::MinusMinus => Some(UnaryOp::MinusMinus),
                    _ => None,
                };
                if let Some(op) = op {
                    p.advance();
                    let operand = p.parse_unary();
                    return Expr::Unary {
                        op,
                        operand: Box::new(operand),
                        span: p.span_from(start),
                    };
                }
                match p.cur().kind {
                    TokenKind::Await if p.ctx.body.is_async() => {
                        p.advance();
                        let operand = p.parse_unary();
                        Expr::Await {
                            expr: Box::new(operand),
                            span: p.span_from(start),
                        }
                    }
                    TokenKind::Throw => {
                        p.advance();
                        let operand = p.parse_expr();
                        Expr::Throw {
                            expr: Box::new(operand),
                            span: p.span_from(start),
                        }
                    }
                    _ => p.parse_postfix(),
                }
            },
        )
    }

    fn parse_postfix(&mut self) -> Expr {
        self.guarded(
            |p| Expr::Error { span: p.cur_span() },
            |p| {
                let start = p.cur().offset;
                let mut expr = p.parse_primary();

                loop {
                    match p.cur().kind {
                        TokenKind::PlusPlus => {
                            p.advance();
                            expr = Expr::PostfixIncDec {
                                op: PostfixIncDec::Increment,
                                operand: Box::new(expr),
                                span: p.span_from(start),
                            };
                        }
                        TokenKind::MinusMinus => {
                            p.advance();
                            expr = Expr::PostfixIncDec {
                                op: PostfixIncDec::Decrement,
                                operand: Box::new(expr),
                                span: p.span_from(start),
                            };
                        }
                        TokenKind::Bang => {
                            p.advance();
                            expr = Expr::NullAssert {
                                operand: Box::new(expr),
                                span: p.span_from(start),
                            };
                        }
                        TokenKind::Dot | TokenKind::QmarkDot => {
                            let is_null_safe = p.cur().kind == TokenKind::QmarkDot;
                            p.advance();
                            let field = p.expect_ctor_name();
                            expr = Expr::Field {
                                object: Box::new(expr),
                                field,
                                is_null_safe,
                                span: p.span_from(start),
                            };
                        }
                        TokenKind::LBracket | TokenKind::QmarkLBracket => {
                            let is_null_safe = p.cur().kind == TokenKind::QmarkLBracket;
                            p.advance();
                            let index = p.parse_expr();
                            p.expect(TokenKind::RBracket);
                            expr = Expr::Index {
                                object: Box::new(expr),
                                index: Box::new(index),
                                is_null_safe,
                                span: p.span_from(start),
                            };
                        }
                        TokenKind::LParen => {
                            let args = p.parse_arg_list();
                            expr = Expr::Call {
                                callee: Box::new(expr),
                                type_args: Vec::new(),
                                args,
                                span: p.span_from(start),
                            };
                        }
                        TokenKind::Lt => {
                            // A balanced `<...>` closed immediately before `(`, `.`, or
                            // `?.` is a generic invocation; one followed by a token that
                            // cannot continue a comparison is a bare tear-off
                            // instantiation; otherwise `<` is a comparison operator
                            // (`a < b ? () => c() : null`).
                            if p.is_generic_invocation() {
                                let parsed = p.speculate(|p| {
                                    let type_args = p.parse_type_args();
                                    if p.at(TokenKind::LParen) {
                                        Some((type_args, Some(p.parse_arg_list())))
                                    } else if matches!(
                                        p.cur().kind,
                                        TokenKind::Dot | TokenKind::QmarkDot
                                    ) {
                                        Some((type_args, None))
                                    } else {
                                        None
                                    }
                                });
                                let Some((type_args, args)) = parsed else {
                                    break;
                                };
                                expr = match args {
                                    Some(args) => Expr::Call {
                                        callee: Box::new(expr),
                                        type_args,
                                        args,
                                        span: p.span_from(start),
                                    },
                                    None => Expr::GenericInstantiation {
                                        target: Box::new(expr),
                                        type_args,
                                        span: p.span_from(start),
                                    },
                                };
                            } else if p.is_generic_instantiation() {
                                // Bare generic tear-off: `identity<int>` with no call/`.`
                                // following. Preserve the target and its type arguments.
                                let type_args = p.parse_type_args();
                                expr = Expr::GenericInstantiation {
                                    target: Box::new(expr),
                                    type_args,
                                    span: p.span_from(start),
                                };
                            } else {
                                break;
                            }
                        }
                        TokenKind::Switch => {
                            p.advance();
                            let arms = p.parse_switch_expr_arms();
                            expr = Expr::Switch {
                                subject: Box::new(expr),
                                arms,
                                span: p.span_from(start),
                            };
                        }
                        _ => break,
                    }
                }
                expr
            },
        )
    }

    fn parse_cascade_sections(&mut self) -> Vec<CascadeSection> {
        let mut sections = Vec::new();
        while matches!(self.cur().kind, TokenKind::DotDot | TokenKind::DotDotQmark) {
            let start = self.cur().offset;
            // A `?..` section is reached null-awarely; that shorting also covers a
            // `?[` null-aware index selector on the section's first selector.
            let section_null_aware = self.at(TokenKind::DotDotQmark);
            self.advance();
            let ops = self.parse_cascade_section_ops(section_null_aware);
            if ops.is_empty() {
                break;
            }
            sections.push(CascadeSection {
                ops,
                span: self.span_from(start),
            });
        }
        sections
    }

    /// Parse the selector chain of a single cascade section (everything after the
    /// `..`/`?..`). Dart allows a full assignable-selector chain here — `..a.b()`,
    /// `..m().n()`, `..a[i]=x`, `..a?.b()` — not just one selector, so this mirrors
    /// the `parse_postfix` selector loop but seeds the first selector as a bare
    /// identifier or `[index]` (no leading dot) and stops at a terminal assignment.
    fn parse_cascade_section_ops(&mut self, section_null_aware: bool) -> Vec<CascadeOp> {
        let mut ops = Vec::new();
        // First selector after `..`: bare identifier or `[index]`, no leading dot.
        let first = if self.at(TokenKind::LBracket) || self.at(TokenKind::QmarkLBracket) {
            let index_null_aware = section_null_aware || self.at(TokenKind::QmarkLBracket);
            self.advance();
            let idx = self.parse_expr();
            self.expect(TokenKind::RBracket);
            if let Some(assign_op) = self.try_parse_assign_op() {
                let rhs = self.parse_expr_without_cascade();
                ops.push(CascadeOp::Assign(Box::new(idx), assign_op, Box::new(rhs)));
                return ops; // assignment terminates the section
            }
            CascadeOp::Index {
                index: Box::new(idx),
                is_null_aware: index_null_aware,
            }
        } else if self.cur().kind.is_identifier_like() {
            let name = self.expect_ident();
            if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) {
                let type_args = self.parse_type_args();
                let args = self.parse_arg_list();
                CascadeOp::Call(name, type_args, args)
            } else if let Some(assign_op) = self.try_parse_assign_op() {
                let rhs = self.parse_expr_without_cascade();
                ops.push(CascadeOp::Assign(
                    Box::new(Expr::Ident(name)),
                    assign_op,
                    Box::new(rhs),
                ));
                return ops;
            } else {
                CascadeOp::Field {
                    name,
                    is_null_aware: section_null_aware,
                }
            }
        } else {
            return ops; // empty section
        };
        ops.push(first);
        loop {
            match self.cur().kind {
                TokenKind::Dot | TokenKind::QmarkDot => {
                    let is_null_safe = self.at(TokenKind::QmarkDot);
                    self.advance();
                    let name = self.expect_ctor_name();
                    if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) {
                        let type_args = self.parse_type_args();
                        let args = self.parse_arg_list();
                        ops.push(CascadeOp::Call(name, type_args, args));
                    } else if let Some(assign_op) = self.try_parse_assign_op() {
                        let rhs = self.parse_expr_without_cascade();
                        ops.push(CascadeOp::Assign(
                            Box::new(Expr::Ident(name)),
                            assign_op,
                            Box::new(rhs),
                        ));
                        break;
                    } else {
                        ops.push(CascadeOp::Field {
                            name,
                            is_null_aware: is_null_safe,
                        });
                    }
                }
                TokenKind::LBracket | TokenKind::QmarkLBracket => {
                    let is_null_safe = self.at(TokenKind::QmarkLBracket);
                    self.advance();
                    let idx = self.parse_expr();
                    self.expect(TokenKind::RBracket);
                    if let Some(assign_op) = self.try_parse_assign_op() {
                        let rhs = self.parse_expr_without_cascade();
                        ops.push(CascadeOp::Assign(Box::new(idx), assign_op, Box::new(rhs)));
                        break;
                    }
                    ops.push(CascadeOp::Index {
                        index: Box::new(idx),
                        is_null_aware: is_null_safe,
                    });
                }
                _ => break,
            }
        }
        ops
    }

    fn try_parse_assign_op(&mut self) -> Option<AssignOp> {
        let op = match self.cur().kind {
            TokenKind::Eq => AssignOp::Eq,
            TokenKind::PlusEq => AssignOp::PlusEq,
            TokenKind::MinusEq => AssignOp::MinusEq,
            TokenKind::StarEq => AssignOp::MulEq,
            TokenKind::SlashEq => AssignOp::DivEq,
            TokenKind::PercentEq => AssignOp::ModEq,
            TokenKind::TildeSlashEq => AssignOp::IntDivEq,
            TokenKind::AmpEq => AssignOp::AndEq,
            TokenKind::PipeEq => AssignOp::OrEq,
            TokenKind::CaretEq => AssignOp::XorEq,
            TokenKind::LtLtEq => AssignOp::ShlEq,
            TokenKind::GtGtEq => AssignOp::ShrEq,
            TokenKind::GtGtGtEq => AssignOp::UShrEq,
            TokenKind::QmarkQmarkEq => AssignOp::NullCoalesceEq,
            _ => return None,
        };
        self.advance();
        Some(op)
    }

    pub(super) fn parse_primary(&mut self) -> Expr {
        self.guarded(
            |p| Expr::Error { span: p.cur_span() },
            |p| {
                let start = p.cur().offset;
                match p.cur().kind {
                    TokenKind::IntLit => {
                        let text = p.cur_text().to_string();
                        p.advance();
                        Expr::IntLit {
                            value: text,
                            span: p.span_from(start),
                        }
                    }
                    TokenKind::DoubleLit => {
                        let text = p.cur_text().to_string();
                        p.advance();
                        Expr::DoubleLit {
                            value: text,
                            span: p.span_from(start),
                        }
                    }
                    TokenKind::StringLit => Expr::StringLit(p.parse_adjacent_strings()),
                    TokenKind::True => {
                        p.advance();
                        Expr::BoolLit {
                            value: true,
                            span: p.span_from(start),
                        }
                    }
                    TokenKind::False => {
                        p.advance();
                        Expr::BoolLit {
                            value: false,
                            span: p.span_from(start),
                        }
                    }
                    TokenKind::Null => {
                        p.advance();
                        Expr::NullLit {
                            span: p.span_from(start),
                        }
                    }
                    TokenKind::This => {
                        p.advance();
                        Expr::This {
                            span: p.span_from(start),
                        }
                    }
                    TokenKind::Super => {
                        p.advance();
                        Expr::Super {
                            span: p.span_from(start),
                        }
                    }
                    TokenKind::Switch => p.parse_switch_expr_primary(start),
                    TokenKind::LParen => {
                        if p.looks_like_function_expr() {
                            // `(...)` followed by `{` is ambiguous: a lambda `(params) {body}`,
                            // or a parenthesized expression whose trailing `{` opens an
                            // *enclosing* block (e.g. a constructor body in `C(): g = (e) {}`).
                            // Speculatively parse the lambda; if its parameter list doesn't
                            // parse cleanly, roll back and treat it as a paren/record.
                            p.speculate(|p| {
                                let errors = p.errors.len();
                                let expr =
                                    p.parse_function_expr_with_type_params(Vec::new(), start);
                                let is_plain_block_lambda = matches!(
                                    &expr,
                                    Expr::FuncExpr { async_marker: AsyncMarker::Sync, body, .. }
                                        if matches!(**body, FunctionBody::Block(_))
                                );
                                if p.errors.len() > errors
                                    || (p.ctx.in_ctor_init_value && is_plain_block_lambda)
                                {
                                    None
                                } else {
                                    Some(expr)
                                }
                            })
                            .unwrap_or_else(|| p.parse_paren_or_record(false, start))
                        } else {
                            p.parse_paren_or_record(false, start)
                        }
                    }
                    TokenKind::Lt if p.looks_like_generic_function_expr() => {
                        let type_params = p.parse_type_params();
                        p.parse_function_expr_with_type_params(type_params, start)
                    }
                    TokenKind::Lt => p.parse_typed_collection(false, start),
                    TokenKind::LBracket => p.parse_list_literal(false, None, start),
                    TokenKind::LBrace => p.parse_map_or_set_literal(false, Vec::new(), start),
                    // Dot shorthand (Dart 3.9): `.name` / `.new` in a context position.
                    TokenKind::Dot => p.parse_dot_shorthand(false, start),
                    TokenKind::Const => {
                        p.advance();
                        p.parse_const_expr(start)
                    }
                    TokenKind::New => {
                        p.advance();
                        let dart_type = p.parse_type();
                        let constructor_name = if p.eat(TokenKind::Dot).is_some() {
                            Some(p.expect_ident())
                        } else {
                            None
                        };
                        let args = p.parse_arg_list();
                        Expr::New {
                            is_const: false,
                            dart_type,
                            constructor_name,
                            args,
                            span: p.span_from(start),
                        }
                    }
                    TokenKind::Hash => p.parse_symbol_literal(start),
                    _ if p.cur().kind.is_identifier_like() => {
                        let id = p.expect_ident();
                        Expr::Ident(id)
                    }
                    _ => {
                        p.error(format!(
                            "unexpected token in expression: {:?}",
                            p.cur().kind
                        ));
                        p.advance();
                        Expr::Error {
                            span: p.span_from(start),
                        }
                    }
                }
            },
        )
    }

    pub(super) fn parse_paren_or_record(&mut self, is_const: bool, start: usize) -> Expr {
        self.advance();
        // Within these parens a trailing `{` never opens the constructor body.
        self.ctx.in_ctor_init_value = false;
        if self.at(TokenKind::RParen) {
            self.advance();
            return Expr::Record {
                is_const,
                fields: Vec::new(),
                span: self.span_from(start),
            };
        }
        // Detect named first field before calling parse_expr, so `:` stays in stream.
        let (first_name, first_value) =
            if self.cur().kind.is_identifier_like() && self.peek(1).kind == TokenKind::Colon {
                let name = self.expect_ident();
                self.advance();
                let value = self.parse_expr();
                (Some(name), value)
            } else {
                (None, self.parse_expr())
            };
        if self.eat(TokenKind::RParen).is_some() {
            if first_name.is_none() {
                return first_value;
            }
            let fields = vec![RecordField {
                name: first_name,
                value: first_value,
                span: self.span_from(start),
            }];
            return Expr::Record {
                is_const,
                fields,
                span: self.span_from(start),
            };
        }
        let mut fields = vec![RecordField {
            name: first_name,
            value: first_value,
            span: self.span_from(start),
        }];
        while self.eat(TokenKind::Comma).is_some() && !self.at(TokenKind::RParen) {
            let f_start = self.cur().offset;
            if self.cur().kind.is_identifier_like() && self.peek(1).kind == TokenKind::Colon {
                let name = self.expect_ident();
                self.advance();
                let value = self.parse_expr();
                fields.push(RecordField {
                    name: Some(name),
                    value,
                    span: self.span_from(f_start),
                });
            } else {
                let value = self.parse_expr();
                fields.push(RecordField {
                    name: None,
                    value,
                    span: self.span_from(f_start),
                });
            }
        }
        self.eat(TokenKind::Comma);
        self.expect(TokenKind::RParen);
        Expr::Record {
            is_const,
            fields,
            span: self.span_from(start),
        }
    }

    /// Returns `true` when the `(` at `self.pos` starts a function expression
    /// rather than a parenthesised expression or record literal.
    ///
    /// Fast path: a modifier keyword (`final`, `var`, `required`, `covariant`)
    /// immediately inside the parens unambiguously signals a parameter list.
    fn looks_like_function_expr(&self) -> bool {
        debug_assert_eq!(self.cur().kind, TokenKind::LParen);

        if matches!(
            self.peek(1).kind,
            TokenKind::Final | TokenKind::Var | TokenKind::Required | TokenKind::Covariant
        ) {
            return true;
        }

        self.matching_close(
            self.pos,
            Brackets::ParenOnly,
            |kind| kind == TokenKind::Semicolon,
            None,
        )
        .is_some_and(|i| {
            matches!(
                self.kind_at(i + 1),
                TokenKind::Arrow | TokenKind::Async | TokenKind::Sync | TokenKind::LBrace
            )
        })
    }

    /// True when a `<` at the cursor opens the type-parameter list of a generic
    /// function expression: a balanced `<...>` closed immediately before a `(`
    /// whose matching `)` is followed by `=>`, `{`, `async`, or `sync`.
    fn looks_like_generic_function_expr(&self) -> bool {
        use TokenKind::*;
        let Some(close) = self.matching_angle_close(
            self.pos,
            |kind| {
                kind.is_identifier_like()
                    || matches!(
                        kind,
                        Dot | Comma
                            | Qmark
                            | Extends
                            | Void
                            | LParen
                            | RParen
                            | LBracket
                            | RBracket
                            | LBrace
                            | RBrace
                    )
            },
            Some(513),
        ) else {
            return false;
        };
        let paren = close + 1;
        self.kind_at(paren) == LParen
            && self
                .matching_close(paren, Brackets::ParenOnly, |_| false, None)
                .is_some_and(|i| matches!(self.kind_at(i + 1), Arrow | LBrace | Async | Sync))
    }

    fn parse_function_expr_with_type_params(
        &mut self,
        type_params: Vec<TypeParam>,
        start: usize,
    ) -> Expr {
        let params = self.parse_formal_param_list();
        let async_marker = self.parse_async_marker();
        // Closure arrows leave the enclosing declaration's semicolon untouched.
        let body = self.with_body(async_marker, |p| match p.cur().kind {
            TokenKind::Arrow => {
                let bstart = p.cur().offset;
                p.advance();
                let e = p.parse_expr();
                FunctionBody::Arrow(Box::new(e), p.span_from(bstart))
            }
            TokenKind::LBrace => FunctionBody::Block(p.parse_block()),
            _ => {
                p.error("expected function body after parameter list");
                FunctionBody::Block(Block {
                    stmts: Vec::new(),
                    span: p.span_from(start),
                })
            }
        });
        Expr::FuncExpr {
            type_params,
            params,
            async_marker,
            body: Box::new(body),
            span: self.span_from(start),
        }
    }

    fn parse_switch_expr_primary(&mut self, start: usize) -> Expr {
        self.advance();
        self.expect(TokenKind::LParen);
        let subject = self.parse_expr();
        self.expect(TokenKind::RParen);
        let arms = self.parse_switch_expr_arms();
        Expr::Switch {
            subject: Box::new(subject),
            arms,
            span: self.span_from(start),
        }
    }

    fn parse_list_literal(
        &mut self,
        is_const: bool,
        type_arg: Option<DartType>,
        start: usize,
    ) -> Expr {
        self.advance();
        // Inside a list literal a trailing `{` never opens the constructor body.
        self.ctx.in_ctor_init_value = false;
        let mut elements = Vec::new();
        while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
            elements.push(self.parse_collection_element());
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect(TokenKind::RBracket);
        Expr::List {
            is_const,
            type_arg,
            elements,
            span: self.span_from(start),
        }
    }

    fn parse_map_or_set_literal(
        &mut self,
        is_const: bool,
        type_args: Vec<DartType>,
        start: usize,
    ) -> Expr {
        self.advance();
        // Inside a set/map literal a trailing `{` never opens the constructor body.
        self.ctx.in_ctor_init_value = false;
        if self.at(TokenKind::RBrace) {
            self.advance();
            // Empty braces: a single type arg (`<int>{}`) is a Set; no type args
            // (`{}`) or two (`<K, V>{}`) is a Map.
            if type_args.len() == 1 {
                return Expr::Set {
                    is_const,
                    type_arg: type_args.into_iter().next(),
                    elements: Vec::new(),
                    span: self.span_from(start),
                };
            }
            return Expr::Map {
                is_const,
                type_args,
                elements: Vec::new(),
                span: self.span_from(start),
            };
        }
        // Decide map vs set by looking for a `k: v` entry leaf: a colon means a
        // map, a plain/spread element a set. Works for plain, spread- and
        // comprehension-led (`for`/`if`) literals alike. A spread-only literal is
        // ambiguous (`{...a}` could be either) — explicit type-arg arity decides,
        // otherwise it defaults to a Set.
        let is_map = self.map_literal_lookahead().unwrap_or(type_args.len() == 2);
        if is_map {
            let mut elements = Vec::new();
            while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                elements.push(self.parse_map_element());
                if self.eat(TokenKind::Comma).is_none() {
                    break;
                }
            }
            self.expect(TokenKind::RBrace);
            Expr::Map {
                is_const,
                type_args,
                elements,
                span: self.span_from(start),
            }
        } else {
            let mut elements = Vec::new();
            while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                elements.push(self.parse_collection_element());
                if self.eat(TokenKind::Comma).is_none() {
                    break;
                }
            }
            self.expect(TokenKind::RBrace);
            let type_arg = type_args.into_iter().next();
            Expr::Set {
                is_const,
                type_arg,
                elements,
                span: self.span_from(start),
            }
        }
    }

    /// Look ahead (without consuming) to classify a non-empty `{...}` literal:
    /// `Some(true)` for a map (a `k: v` entry leaf), `Some(false)` for a set (a
    /// plain element leaf), `None` when every element is a spread and no entry is
    /// present (`{...a}` / `{...a, ...b}`) — genuinely ambiguous, so the caller
    /// decides. Descends through leading `for`/`if` comprehension headers and
    /// looks *past* leading spreads for a decisive `:` leaf.
    fn map_literal_lookahead(&mut self) -> Option<bool> {
        let checkpoint = self.checkpoint();
        let result = loop {
            loop {
                // `await for` comprehension headers lead with `await`; skip it so the
                // `for` is recognized and `{ await for (...) k: v }` reaches the map
                // classification path.
                if self.at(TokenKind::Await) && self.peek(1).kind == TokenKind::For {
                    self.advance();
                }
                match self.cur().kind {
                    TokenKind::For | TokenKind::If => {
                        self.advance();
                        self.skip_balanced_parens();
                    }
                    _ => break,
                }
            }
            match self.cur().kind {
                TokenKind::DotDotDot | TokenKind::DotDotDotQmark => {
                    // A spread is ambiguous on its own; skip it and inspect the
                    // next element for a decisive `k: v` entry.
                    self.advance();
                    let _ = self.parse_expr();
                    if self.eat(TokenKind::Comma).is_some() && !self.at(TokenKind::RBrace) {
                        continue;
                    }
                    break None;
                }
                _ => {
                    // A null-aware key/element leads with `?`; skip it so `{?k: v}`
                    // (map) is told apart from `{?x}` (set).
                    let _ = self.eat(TokenKind::Qmark);
                    let _ = self.parse_expr();
                    break Some(self.at(TokenKind::Colon));
                }
            }
        };
        self.restore(checkpoint);
        result
    }

    /// Skip a balanced `( ... )` group starting at the current token. Used by
    /// lookahead to step over a comprehension header without interpreting it.
    fn skip_balanced_parens(&mut self) {
        if self.at(TokenKind::LParen) {
            self.pos = self
                .index_after_balanced_parens(self.pos)
                .min(self.tokens.len() - 1);
        }
    }

    fn parse_spread(&mut self) -> (Expr, bool) {
        let is_null_aware = self.at(TokenKind::DotDotDotQmark);
        self.advance();
        (self.parse_expr(), is_null_aware)
    }

    /// Parse one element of a comprehension-form map literal (`k: v`, or a
    /// `for`/`if`/spread wrapping such entries).
    fn parse_map_element(&mut self) -> MapElement {
        let start = self.cur().offset;
        match self.cur().kind {
            TokenKind::DotDotDot | TokenKind::DotDotDotQmark => {
                let (expr, is_null_aware) = self.parse_spread();
                MapElement::Spread {
                    expr,
                    is_null_aware,
                    span: self.span_from(start),
                }
            }
            TokenKind::If => {
                self.advance();
                let condition = self.parse_if_condition();
                let then_entry = Box::new(self.parse_map_element());
                let else_entry = if self.eat(TokenKind::Else).is_some() {
                    Some(Box::new(self.parse_map_element()))
                } else {
                    None
                };
                MapElement::If {
                    condition,
                    then_entry,
                    else_entry,
                    span: self.span_from(start),
                }
            }
            TokenKind::Await if self.peek(1).kind == TokenKind::For => {
                self.advance();
                self.advance();
                self.finish_for_map_element(true, start)
            }
            TokenKind::For => {
                self.advance();
                self.finish_for_map_element(false, start)
            }
            _ => {
                // Dart 3.0 null-aware key `?k: v` and/or value `k: ?v`.
                let key_null_aware = self.eat(TokenKind::Qmark).is_some();
                let key = self.parse_expr();
                self.expect(TokenKind::Colon);
                let value_null_aware = self.eat(TokenKind::Qmark).is_some();
                let value = self.parse_expr();
                MapElement::Entry(MapEntry {
                    key,
                    value,
                    key_null_aware,
                    value_null_aware,
                    span: self.span_from(start),
                })
            }
        }
    }

    fn parse_collection_element(&mut self) -> CollectionElement {
        let start = self.cur().offset;
        match self.cur().kind {
            // Dart 3.0 null-aware element `?expr`
            TokenKind::Qmark => {
                self.advance();
                let expr = self.parse_expr();
                CollectionElement::NullAware {
                    expr,
                    span: self.span_from(start),
                }
            }
            TokenKind::DotDotDot | TokenKind::DotDotDotQmark => {
                let (expr, is_null_aware) = self.parse_spread();
                CollectionElement::Spread {
                    expr,
                    is_null_aware,
                    span: self.span_from(start),
                }
            }
            TokenKind::If => {
                self.advance();
                let condition = self.parse_if_condition();
                let then_elem = Box::new(self.parse_collection_element());
                let else_elem = if self.eat(TokenKind::Else).is_some() {
                    Some(Box::new(self.parse_collection_element()))
                } else {
                    None
                };
                CollectionElement::If {
                    condition,
                    then_elem,
                    else_elem,
                    span: self.span_from(start),
                }
            }
            // `await for (...)` — an asynchronous for-in element (async context).
            TokenKind::Await if self.peek(1).kind == TokenKind::For => {
                self.advance();
                self.advance();
                self.finish_for_collection_element(true, start)
            }
            TokenKind::For => {
                self.advance();
                self.finish_for_collection_element(false, start)
            }
            _ => CollectionElement::Expr(self.parse_expr()),
        }
    }

    fn finish_for_collection_element(&mut self, is_await: bool, start: usize) -> CollectionElement {
        let header = self.parse_collection_for_header(is_await);
        let element = Box::new(self.parse_collection_element());
        CollectionElement::For {
            is_await,
            header,
            element,
            span: self.span_from(start),
        }
    }

    fn finish_for_map_element(&mut self, is_await: bool, start: usize) -> MapElement {
        let header = self.parse_collection_for_header(is_await);
        let entry = Box::new(self.parse_map_element());
        MapElement::For {
            is_await,
            header,
            entry,
            span: self.span_from(start),
        }
    }

    pub(super) fn parse_if_condition(&mut self) -> IfCondition {
        self.expect(TokenKind::LParen);
        let scrutinee = self.parse_expr();
        let condition = if self.eat(TokenKind::Case).is_some() {
            let pattern = self.parse_pattern();
            let guard = if self.eat(TokenKind::When).is_some() {
                Some(Box::new(self.parse_expr()))
            } else {
                None
            };
            IfCondition::Case(scrutinee, Box::new(pattern), guard)
        } else {
            IfCondition::Expr(scrutinee)
        };
        self.expect(TokenKind::RParen);
        condition
    }

    fn parse_collection_for_header(&mut self, is_await: bool) -> ForHeader {
        self.expect(TokenKind::LParen);
        if matches!(
            self.cur().kind,
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace
        ) {
            let pattern = Box::new(self.parse_binding_pattern());
            self.expect(TokenKind::In);
            let iterable = Box::new(self.parse_expr());
            self.expect(TokenKind::RParen);
            return ForHeader {
                init: Some(ForInit::PatternForIn {
                    keyword: VarKeyword::Var,
                    pattern,
                    iterable,
                }),
                condition: None,
                updates: Vec::new(),
            };
        }
        let (init, condition, updates) = self.parse_for_clauses();
        self.check_await_for_header(is_await, &init);
        ForHeader {
            init,
            condition,
            updates,
        }
    }

    pub(super) fn check_await_for_header(&mut self, is_await: bool, init: &Option<ForInit>) {
        if is_await
            && !matches!(
                init,
                Some(ForInit::ForIn { .. } | ForInit::PatternForIn { .. })
            )
        {
            self.error(
                "'await' cannot be used with a C-style for loop; use 'await for (x in ...)'",
            );
        }
    }

    /// Parses a dot shorthand head: `.name` or `.new`. The caller has already
    /// recorded `start`; the leading `.` is still current. Any trailing
    /// invocation or selector is left for `parse_postfix`.
    pub(super) fn parse_dot_shorthand(&mut self, is_const: bool, start: usize) -> Expr {
        self.advance();
        let name = self.expect_ctor_name();
        Expr::DotShorthand {
            is_const,
            name,
            span: self.span_from(start),
        }
    }

    /// Parse a symbol literal — the leading `#` is current. The body is either a
    /// dotted identifier chain (`#foo`, `#foo.bar.baz`) or a user-definable
    /// operator (`#+`, `#==`, `#[]`, `#[]=`, `#~`). `raw` is sliced verbatim from
    /// source so it always includes the `#` and exact operator spelling.
    pub(super) fn parse_symbol_literal(&mut self, start: usize) -> Expr {
        use TokenKind::*;
        let hash = self.advance();
        let mut end = hash.offset + hash.len;
        if self.cur().kind.is_identifier_like() || self.at(Void) {
            let tok = self.advance();
            end = tok.offset + tok.len;
            while self.at(Dot) {
                let dot = self.advance();
                end = dot.offset + dot.len;
                if self.cur().kind.is_identifier_like() || self.at(Void) {
                    let id = self.advance();
                    end = id.offset + id.len;
                } else {
                    self.error("expected identifier after '.' in symbol literal");
                    break;
                }
            }
        } else if self.at(LBracket) {
            // Index operator `[]` or index-assign `[]=`.
            self.advance();
            let rb = self.expect(RBracket);
            end = rb.offset + rb.len;
            if let Some(eq) = self.eat(Eq) {
                end = eq.offset + eq.len;
            }
        } else if self.cur().kind.is_user_definable_operator() {
            let op = self.advance();
            end = op.offset + op.len;
        } else {
            self.error("expected symbol name after '#'");
        }
        let raw = self.src[start..end].to_string();
        Expr::SymbolLit {
            raw,
            span: Span::new(start, end),
        }
    }

    fn parse_const_expr(&mut self, start: usize) -> Expr {
        // const can prefix a constructor call, collection literal, or (Dart 3.9)
        // a dot shorthand: `const .fromLtrb(...)`.
        match self.cur().kind {
            TokenKind::Dot => self.parse_dot_shorthand(true, start),
            // `const ('', X)` — a const record literal. Without this the `_` arm
            // would demand a type name after `const` and choke on the `(`.
            TokenKind::LParen => self.parse_paren_or_record(true, start),
            TokenKind::LBracket => self.parse_list_literal(true, None, start),
            TokenKind::LBrace => self.parse_map_or_set_literal(true, Vec::new(), start),
            TokenKind::Lt => self.parse_typed_collection(true, start),
            _ => {
                let dart_type = self.parse_type();
                // A named constructor may be `.new` (`const C.new()`); `new` is a
                // keyword token, so accept it explicitly here.
                let constructor_name = if self.eat(TokenKind::Dot).is_some() {
                    Some(self.expect_ctor_name())
                } else {
                    None
                };
                let args = self.parse_arg_list();
                Expr::New {
                    is_const: true,
                    dart_type,
                    constructor_name,
                    args,
                    span: self.span_from(start),
                }
            }
        }
    }
}
