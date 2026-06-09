use jdlint_syntax::ast::*;
use jdlint_syntax::token::TokenKind;

use super::Parser;

impl<'src> Parser<'src> {
    // ── Public entry ──────────────────────────────────────────────────────────

    pub(super) fn parse_expr(&mut self) -> Expr {
        self.parse_assign()
    }

    // ── Assignment (right-associative) ────────────────────────────────────────

    fn parse_assign(&mut self) -> Expr {
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
        let rhs = self.parse_assign(); // right-associative
        let span = self.span_from(start);
        Expr::Assign { target: Box::new(lhs), op, value: Box::new(rhs), span }
    }

    // ── Conditional ternary ───────────────────────────────────────────────────

    fn parse_conditional(&mut self) -> Expr {
        let start = self.cur().offset;
        let cond = self.parse_null_coalesce();
        if self.eat(TokenKind::Qmark).is_some() {
            let then_expr = self.parse_expr();
            self.expect(TokenKind::Colon);
            let else_expr = self.parse_conditional();
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

    // ── Null coalescing ?? ────────────────────────────────────────────────────

    fn parse_null_coalesce(&mut self) -> Expr {
        let start = self.cur().offset;
        let mut lhs = self.parse_or();
        while self.eat(TokenKind::QmarkQmark).is_some() {
            let rhs = self.parse_or();
            let span = self.span_from(start);
            lhs = Expr::Binary { op: BinaryOp::NullCoalesce, left: Box::new(lhs), right: Box::new(rhs), span };
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
            lhs = Expr::Binary { op: BinaryOp::Or, left: Box::new(lhs), right: Box::new(rhs), span };
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
            lhs = Expr::Binary { op: BinaryOp::And, left: Box::new(lhs), right: Box::new(rhs), span };
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
        Expr::Binary { op, left: Box::new(lhs), right: Box::new(rhs), span: self.span_from(start) }
    }

    // ── Relational / type tests ───────────────────────────────────────────────

    fn parse_relational(&mut self) -> Expr {
        let start = self.cur().offset;
        let lhs = self.parse_bitwise_or();
        match self.cur().kind {
            TokenKind::Lt => { self.advance(); let r = self.parse_bitwise_or(); Expr::Binary { op: BinaryOp::Lt, left: Box::new(lhs), right: Box::new(r), span: self.span_from(start) } }
            TokenKind::Gt => { self.advance(); let r = self.parse_bitwise_or(); Expr::Binary { op: BinaryOp::Gt, left: Box::new(lhs), right: Box::new(r), span: self.span_from(start) } }
            TokenKind::LtEq => { self.advance(); let r = self.parse_bitwise_or(); Expr::Binary { op: BinaryOp::LtEq, left: Box::new(lhs), right: Box::new(r), span: self.span_from(start) } }
            TokenKind::GtEq => { self.advance(); let r = self.parse_bitwise_or(); Expr::Binary { op: BinaryOp::GtEq, left: Box::new(lhs), right: Box::new(r), span: self.span_from(start) } }
            TokenKind::Is => {
                self.advance();
                let negated = self.eat(TokenKind::Bang).is_some();
                let dart_type = self.parse_type();
                Expr::Is { expr: Box::new(lhs), dart_type, negated, span: self.span_from(start) }
            }
            TokenKind::As => {
                self.advance();
                let dart_type = self.parse_type();
                Expr::As { expr: Box::new(lhs), dart_type, span: self.span_from(start) }
            }
            _ => lhs,
        }
    }

    // ── Bitwise OR/XOR/AND ────────────────────────────────────────────────────

    fn parse_bitwise_or(&mut self) -> Expr {
        let start = self.cur().offset;
        let mut lhs = self.parse_bitwise_xor();
        while self.eat(TokenKind::Pipe).is_some() {
            let rhs = self.parse_bitwise_xor();
            lhs = Expr::Binary { op: BinaryOp::BitOr, left: Box::new(lhs), right: Box::new(rhs), span: self.span_from(start) };
        }
        lhs
    }

    fn parse_bitwise_xor(&mut self) -> Expr {
        let start = self.cur().offset;
        let mut lhs = self.parse_bitwise_and();
        while self.eat(TokenKind::Caret).is_some() {
            let rhs = self.parse_bitwise_and();
            lhs = Expr::Binary { op: BinaryOp::BitXor, left: Box::new(lhs), right: Box::new(rhs), span: self.span_from(start) };
        }
        lhs
    }

    fn parse_bitwise_and(&mut self) -> Expr {
        let start = self.cur().offset;
        let mut lhs = self.parse_shift();
        while self.eat(TokenKind::Amp).is_some() {
            let rhs = self.parse_shift();
            lhs = Expr::Binary { op: BinaryOp::BitAnd, left: Box::new(lhs), right: Box::new(rhs), span: self.span_from(start) };
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
            lhs = Expr::Binary { op, left: Box::new(lhs), right: Box::new(rhs), span: self.span_from(start) };
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
            lhs = Expr::Binary { op, left: Box::new(lhs), right: Box::new(rhs), span: self.span_from(start) };
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
            lhs = Expr::Binary { op, left: Box::new(lhs), right: Box::new(rhs), span: self.span_from(start) };
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
                Expr::Unary { op: UnaryOp::Minus, operand: Box::new(operand), span: self.span_from(start) }
            }
            TokenKind::Bang => {
                self.advance();
                let operand = self.parse_unary();
                Expr::Unary { op: UnaryOp::Bang, operand: Box::new(operand), span: self.span_from(start) }
            }
            TokenKind::Tilde => {
                self.advance();
                let operand = self.parse_unary();
                Expr::Unary { op: UnaryOp::Tilde, operand: Box::new(operand), span: self.span_from(start) }
            }
            TokenKind::PlusPlus => {
                self.advance();
                let operand = self.parse_unary();
                Expr::Unary { op: UnaryOp::PlusPlus, operand: Box::new(operand), span: self.span_from(start) }
            }
            TokenKind::MinusMinus => {
                self.advance();
                let operand = self.parse_unary();
                Expr::Unary { op: UnaryOp::MinusMinus, operand: Box::new(operand), span: self.span_from(start) }
            }
            TokenKind::Await => {
                self.advance();
                let operand = self.parse_unary();
                Expr::Await { expr: Box::new(operand), span: self.span_from(start) }
            }
            TokenKind::Throw => {
                self.advance();
                let operand = self.parse_expr();
                Expr::Throw { expr: Box::new(operand), span: self.span_from(start) }
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
                    expr = Expr::PostfixIncDec { op: PostfixIncDec::Increment, operand: Box::new(expr), span: self.span_from(start) };
                }
                TokenKind::MinusMinus => {
                    self.advance();
                    expr = Expr::PostfixIncDec { op: PostfixIncDec::Decrement, operand: Box::new(expr), span: self.span_from(start) };
                }
                // Null-assertion  !
                TokenKind::Bang => {
                    // Could be postfix null-assertion in Dart (expr!)
                    // Distinguish from unary ! by checking it follows a complete expression
                    self.advance();
                    expr = Expr::Unary { op: UnaryOp::Bang, operand: Box::new(expr), span: self.span_from(start) };
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
                    expr = Expr::Field { object: Box::new(expr), field, is_null_safe, span: self.span_from(start) };
                }
                // Index  [i]  ?[i]
                TokenKind::LBracket | TokenKind::QmarkLBracket => {
                    let is_null_safe = self.cur().kind == TokenKind::QmarkLBracket;
                    self.advance();
                    let index = self.parse_expr();
                    self.expect(TokenKind::RBracket);
                    expr = Expr::Index { object: Box::new(expr), index: Box::new(index), is_null_safe, span: self.span_from(start) };
                }
                // Call  (args)  with optional type args
                TokenKind::LParen => {
                    let args = self.parse_arg_list();
                    expr = Expr::Call { callee: Box::new(expr), type_args: Vec::new(), args, span: self.span_from(start) };
                }
                // Generic call  <T>(args)
                TokenKind::Lt => {
                    // Speculative: try to parse type args; restore errors on rollback
                    // to avoid spurious "expected type" errors in relational expressions.
                    let saved = self.pos;
                    let saved_errors = self.errors.len();
                    let type_args = self.parse_type_args();
                    if self.at(TokenKind::LParen) {
                        // Generic call: expr<T>(args)
                        let args = self.parse_arg_list();
                        expr = Expr::Call { callee: Box::new(expr), type_args, args, span: self.span_from(start) };
                    } else if self.at(TokenKind::Dot) || self.at(TokenKind::QmarkDot) {
                        // Type instantiation expression: Name<T>.method(args) — keep type args,
                        // represent as Call with empty args; next iteration handles the `.`.
                        let empty_args = ArgList { positional: Vec::new(), named: Vec::new(), span: self.span_from(start) };
                        expr = Expr::Call { callee: Box::new(expr), type_args, args: empty_args, span: self.span_from(start) };
                    } else {
                        // Not a call or type instantiation — restore position and any spurious errors
                        self.pos = saved;
                        self.errors.truncate(saved_errors);
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
                        arms.push(SwitchExprArm { pattern, guard, body, span: self.span_from(arm_start) });
                        self.eat(TokenKind::Comma);
                    }
                    self.expect(TokenKind::RBrace);
                    expr = Expr::Switch { subject: Box::new(expr), arms, span: self.span_from(start) };
                }
                // Cascade  ..  ?..
                TokenKind::DotDot | TokenKind::DotDotQmark => {
                    let sections = self.parse_cascade_sections();
                    expr = Expr::Cascade { object: Box::new(expr), sections, span: self.span_from(start) };
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
            self.advance(); // .. or ?..
            let op = if self.at(TokenKind::LBracket) {
                self.advance();
                let idx = self.parse_expr();
                self.expect(TokenKind::RBracket);
                // optional assignment
                if let Some(assign_op) = self.try_parse_assign_op() {
                    let rhs = self.parse_expr();
                    CascadeOp::Assign(Box::new(idx), assign_op, Box::new(rhs))
                } else {
                    CascadeOp::Index(Box::new(idx), false)
                }
            } else if self.is_ident_like() {
                let name = self.expect_ident();
                if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) {
                    let type_args = if self.at(TokenKind::Lt) { self.parse_type_args() } else { Vec::new() };
                    let args = self.parse_arg_list();
                    CascadeOp::Call(name, type_args, args)
                } else if let Some(assign_op) = self.try_parse_assign_op() {
                    let rhs = self.parse_expr();
                    let field_expr = Expr::Ident(name);
                    CascadeOp::Assign(Box::new(field_expr), assign_op, Box::new(rhs))
                } else {
                    CascadeOp::Field(name, false)
                }
            } else {
                break;
            };
            sections.push(CascadeSection { op, span: self.span_from(start) });
        }
        sections
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
                Expr::IntLit { value: text, span: self.span_from(start) }
            }
            TokenKind::DoubleLit => {
                let text = self.cur_text().to_string();
                self.advance();
                Expr::DoubleLit { value: text, span: self.span_from(start) }
            }
            TokenKind::StringLit => {
                // Adjacent string literals are implicitly concatenated in Dart.
                let mut node = self.parse_string_lit();
                while self.at(TokenKind::StringLit) {
                    let next = self.parse_string_lit();
                    node = StringLitNode {
                        raw: node.raw + &next.raw,
                        value: node.value + &next.value,
                        span: next.span,
                    };
                }
                Expr::StringLit(node)
            }
            TokenKind::True => {
                self.advance();
                Expr::BoolLit { value: true, span: self.span_from(start) }
            }
            TokenKind::False => {
                self.advance();
                Expr::BoolLit { value: false, span: self.span_from(start) }
            }
            TokenKind::Null => {
                self.advance();
                Expr::NullLit { span: self.span_from(start) }
            }
            TokenKind::This => {
                self.advance();
                Expr::This { span: self.span_from(start) }
            }
            TokenKind::Super => {
                self.advance();
                Expr::Super { span: self.span_from(start) }
            }
            // Switch expression (primary form): `switch (expr) { pattern => expr, ... }`
            TokenKind::Switch => self.parse_switch_expr_primary(start),
            // Parenthesised / record / function expression
            TokenKind::LParen => {
                if self.looks_like_function_expr() {
                    self.parse_function_expr(start)
                } else {
                    self.parse_paren_or_record(start)
                }
            }
            // Typed collection literal without const: <T>[...] or <K,V>{...}
            TokenKind::Lt => {
                let type_args = self.parse_type_args();
                match self.cur().kind {
                    TokenKind::LBracket => self.parse_list_literal(false, type_args.into_iter().next(), start),
                    TokenKind::LBrace => self.parse_map_or_set_literal(false, type_args, start),
                    _ => {
                        self.error("expected '[' or '{' after type arguments in collection literal".to_string());
                        Expr::Error { span: self.span_from(start) }
                    }
                }
            }
            // List literal
            TokenKind::LBracket => self.parse_list_literal(false, None, start),
            // Map/set literal
            TokenKind::LBrace => self.parse_map_or_set_literal(false, Vec::new(), start),
            // const expression
            TokenKind::Const => {
                self.advance();
                self.parse_const_expr(start)
            }
            // new expression
            TokenKind::New => {
                self.advance();
                let dart_type = self.parse_type();
                let constructor_name = if self.eat(TokenKind::Dot).is_some() { Some(self.expect_ident()) } else { None };
                let args = self.parse_arg_list();
                Expr::New { is_const: false, dart_type, constructor_name, args, span: self.span_from(start) }
            }
            // function expression: (params) => / (params) {
            // handled by falling through to identifier parse then seeing no ident
            _ if self.is_ident_like() => {
                let id = self.expect_ident();
                Expr::Ident(id)
            }
            _ => {
                self.error(format!("unexpected token in expression: {:?}", self.cur().kind));
                self.advance();
                Expr::Error { span: self.span_from(start) }
            }
        }
    }

    fn parse_paren_or_record(&mut self, start: usize) -> Expr {
        self.advance(); // (
        if self.at(TokenKind::RParen) {
            self.advance();
            return Expr::Record { fields: Vec::new(), span: self.span_from(start) };
        }
        // Detect named first field before calling parse_expr, so `:` stays in stream.
        let (first_name, first_value) = if self.is_ident_like() && self.peek(1).kind == TokenKind::Colon {
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
            let fields = vec![RecordField { name: first_name, value: first_value, span: self.span_from(start) }];
            return Expr::Record { fields, span: self.span_from(start) };
        }
        let mut fields = vec![RecordField { name: first_name, value: first_value, span: self.span_from(start) }];
        while self.eat(TokenKind::Comma).is_some() && !self.at(TokenKind::RParen) {
            let f_start = self.cur().offset;
            if self.is_ident_like() && self.peek(1).kind == TokenKind::Colon {
                let name = self.expect_ident();
                self.advance(); // :
                let value = self.parse_expr();
                fields.push(RecordField { name: Some(name), value, span: self.span_from(f_start) });
            } else {
                let value = self.parse_expr();
                fields.push(RecordField { name: None, value, span: self.span_from(f_start) });
            }
        }
        self.eat(TokenKind::Comma);
        self.expect(TokenKind::RParen);
        Expr::Record { fields, span: self.span_from(start) }
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
            TokenKind::Final
                | TokenKind::Var
                | TokenKind::Required
                | TokenKind::Covariant
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

    fn parse_function_expr(&mut self, start: usize) -> Expr {
        let params = self.parse_formal_param_list();
        let (_is_async, _is_generator) = self.parse_async_marker();
        let body = self.parse_function_body().unwrap_or_else(|| {
            self.error("expected function body after parameter list".to_string());
            FunctionBody::Block(Block { stmts: Vec::new(), span: self.span_from(start) })
        });
        Expr::FuncExpr { type_params: Vec::new(), params, body: Box::new(body), span: self.span_from(start) }
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
            arms.push(SwitchExprArm { pattern, guard, body, span: self.span_from(arm_start) });
            self.eat(TokenKind::Comma);
        }
        self.expect(TokenKind::RBrace);
        Expr::Switch { subject: Box::new(subject), arms, span: self.span_from(start) }
    }

    fn parse_list_literal(&mut self, is_const: bool, type_arg: Option<DartType>, start: usize) -> Expr {
        self.advance(); // [
        let mut elements = Vec::new();
        while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
            elements.push(self.parse_collection_element());
            if self.eat(TokenKind::Comma).is_none() { break; }
        }
        self.expect(TokenKind::RBracket);
        Expr::List { is_const, type_arg, elements, span: self.span_from(start) }
    }

    fn parse_map_or_set_literal(&mut self, is_const: bool, type_args: Vec<DartType>, start: usize) -> Expr {
        self.advance(); // {
        if self.at(TokenKind::RBrace) {
            self.advance();
            // Empty literal — default to map
            return Expr::Map { is_const, type_args, entries: Vec::new(), span: self.span_from(start) };
        }
        // Peek: if first element has a colon it's a map; otherwise set
        let first = self.parse_expr();
        if self.eat(TokenKind::Colon).is_some() {
            let value = self.parse_expr();
            let e_span = self.span_from(start);
            let mut entries = vec![MapEntry { key: first, value, span: e_span }];
            while self.eat(TokenKind::Comma).is_some() && !self.at(TokenKind::RBrace) {
                let k = self.parse_expr();
                self.expect(TokenKind::Colon);
                let v = self.parse_expr();
                let sp = self.span_from(start);
                entries.push(MapEntry { key: k, value: v, span: sp });
            }
            self.eat(TokenKind::Comma);
            self.expect(TokenKind::RBrace);
            Expr::Map { is_const, type_args, entries, span: self.span_from(start) }
        } else {
            let mut elements = vec![CollectionElement::Expr(first)];
            while self.eat(TokenKind::Comma).is_some() && !self.at(TokenKind::RBrace) {
                elements.push(self.parse_collection_element());
            }
            self.eat(TokenKind::Comma);
            self.expect(TokenKind::RBrace);
            let type_arg = type_args.into_iter().next();
            Expr::Set { is_const, type_arg, elements, span: self.span_from(start) }
        }
    }

    fn parse_collection_element(&mut self) -> CollectionElement {
        let start = self.cur().offset;
        match self.cur().kind {
            TokenKind::DotDotDot | TokenKind::DotDotDotQmark => {
                let is_null_aware = self.cur().kind == TokenKind::DotDotDotQmark;
                self.advance();
                let expr = self.parse_expr();
                CollectionElement::Spread { expr, is_null_aware, span: self.span_from(start) }
            }
            TokenKind::If => {
                self.advance(); // if
                self.expect(TokenKind::LParen);
                let condition = if self.at(TokenKind::Case) {
                    // if (expr case pattern)
                    let e = self.parse_expr();
                    self.advance(); // case
                    let p = self.parse_pattern();
                    IfCondition::Case(e, Box::new(p))
                } else {
                    IfCondition::Expr(self.parse_expr())
                };
                self.expect(TokenKind::RParen);
                let then_elem = Box::new(self.parse_collection_element());
                let else_elem = if self.eat(TokenKind::Else).is_some() {
                    Some(Box::new(self.parse_collection_element()))
                } else { None };
                CollectionElement::If { condition, then_elem, else_elem, span: self.span_from(start) }
            }
            TokenKind::For => {
                self.advance(); // for
                self.expect(TokenKind::LParen);
                // Consume optional var/final/const keyword
                let _ = self.eat(TokenKind::Var)
                    .or_else(|| self.eat(TokenKind::Final))
                    .or_else(|| self.eat(TokenKind::Const));
                // Try to parse an optional type annotation before the variable name
                let var_type = {
                    let saved = self.pos;
                    if self.is_type_start() && !self.at(TokenKind::LParen) {
                        let ty = self.parse_type();
                        if self.is_ident_like() { Some(ty) } else { self.pos = saved; None }
                    } else {
                        None
                    }
                };
                let variable = self.expect_ident();
                self.eat(TokenKind::In);
                let iterable = self.parse_expr();
                self.expect(TokenKind::RParen);
                let element = Box::new(self.parse_collection_element());
                CollectionElement::For { variable, var_type, iterable, element, span: self.span_from(start) }
            }
            _ => CollectionElement::Expr(self.parse_expr()),
        }
    }

    fn parse_const_expr(&mut self, start: usize) -> Expr {
        // const can prefix a constructor call or collection literal
        match self.cur().kind {
            TokenKind::LBracket => self.parse_list_literal(true, None, start),
            TokenKind::LBrace => self.parse_map_or_set_literal(true, Vec::new(), start),
            TokenKind::Lt => {
                let type_args = self.parse_type_args();
                match self.cur().kind {
                    TokenKind::LBracket => self.parse_list_literal(true, type_args.into_iter().next(), start),
                    TokenKind::LBrace => self.parse_map_or_set_literal(true, type_args, start),
                    _ => Expr::Error { span: self.span_from(start) },
                }
            }
            _ => {
                // const constructor
                let dart_type = self.parse_type();
                let constructor_name = if self.eat(TokenKind::Dot).is_some() { Some(self.expect_ident()) } else { None };
                let args = self.parse_arg_list();
                Expr::New { is_const: true, dart_type, constructor_name, args, span: self.span_from(start) }
            }
        }
    }

}

