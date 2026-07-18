use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;

use super::Parser;

/// Result of parsing a collection/map comprehension `for (...)` header.
enum ForHeader {
    ForIn {
        variable: Option<Identifier>,
        var_type: Option<DartType>,
        pattern: Option<Box<Pattern>>,
        iterable: Expr,
    },
    CStyle {
        init: Option<ForInit>,
        condition: Option<Expr>,
        updates: Vec<Expr>,
    },
}

impl<'src> Parser<'src> {
    // ── Public entry ──────────────────────────────────────────────────────────

    pub(super) fn parse_expr(&mut self) -> Expr {
        self.parse_assign()
    }

    // ── Assignment (right-associative) ────────────────────────────────────────

    fn parse_assign(&mut self) -> Expr {
        let start = self.cur().offset;
        let lhs = self.parse_cascade();

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
            _ => return lhs,
        };
        self.advance();
        let rhs = self.parse_assign(); // right-associative
        let span = self.span_from(start);
        Expr::Assign {
            target: Box::new(lhs),
            op,
            value: Box::new(rhs),
            span,
        }
    }

    // ── Cascade  ..  ?.. ──────────────────────────────────────────────────────

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

    // ── Conditional ternary ───────────────────────────────────────────────────

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
            _ => return lhs,
        };
        self.advance();
        let rhs = self.parse_expr_without_cascade(); // right-associative
        Expr::Assign {
            target: Box::new(lhs),
            op,
            value: Box::new(rhs),
            span: self.span_from(start),
        }
    }

    // ── Null coalescing ?? ────────────────────────────────────────────────────

    fn parse_null_coalesce(&mut self) -> Expr {
        let start = self.cur().offset;
        let mut lhs = self.parse_or();
        while self.eat(TokenKind::QmarkQmark).is_some() {
            let rhs = self.parse_or();
            let span = self.span_from(start);
            lhs = Expr::Binary {
                op: BinaryOp::NullCoalesce,
                left: Box::new(lhs),
                right: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    // ── Logical OR ────────────────────────────────────────────────────────────

    fn parse_or(&mut self) -> Expr {
        let start = self.cur().offset;
        let mut lhs = self.parse_and();
        while self.eat(TokenKind::PipePipe).is_some() {
            let rhs = self.parse_and();
            let span = self.span_from(start);
            lhs = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(lhs),
                right: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    // ── Logical AND ───────────────────────────────────────────────────────────

    fn parse_and(&mut self) -> Expr {
        let start = self.cur().offset;
        let mut lhs = self.parse_equality();
        while self.eat(TokenKind::AmpAmp).is_some() {
            let rhs = self.parse_equality();
            let span = self.span_from(start);
            lhs = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(lhs),
                right: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    // ── Equality ──────────────────────────────────────────────────────────────

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

    // ── Relational / type tests ───────────────────────────────────────────────

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
                k if self.is_ident_like_kind(k) => {}
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
        let prev = self.suppress_conditional_qmark;
        self.suppress_conditional_qmark = true;
        let ty = self.parse_type();
        self.suppress_conditional_qmark = prev;
        ty
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

    // ── Bitwise OR/XOR/AND ────────────────────────────────────────────────────

    pub(super) fn parse_bitwise_or(&mut self) -> Expr {
        let start = self.cur().offset;
        let mut lhs = self.parse_bitwise_xor();
        while self.eat(TokenKind::Pipe).is_some() {
            let rhs = self.parse_bitwise_xor();
            lhs = Expr::Binary {
                op: BinaryOp::BitOr,
                left: Box::new(lhs),
                right: Box::new(rhs),
                span: self.span_from(start),
            };
        }
        lhs
    }

    fn parse_bitwise_xor(&mut self) -> Expr {
        let start = self.cur().offset;
        let mut lhs = self.parse_bitwise_and();
        while self.eat(TokenKind::Caret).is_some() {
            let rhs = self.parse_bitwise_and();
            lhs = Expr::Binary {
                op: BinaryOp::BitXor,
                left: Box::new(lhs),
                right: Box::new(rhs),
                span: self.span_from(start),
            };
        }
        lhs
    }

    fn parse_bitwise_and(&mut self) -> Expr {
        let start = self.cur().offset;
        let mut lhs = self.parse_shift();
        while self.eat(TokenKind::Amp).is_some() {
            let rhs = self.parse_shift();
            lhs = Expr::Binary {
                op: BinaryOp::BitAnd,
                left: Box::new(lhs),
                right: Box::new(rhs),
                span: self.span_from(start),
            };
        }
        lhs
    }

    // ── Shift ─────────────────────────────────────────────────────────────────

    fn parse_shift(&mut self) -> Expr {
        let start = self.cur().offset;
        let mut lhs = self.parse_additive();
        loop {
            let op = match self.cur().kind {
                TokenKind::LtLt => BinaryOp::Shl,
                TokenKind::GtGt => BinaryOp::Shr,
                TokenKind::GtGtGt => BinaryOp::UShr,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_additive();
            lhs = Expr::Binary {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
                span: self.span_from(start),
            };
        }
        lhs
    }

    // ── Additive ──────────────────────────────────────────────────────────────

    fn parse_additive(&mut self) -> Expr {
        let start = self.cur().offset;
        let mut lhs = self.parse_multiplicative();
        loop {
            let op = match self.cur().kind {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_multiplicative();
            lhs = Expr::Binary {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
                span: self.span_from(start),
            };
        }
        lhs
    }

    // ── Multiplicative ────────────────────────────────────────────────────────

    fn parse_multiplicative(&mut self) -> Expr {
        let start = self.cur().offset;
        let mut lhs = self.parse_unary();
        loop {
            let op = match self.cur().kind {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Mod,
                TokenKind::TildeSlash => BinaryOp::IntDiv,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_unary();
            lhs = Expr::Binary {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
                span: self.span_from(start),
            };
        }
        lhs
    }

    // ── Unary ─────────────────────────────────────────────────────────────────

    fn parse_unary(&mut self) -> Expr {
        let start = self.cur().offset;
        match self.cur().kind {
            TokenKind::Minus => {
                self.advance();
                let operand = self.parse_unary();
                Expr::Unary {
                    op: UnaryOp::Minus,
                    operand: Box::new(operand),
                    span: self.span_from(start),
                }
            }
            TokenKind::Bang => {
                self.advance();
                let operand = self.parse_unary();
                Expr::Unary {
                    op: UnaryOp::Bang,
                    operand: Box::new(operand),
                    span: self.span_from(start),
                }
            }
            TokenKind::Tilde => {
                self.advance();
                let operand = self.parse_unary();
                Expr::Unary {
                    op: UnaryOp::Tilde,
                    operand: Box::new(operand),
                    span: self.span_from(start),
                }
            }
            TokenKind::PlusPlus => {
                self.advance();
                let operand = self.parse_unary();
                Expr::Unary {
                    op: UnaryOp::PlusPlus,
                    operand: Box::new(operand),
                    span: self.span_from(start),
                }
            }
            TokenKind::MinusMinus => {
                self.advance();
                let operand = self.parse_unary();
                Expr::Unary {
                    op: UnaryOp::MinusMinus,
                    operand: Box::new(operand),
                    span: self.span_from(start),
                }
            }
            TokenKind::Await => {
                self.advance();
                let operand = self.parse_unary();
                Expr::Await {
                    expr: Box::new(operand),
                    span: self.span_from(start),
                }
            }
            TokenKind::Throw => {
                self.advance();
                let operand = self.parse_expr();
                Expr::Throw {
                    expr: Box::new(operand),
                    span: self.span_from(start),
                }
            }
            _ => self.parse_postfix(),
        }
    }

    // ── Postfix ───────────────────────────────────────────────────────────────

    fn parse_postfix(&mut self) -> Expr {
        let start = self.cur().offset;
        let mut expr = self.parse_primary();

        loop {
            match self.cur().kind {
                // Postfix ++ / --
                TokenKind::PlusPlus => {
                    self.advance();
                    expr = Expr::PostfixIncDec {
                        op: PostfixIncDec::Increment,
                        operand: Box::new(expr),
                        span: self.span_from(start),
                    };
                }
                TokenKind::MinusMinus => {
                    self.advance();
                    expr = Expr::PostfixIncDec {
                        op: PostfixIncDec::Decrement,
                        operand: Box::new(expr),
                        span: self.span_from(start),
                    };
                }
                // Null-assertion  expr!
                TokenKind::Bang => {
                    self.advance();
                    expr = Expr::NullAssert {
                        operand: Box::new(expr),
                        span: self.span_from(start),
                    };
                }
                // Member access  .field  ?.field  .new (constructor tear-off)
                TokenKind::Dot | TokenKind::QmarkDot => {
                    let is_null_safe = self.cur().kind == TokenKind::QmarkDot;
                    self.advance();
                    // `.new` is a valid constructor tear-off in Dart 3
                    let field = if self.at(TokenKind::New) {
                        let tok = self.advance();
                        Identifier::new("new", Self::tok_span(&tok))
                    } else {
                        self.expect_ident()
                    };
                    expr = Expr::Field {
                        object: Box::new(expr),
                        field,
                        is_null_safe,
                        span: self.span_from(start),
                    };
                }
                // Index  [i]  ?[i]
                TokenKind::LBracket | TokenKind::QmarkLBracket => {
                    let is_null_safe = self.cur().kind == TokenKind::QmarkLBracket;
                    self.advance();
                    let index = self.parse_expr();
                    self.expect(TokenKind::RBracket);
                    expr = Expr::Index {
                        object: Box::new(expr),
                        index: Box::new(index),
                        is_null_safe,
                        span: self.span_from(start),
                    };
                }
                // Call  (args)  with optional type args
                TokenKind::LParen => {
                    let args = self.parse_arg_list();
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        type_args: Vec::new(),
                        args,
                        span: self.span_from(start),
                    };
                }
                // Generic call  <T>(args)  or bare instantiation  identity<int>
                TokenKind::Lt => {
                    // A balanced `<...>` closed immediately before `(`, `.`, or
                    // `?.` is a generic invocation; one followed by a token that
                    // cannot continue a comparison is a bare tear-off
                    // instantiation; otherwise `<` is a comparison operator
                    // (`a < b ? () => c() : null`).
                    if self.is_generic_invocation() {
                        // Speculative: try to parse type args; restore errors on
                        // rollback to avoid spurious "expected type" errors.
                        let saved = self.pos;
                        let saved_errors = self.errors.len();
                        let type_args = self.parse_type_args();
                        if self.at(TokenKind::LParen) {
                            // Generic call: expr<T>(args)
                            let args = self.parse_arg_list();
                            expr = Expr::Call {
                                callee: Box::new(expr),
                                type_args,
                                args,
                                span: self.span_from(start),
                            };
                        } else if self.at(TokenKind::Dot) || self.at(TokenKind::QmarkDot) {
                            // Type instantiation expression: Name<T>.method(args) — keep type args,
                            // represent as Call with empty args; next iteration handles the `.`.
                            let empty_args = ArgList {
                                positional: Vec::new(),
                                named: Vec::new(),
                                span: self.span_from(start),
                            };
                            expr = Expr::Call {
                                callee: Box::new(expr),
                                type_args,
                                args: empty_args,
                                span: self.span_from(start),
                            };
                        } else {
                            // Not a call or type instantiation — restore position and any spurious errors
                            self.rewind_to(saved);
                            self.errors.truncate(saved_errors);
                            break;
                        }
                    } else if self.is_generic_instantiation() {
                        // Bare generic tear-off: `identity<int>` with no call/`.`
                        // following. Preserve the target and its type arguments.
                        let type_args = self.parse_type_args();
                        expr = Expr::GenericInstantiation {
                            target: Box::new(expr),
                            type_args,
                            span: self.span_from(start),
                        };
                    } else {
                        break;
                    }
                }
                // Switch expression (Dart 3.x): `expr switch { pattern => expr, ... }`
                TokenKind::Switch => {
                    self.advance(); // switch
                    self.expect(TokenKind::LBrace);
                    let mut arms = Vec::new();
                    while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                        let arm_start = self.cur().offset;
                        let pattern = self.parse_pattern();
                        let guard = if self.at(TokenKind::When) {
                            self.advance();
                            Some(self.parse_expr())
                        } else {
                            None
                        };
                        self.expect(TokenKind::Arrow);
                        let body = self.parse_expr();
                        arms.push(SwitchExprArm {
                            pattern,
                            guard,
                            body,
                            span: self.span_from(arm_start),
                        });
                        self.eat(TokenKind::Comma);
                    }
                    self.expect(TokenKind::RBrace);
                    expr = Expr::Switch {
                        subject: Box::new(expr),
                        arms,
                        span: self.span_from(start),
                    };
                }
                _ => break,
            }
        }
        expr
    }

    fn parse_cascade_sections(&mut self) -> Vec<CascadeSection> {
        let mut sections = Vec::new();
        while matches!(self.cur().kind, TokenKind::DotDot | TokenKind::DotDotQmark) {
            let start = self.cur().offset;
            // A `?..` section is reached null-awarely; that shorting also covers a
            // `?[` null-aware index selector on the section's first selector.
            let section_null_aware = self.at(TokenKind::DotDotQmark);
            self.advance(); // .. or ?..
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
                let rhs = self.parse_expr();
                ops.push(CascadeOp::Assign(Box::new(idx), assign_op, Box::new(rhs)));
                return ops; // assignment terminates the section
            }
            CascadeOp::Index(Box::new(idx), index_null_aware)
        } else if self.is_ident_like() {
            let name = self.expect_ident();
            if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) {
                let type_args = if self.at(TokenKind::Lt) {
                    self.parse_type_args()
                } else {
                    Vec::new()
                };
                let args = self.parse_arg_list();
                CascadeOp::Call(name, type_args, args)
            } else if let Some(assign_op) = self.try_parse_assign_op() {
                let rhs = self.parse_expr();
                ops.push(CascadeOp::Assign(
                    Box::new(Expr::Ident(name)),
                    assign_op,
                    Box::new(rhs),
                ));
                return ops;
            } else {
                CascadeOp::Field(name, section_null_aware)
            }
        } else {
            return ops; // empty section
        };
        ops.push(first);
        // Subsequent selectors use a leading `.`/`?.` or `[`/`?[`, each optionally
        // invoked, with an optional terminal assignment.
        loop {
            match self.cur().kind {
                TokenKind::Dot | TokenKind::QmarkDot => {
                    let is_null_safe = self.at(TokenKind::QmarkDot);
                    self.advance();
                    // `.new` is a valid constructor tear-off in Dart 3.
                    let name = if self.at(TokenKind::New) {
                        let tok = self.advance();
                        Identifier::new("new", Self::tok_span(&tok))
                    } else {
                        self.expect_ident()
                    };
                    if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) {
                        let type_args = if self.at(TokenKind::Lt) {
                            self.parse_type_args()
                        } else {
                            Vec::new()
                        };
                        let args = self.parse_arg_list();
                        ops.push(CascadeOp::Call(name, type_args, args));
                    } else if let Some(assign_op) = self.try_parse_assign_op() {
                        let rhs = self.parse_expr();
                        ops.push(CascadeOp::Assign(
                            Box::new(Expr::Ident(name)),
                            assign_op,
                            Box::new(rhs),
                        ));
                        break;
                    } else {
                        ops.push(CascadeOp::Field(name, is_null_safe));
                    }
                }
                TokenKind::LBracket | TokenKind::QmarkLBracket => {
                    let is_null_safe = self.at(TokenKind::QmarkLBracket);
                    self.advance();
                    let idx = self.parse_expr();
                    self.expect(TokenKind::RBracket);
                    if let Some(assign_op) = self.try_parse_assign_op() {
                        let rhs = self.parse_expr();
                        ops.push(CascadeOp::Assign(Box::new(idx), assign_op, Box::new(rhs)));
                        break;
                    }
                    ops.push(CascadeOp::Index(Box::new(idx), is_null_safe));
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

    // ── Primary ───────────────────────────────────────────────────────────────

    pub(super) fn parse_primary(&mut self) -> Expr {
        let start = self.cur().offset;
        match self.cur().kind.clone() {
            TokenKind::IntLit => {
                let text = self.cur_text().to_string();
                self.advance();
                Expr::IntLit {
                    value: text,
                    span: self.span_from(start),
                }
            }
            TokenKind::DoubleLit => {
                let text = self.cur_text().to_string();
                self.advance();
                Expr::DoubleLit {
                    value: text,
                    span: self.span_from(start),
                }
            }
            TokenKind::StringLit => {
                // Adjacent string literals are implicitly concatenated in Dart.
                let mut node = self.parse_string_lit();
                while self.at(TokenKind::StringLit) {
                    let next = self.parse_string_lit();
                    let mut interpolations = node.interpolations;
                    interpolations.extend(next.interpolations);
                    node = StringLitNode {
                        raw: node.raw + &next.raw,
                        value: node.value + &next.value,
                        span: node.span.merge(&next.span),
                        interpolations,
                    };
                }
                Expr::StringLit(node)
            }
            TokenKind::True => {
                self.advance();
                Expr::BoolLit {
                    value: true,
                    span: self.span_from(start),
                }
            }
            TokenKind::False => {
                self.advance();
                Expr::BoolLit {
                    value: false,
                    span: self.span_from(start),
                }
            }
            TokenKind::Null => {
                self.advance();
                Expr::NullLit {
                    span: self.span_from(start),
                }
            }
            TokenKind::This => {
                self.advance();
                Expr::This {
                    span: self.span_from(start),
                }
            }
            TokenKind::Super => {
                self.advance();
                Expr::Super {
                    span: self.span_from(start),
                }
            }
            // Switch expression (primary form): `switch (expr) { pattern => expr, ... }`
            TokenKind::Switch => self.parse_switch_expr_primary(start),
            // Parenthesised / record / function expression
            TokenKind::LParen => {
                if self.looks_like_function_expr() {
                    // `(...)` followed by `{` is ambiguous: a lambda `(params) {body}`,
                    // or a parenthesized expression whose trailing `{` opens an
                    // *enclosing* block (e.g. a constructor body in `C(): g = (e) {}`).
                    // Speculatively parse the lambda; if its parameter list doesn't
                    // parse cleanly, roll back and treat it as a paren/record.
                    let saved = self.pos;
                    let saved_errors = self.errors.len();
                    let expr = self.parse_function_expr_with_type_params(Vec::new(), start);
                    if self.errors.len() > saved_errors {
                        self.rewind_to(saved);
                        self.errors.truncate(saved_errors);
                        self.parse_paren_or_record(false, start)
                    } else {
                        expr
                    }
                } else {
                    self.parse_paren_or_record(false, start)
                }
            }
            // Generic function expression: `<T>(params) => ...` / `<T>(params) { ... }`
            TokenKind::Lt if self.looks_like_generic_function_expr() => {
                let type_params = self.parse_type_params();
                self.parse_function_expr_with_type_params(type_params, start)
            }
            // Typed collection literal without const: <T>[...] or <K,V>{...}
            TokenKind::Lt => {
                let type_args = self.parse_type_args();
                match self.cur().kind {
                    TokenKind::LBracket => {
                        self.parse_list_literal(false, type_args.into_iter().next(), start)
                    }
                    TokenKind::LBrace => self.parse_map_or_set_literal(false, type_args, start),
                    _ => {
                        self.error(
                            "expected '[' or '{' after type arguments in collection literal"
                                .to_string(),
                        );
                        Expr::Error {
                            span: self.span_from(start),
                        }
                    }
                }
            }
            // List literal
            TokenKind::LBracket => self.parse_list_literal(false, None, start),
            // Map/set literal
            TokenKind::LBrace => self.parse_map_or_set_literal(false, Vec::new(), start),
            // Dot shorthand (Dart 3.9): `.name` / `.new` in a context position.
            TokenKind::Dot => self.parse_dot_shorthand(false, start),
            // const expression
            TokenKind::Const => {
                self.advance();
                self.parse_const_expr(start)
            }
            // new expression
            TokenKind::New => {
                self.advance();
                let dart_type = self.parse_type();
                let constructor_name = if self.eat(TokenKind::Dot).is_some() {
                    Some(self.expect_ident())
                } else {
                    None
                };
                let args = self.parse_arg_list();
                Expr::New {
                    is_const: false,
                    dart_type,
                    constructor_name,
                    args,
                    span: self.span_from(start),
                }
            }
            // Symbol literal:  #name  #foo.bar  #+  #[]  #[]=
            TokenKind::Hash => self.parse_symbol_literal(start),
            // function expression: (params) => / (params) {
            // handled by falling through to identifier parse then seeing no ident
            _ if self.is_ident_like() => {
                let id = self.expect_ident();
                Expr::Ident(id)
            }
            _ => {
                self.error(format!(
                    "unexpected token in expression: {:?}",
                    self.cur().kind
                ));
                self.advance();
                Expr::Error {
                    span: self.span_from(start),
                }
            }
        }
    }

    pub(super) fn parse_paren_or_record(&mut self, is_const: bool, start: usize) -> Expr {
        self.advance(); // (
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
            if self.is_ident_like() && self.peek(1).kind == TokenKind::Colon {
                let name = self.expect_ident();
                self.advance(); // :
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
            if self.is_ident_like() && self.peek(1).kind == TokenKind::Colon {
                let name = self.expect_ident();
                self.advance(); // :
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
    ///
    /// Slow path: scan forward to the matching `)` and check whether it is
    /// followed by `=>`, `async`, `sync`, or `{`.
    fn looks_like_function_expr(&self) -> bool {
        debug_assert_eq!(self.cur().kind, TokenKind::LParen);

        // Fast path — modifier keyword right after `(`
        if matches!(
            self.peek(1).kind,
            TokenKind::Final | TokenKind::Var | TokenKind::Required | TokenKind::Covariant
        ) {
            return true;
        }

        // Scan for matching `)` then inspect the following token.
        let mut depth = 0usize;
        let mut i = self.pos;
        while let Some(tok) = self.tokens.get(i) {
            match tok.kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        return matches!(
                            self.tokens.get(i + 1).map(|t| &t.kind),
                            Some(
                                TokenKind::Arrow
                                    | TokenKind::Async
                                    | TokenKind::Sync
                                    | TokenKind::LBrace
                            )
                        );
                    }
                }
                TokenKind::Eof | TokenKind::Semicolon => break,
                _ => {}
            }
            i += 1;
        }
        false
    }

    /// True when a `<` at the cursor opens the type-parameter list of a generic
    /// function expression: a balanced `<...>` closed immediately before a `(`
    /// whose matching `)` is followed by `=>`, `{`, `async`, or `sync`.
    fn looks_like_generic_function_expr(&self) -> bool {
        use TokenKind::*;
        debug_assert_eq!(self.cur().kind, Lt);
        // Find the `(` that immediately follows the balanced `<...>`.
        let mut depth = 0i32;
        let mut i = self.pos;
        let paren_idx = loop {
            let Some(tok) = self.tokens.get(i) else {
                return false;
            };
            match &tok.kind {
                Lt => depth += 1,
                Gt => depth -= 1,
                GtGt => depth -= 2,
                GtGtGt => depth -= 3,
                Ident | Dot | Comma | Qmark | Extends | Void | LParen | RParen | LBracket
                | RBracket | LBrace | RBrace => {}
                k if self.is_ident_like_kind(k) => {}
                _ => return false,
            }
            if depth <= 0 {
                break i + 1;
            }
            i += 1;
            if i - self.pos > 512 {
                return false;
            }
        };
        if self.tokens.get(paren_idx).map(|t| &t.kind) != Some(&LParen) {
            return false;
        }
        // Scan from `(` to its matching `)` and inspect the following token.
        let mut pdepth = 0i32;
        let mut j = paren_idx;
        while let Some(tok) = self.tokens.get(j) {
            match &tok.kind {
                LParen => pdepth += 1,
                RParen => {
                    pdepth -= 1;
                    if pdepth == 0 {
                        return matches!(
                            self.tokens.get(j + 1).map(|t| &t.kind),
                            Some(Arrow | LBrace | Async | Sync)
                        );
                    }
                }
                Eof => return false,
                _ => {}
            }
            j += 1;
        }
        false
    }

    fn parse_function_expr_with_type_params(
        &mut self,
        type_params: Vec<TypeParam>,
        start: usize,
    ) -> Expr {
        let params = self.parse_formal_param_list();
        let (is_async, is_generator) = self.parse_async_marker();
        // A closure's `=>` body is an expression, so it must NOT swallow the `;`
        // that terminates the enclosing declaration/statement — unlike a function
        // *declaration* body. `class C { final f = () => 0; }` relies on the `;`
        // reaching the field parser.
        let body = match self.cur().kind {
            TokenKind::Arrow => {
                let bstart = self.cur().offset;
                self.advance();
                let e = self.parse_expr();
                FunctionBody::Arrow(Box::new(e), self.span_from(bstart))
            }
            TokenKind::LBrace => FunctionBody::Block(self.parse_block()),
            _ => {
                self.error("expected function body after parameter list".to_string());
                FunctionBody::Block(Block {
                    stmts: Vec::new(),
                    span: self.span_from(start),
                })
            }
        };
        Expr::FuncExpr {
            type_params,
            params,
            is_async,
            is_generator,
            body: Box::new(body),
            span: self.span_from(start),
        }
    }

    fn parse_switch_expr_primary(&mut self, start: usize) -> Expr {
        self.advance(); // switch
        self.expect(TokenKind::LParen);
        let subject = self.parse_expr();
        self.expect(TokenKind::RParen);
        self.expect(TokenKind::LBrace);
        let mut arms = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let arm_start = self.cur().offset;
            let pattern = self.parse_pattern();
            let guard = if self.at(TokenKind::When) {
                self.advance();
                Some(self.parse_expr())
            } else {
                None
            };
            self.expect(TokenKind::Arrow);
            let body = self.parse_expr();
            arms.push(SwitchExprArm {
                pattern,
                guard,
                body,
                span: self.span_from(arm_start),
            });
            self.eat(TokenKind::Comma);
        }
        self.expect(TokenKind::RBrace);
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
        self.advance(); // [
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
        self.advance(); // {
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
                entries: Vec::new(),
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
            // Lower a plain map (no comprehension) to `entries` to keep the common
            // shape; only comprehension maps use `elements`.
            if elements.iter().all(|e| matches!(e, MapElement::Entry(_))) {
                let entries = elements
                    .into_iter()
                    .map(|e| match e {
                        MapElement::Entry(entry) => entry,
                        _ => unreachable!("only entries remain"),
                    })
                    .collect();
                Expr::Map {
                    is_const,
                    type_args,
                    entries,
                    elements: Vec::new(),
                    span: self.span_from(start),
                }
            } else {
                Expr::Map {
                    is_const,
                    type_args,
                    entries: Vec::new(),
                    elements,
                    span: self.span_from(start),
                }
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
        let saved = self.pos;
        let saved_errors = self.errors.len();
        let result = loop {
            while let TokenKind::For | TokenKind::If = self.cur().kind {
                self.advance();
                self.skip_balanced_parens();
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
        self.rewind_to(saved);
        self.errors.truncate(saved_errors);
        result
    }

    /// Skip a balanced `( ... )` group starting at the current token. Used by
    /// lookahead to step over a comprehension header without interpreting it.
    fn skip_balanced_parens(&mut self) {
        if !self.at(TokenKind::LParen) {
            return;
        }
        let mut depth = 0usize;
        loop {
            match self.cur().kind {
                TokenKind::LParen => {
                    depth += 1;
                    self.advance();
                }
                TokenKind::RParen => {
                    self.advance();
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                TokenKind::Eof => break,
                _ => {
                    self.advance();
                }
            }
        }
    }

    /// Parse one element of a comprehension-form map literal (`k: v`, or a
    /// `for`/`if`/spread wrapping such entries).
    fn parse_map_element(&mut self) -> MapElement {
        let start = self.cur().offset;
        match self.cur().kind {
            TokenKind::DotDotDot | TokenKind::DotDotDotQmark => {
                let is_null_aware = self.cur().kind == TokenKind::DotDotDotQmark;
                self.advance();
                let expr = self.parse_expr();
                MapElement::Spread {
                    expr,
                    is_null_aware,
                    span: self.span_from(start),
                }
            }
            TokenKind::If => {
                self.advance();
                let condition = self.parse_collection_if_condition();
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
                self.advance(); // await
                self.advance(); // for
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
                let is_null_aware = self.cur().kind == TokenKind::DotDotDotQmark;
                self.advance();
                let expr = self.parse_expr();
                CollectionElement::Spread {
                    expr,
                    is_null_aware,
                    span: self.span_from(start),
                }
            }
            TokenKind::If => {
                self.advance(); // if
                let condition = self.parse_collection_if_condition();
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
                self.advance(); // await
                self.advance(); // for
                self.finish_for_collection_element(true, start)
            }
            TokenKind::For => {
                self.advance(); // for
                self.finish_for_collection_element(false, start)
            }
            _ => CollectionElement::Expr(self.parse_expr()),
        }
    }

    /// Finish a collection `for`/`await for` element after the `for` keyword has
    /// been consumed, building either a for-in ([`CollectionElement::For`], which
    /// carries `is_await`) or a C-style ([`CollectionElement::CFor`]) element.
    fn finish_for_collection_element(&mut self, is_await: bool, start: usize) -> CollectionElement {
        match self.parse_collection_for_header() {
            ForHeader::ForIn {
                variable,
                var_type,
                pattern,
                iterable,
            } => {
                let element = Box::new(self.parse_collection_element());
                CollectionElement::For {
                    is_await,
                    variable,
                    var_type,
                    pattern,
                    iterable,
                    element,
                    span: self.span_from(start),
                }
            }
            ForHeader::CStyle {
                init,
                condition,
                updates,
            } => {
                let element = Box::new(self.parse_collection_element());
                CollectionElement::CFor {
                    init,
                    condition,
                    updates,
                    element,
                    span: self.span_from(start),
                }
            }
        }
    }

    /// Map-comprehension twin of [`finish_for_collection_element`].
    fn finish_for_map_element(&mut self, is_await: bool, start: usize) -> MapElement {
        match self.parse_collection_for_header() {
            ForHeader::ForIn {
                variable,
                var_type,
                pattern,
                iterable,
            } => {
                let entry = Box::new(self.parse_map_element());
                MapElement::For {
                    is_await,
                    variable,
                    var_type,
                    pattern,
                    iterable,
                    entry,
                    span: self.span_from(start),
                }
            }
            ForHeader::CStyle {
                init,
                condition,
                updates,
            } => {
                let entry = Box::new(self.parse_map_element());
                MapElement::CFor {
                    init,
                    condition,
                    updates,
                    entry,
                    span: self.span_from(start),
                }
            }
        }
    }

    /// Parse a collection/comprehension `if (...)` condition (the `if` keyword is
    /// already consumed): `(expr)` or `(expr case pattern [when guard])`. The
    /// optional `when` guard is retained in [`IfCondition::Case`].
    fn parse_collection_if_condition(&mut self) -> IfCondition {
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

    /// Parse a collection `for (...)` header (the `for` keyword is already
    /// consumed), returning either a for-in header (loop variable or Dart 3
    /// destructuring pattern plus the iterable) or a C-style header
    /// (`init ; cond ; update`). Leaves the closing `)` consumed.
    fn parse_collection_for_header(&mut self) -> ForHeader {
        self.expect(TokenKind::LParen);
        // A pattern-for header is `final`/`var` (or nothing) followed by a
        // destructuring pattern (`(a, b)`, `[a, b]`, `{..}`).
        let is_pattern_for = {
            let head_offset = if matches!(self.cur().kind, TokenKind::Var | TokenKind::Final) {
                1
            } else {
                0
            };
            matches!(
                self.peek(head_offset).kind,
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace
            )
        };
        if is_pattern_for {
            let _ = self
                .eat(TokenKind::Var)
                .or_else(|| self.eat(TokenKind::Final));
            let pattern = Some(Box::new(self.parse_binding_pattern()));
            self.eat(TokenKind::In);
            let iterable = self.parse_expr();
            self.expect(TokenKind::RParen);
            return ForHeader::ForIn {
                variable: None,
                var_type: None,
                pattern,
                iterable,
            };
        }
        // Reuse the statement for-clause parser, which handles both for-in and
        // C-style headers and consumes the closing `)`.
        let (init, condition, updates) = self.parse_for_clauses();
        match init {
            Some(ForInit::ForIn {
                var_type,
                name,
                iterable,
                ..
            }) => ForHeader::ForIn {
                variable: Some(name),
                var_type,
                pattern: None,
                iterable: *iterable,
            },
            other => ForHeader::CStyle {
                init: other,
                condition,
                updates,
            },
        }
    }

    /// Parses a dot shorthand head: `.name` or `.new`. The caller has already
    /// recorded `start`; the leading `.` is still current. Any trailing
    /// invocation or selector is left for `parse_postfix`.
    pub(super) fn parse_dot_shorthand(&mut self, is_const: bool, start: usize) -> Expr {
        self.advance(); // consume `.`
        let name = if self.at(TokenKind::New) {
            let tok = self.advance();
            Identifier::new("new", Self::tok_span(&tok))
        } else {
            self.expect_ident()
        };
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
        let hash = self.advance(); // #
        let mut end = hash.offset + hash.len;
        if self.is_ident_like() || self.at(Void) {
            let tok = self.advance();
            end = tok.offset + tok.len;
            while self.at(Dot) {
                let dot = self.advance();
                end = dot.offset + dot.len;
                if self.is_ident_like() || self.at(Void) {
                    let id = self.advance();
                    end = id.offset + id.len;
                } else {
                    self.error("expected identifier after '.' in symbol literal".to_string());
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
        } else if matches!(
            self.cur().kind,
            Lt | Gt
                | LtEq
                | GtEq
                | EqEq
                | Minus
                | Plus
                | Slash
                | TildeSlash
                | Star
                | Percent
                | Pipe
                | Caret
                | Amp
                | LtLt
                | GtGt
                | GtGtGt
                | Tilde
        ) {
            let op = self.advance();
            end = op.offset + op.len;
        } else {
            self.error("expected symbol name after '#'".to_string());
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
            TokenKind::Lt => {
                let type_args = self.parse_type_args();
                match self.cur().kind {
                    TokenKind::LBracket => {
                        self.parse_list_literal(true, type_args.into_iter().next(), start)
                    }
                    TokenKind::LBrace => self.parse_map_or_set_literal(true, type_args, start),
                    _ => {
                        self.error(
                            "expected '[' or '{' after type arguments in collection literal"
                                .to_string(),
                        );
                        Expr::Error {
                            span: self.span_from(start),
                        }
                    }
                }
            }
            _ => {
                // const constructor
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
