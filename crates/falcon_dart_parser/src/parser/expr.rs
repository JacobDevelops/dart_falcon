use falcon_syntax::cst::{AssignOp, AsyncMarker, BinaryOp, Event, NodeKind};
use falcon_syntax::token::TokenKind;

use super::{CompletedMarker, Marker, Parser};

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
        for (offset, token) in self.tokens.iter_from(open)?.enumerate() {
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
        for (offset, token) in self.tokens.iter_from(open)?.enumerate() {
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

    pub(super) fn parse_adjacent_strings(&mut self) -> CompletedMarker {
        let marker = self.start();
        let first = self.parse_string_lit();
        let mut adjacent = false;
        while self.at(TokenKind::StringLit) {
            self.parse_string_lit();
            adjacent = true;
        }
        if adjacent {
            marker.complete(self, NodeKind::AdjacentStrings)
        } else {
            marker.abandon(self);
            first
        }
    }

    fn parse_switch_expr_arms(&mut self) {
        self.expect(TokenKind::LBrace);
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let before = self.pos;
            let arm = self.start();
            self.parse_pattern();
            if self.eat(TokenKind::When).is_some() {
                let marker = self.start();
                let arrow = self.switch_guard_arrow();
                self.with_ctx(|ctx| ctx.switch_guard_arrow = arrow, Self::parse_expr);
                marker.complete(self, NodeKind::ExprGuard);
            }
            self.expect(TokenKind::Arrow);
            self.parse_expr();
            arm.complete(self, NodeKind::SwitchExprArm);
            self.eat(TokenKind::Comma);
            if self.pos == before {
                break;
            }
        }
        self.expect(TokenKind::RBrace);
    }

    fn switch_guard_arrow(&self) -> Option<usize> {
        let mut depth = 0usize;
        let mut i = self.pos;
        loop {
            match self.tokens.get(i)?.kind {
                TokenKind::Arrow if depth == 0 => return Some(i),
                TokenKind::Eof => return None,
                TokenKind::Comma | TokenKind::RBrace | TokenKind::Semicolon if depth == 0 => {
                    return None;
                }
                TokenKind::Lt => {
                    if let Some(end) = self.after_generic_type_args(i) {
                        i = end;
                        continue;
                    }
                }
                TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::QmarkLBracket
                | TokenKind::LBrace => depth += 1,
                TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                    depth = depth.checked_sub(1)?;
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn parse_typed_collection(&mut self, marker: Marker) -> CompletedMarker {
        let type_arg_count = self.parse_type_args();
        match self.cur().kind {
            TokenKind::LBracket => self.parse_list_literal(marker),
            TokenKind::LBrace => self.parse_map_or_set_literal(type_arg_count, marker),
            _ => {
                self.error("expected '[' or '{' after type arguments in collection literal");
                marker.complete(self, NodeKind::ErrorExpr)
            }
        }
    }

    /// The front end's `looksLikeExpressionAfterAwaitOrYield` for an `await`
    /// at the cursor outside an async body: `await f();`, `await f.x`,
    /// `(await f)`, `await f + 1`, `await null` parse as (misplaced) await
    /// expressions; anything else leaves `await` an identifier. The
    /// unary-context-only `await f;` / `await f,` forms are not modeled: at
    /// statement level they declare variables of a type named `await`.
    fn await_operand_ahead(&self) -> bool {
        let next = self.peek(1).kind;
        if next == TokenKind::Null {
            return true;
        }
        if !next.is_identifier_like() {
            return false;
        }
        let binary = |kind: TokenKind| {
            (kind.is_user_definable_operator() && kind != TokenKind::Tilde)
                || matches!(
                    kind,
                    TokenKind::BangEq
                        | TokenKind::AmpAmp
                        | TokenKind::PipePipe
                        | TokenKind::QmarkQmark
                )
        };
        let after = self.peek(2).kind;
        if after == TokenKind::LParen {
            let Some(close) = self.matching_close(self.pos + 2, Brackets::Mixed, |_| false, None)
            else {
                return false;
            };
            let kind = self.kind_at(close + 1);
            return binary(kind)
                || matches!(
                    kind,
                    TokenKind::Semicolon
                        | TokenKind::Dot
                        | TokenKind::Comma
                        | TokenKind::DotDot
                        | TokenKind::Qmark
                        | TokenKind::QmarkDot
                        | TokenKind::RParen
                );
        }
        binary(after)
            || matches!(
                after,
                TokenKind::Dot | TokenKind::RParen | TokenKind::RBracket
            )
    }

    pub(super) fn parse_expr(&mut self) -> CompletedMarker {
        self.parse_assign()
    }

    fn parse_assign(&mut self) -> CompletedMarker {
        let lhs = self.parse_cascade();

        let Some(_) = self.try_parse_assign_op() else {
            return lhs;
        };
        let marker = lhs.precede(self);
        self.parse_assign(); // right-associative
        marker.complete(self, NodeKind::AssignExpr)
    }

    /// Cascade binds looser than every operator except assignment, so it wraps a
    /// whole conditional/relational/cast subject: `x as T ..m()` is `(x as T)..m()`
    /// and `a = b..c` is `a = (b..c)`. Parsed here — above the operator ladder —
    /// rather than in `parse_postfix`, which only ever sees a primary target.
    fn parse_cascade(&mut self) -> CompletedMarker {
        let object = self.parse_conditional();
        if matches!(self.cur().kind, TokenKind::DotDot | TokenKind::DotDotQmark) {
            let marker = object.precede(self);
            self.parse_cascade_sections();
            return marker.complete(self, NodeKind::CascadeExpr);
        }
        object
    }

    fn parse_conditional(&mut self) -> CompletedMarker {
        let cond = self.parse_null_coalesce();
        if self.eat(TokenKind::Qmark).is_some() {
            let marker = cond.precede(self);
            // Both branches are `expressionWithoutCascade` in the Dart grammar,
            // which *includes* assignment — `a ? b = c : d = e` is valid, with each
            // branch a right-associative assignment. (Cascades are excluded; a
            // cascade in a branch must be parenthesised.)
            self.parse_expr_without_cascade();
            self.expect(TokenKind::Colon);
            self.parse_expr_without_cascade();
            return marker.complete(self, NodeKind::ConditionalExpr);
        }
        cond
    }

    /// Parse an `expressionWithoutCascade`: an assignment whose left-hand side is a
    /// conditional (cascade-excluding) subject. Used for the branches of a ternary,
    /// where Dart permits assignment but not a bare cascade.
    fn parse_expr_without_cascade(&mut self) -> CompletedMarker {
        let lhs = self.parse_conditional();
        let Some(_) = self.try_parse_assign_op() else {
            return lhs;
        };
        let marker = lhs.precede(self);
        self.parse_expr_without_cascade(); // right-associative
        marker.complete(self, NodeKind::AssignExpr)
    }

    #[inline]
    fn parse_left_assoc(
        &mut self,
        next: fn(&mut Self) -> CompletedMarker,
        operator: fn(TokenKind) -> Option<BinaryOp>,
    ) -> CompletedMarker {
        let mut lhs = next(self);
        while operator(self.cur().kind).is_some() {
            let marker = lhs.precede(self);
            self.advance();
            next(self);
            lhs = marker.complete(self, NodeKind::BinaryExpr);
        }
        lhs
    }

    fn parse_null_coalesce(&mut self) -> CompletedMarker {
        self.parse_left_assoc(Self::parse_or, |kind| match kind {
            TokenKind::QmarkQmark => Some(BinaryOp::NullCoalesce),
            _ => None,
        })
    }

    fn parse_or(&mut self) -> CompletedMarker {
        self.parse_left_assoc(Self::parse_and, |kind| match kind {
            TokenKind::PipePipe => Some(BinaryOp::Or),
            _ => None,
        })
    }

    fn parse_and(&mut self) -> CompletedMarker {
        self.parse_left_assoc(Self::parse_equality, |kind| match kind {
            TokenKind::AmpAmp => Some(BinaryOp::And),
            _ => None,
        })
    }

    fn parse_equality(&mut self) -> CompletedMarker {
        let lhs = self.parse_relational();
        if !matches!(self.cur().kind, TokenKind::EqEq | TokenKind::BangEq) {
            return lhs;
        }
        let marker = lhs.precede(self);
        self.advance();
        self.parse_relational();
        marker.complete(self, NodeKind::BinaryExpr)
    }

    /// Scan a balanced `<...>` starting at `start` (which must be `<`),
    /// returning the token index immediately *after* the closing `>` when the
    /// span contains only type-ish tokens (including type parameter bounds),
    /// or `None` when some token means the `<` is really a comparison operator.
    /// Shared by generic-invocation, instantiation, and statement lookahead.
    pub(super) fn after_generic_type_args(&self, start: usize) -> Option<usize> {
        use TokenKind::*;
        debug_assert_eq!(self.kind_at(start), Lt);
        let mut depth = 0i32;
        // Angle depth captured at each open `(`/`[`/`{`. Records and function types
        // nest inside type arguments (`Foo<(int, Bar<T>)>`, `Foo<int Function()>`),
        // and their own generics must be tracked — but a `>`/`>>` may never close an
        // angle opened *outside* the current bracket group. That would be an
        // expression `>`/shift (`a < b ? (d >> e) : 0`), not a type-arg close.
        let mut bracket_stack: Vec<i32> = Vec::new();
        let mut i = start;
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
                At => {
                    i = self.index_after_annotations(i);
                    continue;
                }
                // Tokens that legitimately appear inside a type-argument list
                // (named/qualified types, records, function types, nullability).
                Dot | Comma | Qmark | Void | Extends => {}
                k if k.is_identifier_like() => {}
                // Anything else (operators, literals, `=>`, `:`, `;`, …) means
                // this `<` is a comparison, not type arguments.
                _ => return None,
            }
            i += 1;
            if i - start > 512 {
                return None;
            }
        }
        None
    }

    /// True when the `<` at the cursor opens a generic-invocation type-argument
    /// list — a balanced `<...>` closed immediately before `(`, `.`, or `?.`.
    /// Distinguishes `f<T>()` from the comparison in `a < b ? () => c() : null`.
    fn is_generic_invocation(&self) -> bool {
        self.after_generic_type_args(self.pos)
            .is_some_and(|idx| self.after_type_args_is_call(idx))
    }

    /// True when the `<` at the cursor opens a bare generic-instantiation
    /// tear-off (`identity<int>`) — a balanced `<...>` *not* followed by a call
    /// token, whose following token cannot continue a `<` comparison (Dart's own
    /// disambiguation rule, per the constructor-tearoffs spec).
    fn is_generic_instantiation(&self) -> bool {
        match self.after_generic_type_args(self.pos) {
            Some(idx) if !self.after_type_args_is_call(idx) => {
                self.token_ends_generic_instantiation(idx)
            }
            _ => false,
        }
    }

    pub(super) fn after_type_args_is_call(&self, idx: usize) -> bool {
        matches!(
            self.tokens.get(idx).map(|t| &t.kind),
            Some(TokenKind::LParen | TokenKind::Dot | TokenKind::QmarkDot)
        )
    }

    /// True when the token at `idx` (following a balanced `<...>`) cannot begin
    /// or continue the right-hand side of a `<` comparison, so the `<...>` must
    /// be a generic-instantiation type-argument list. `f(a < b, c > d)` stays a
    /// comparison because an identifier follows `>`, which is *not* in this set.
    pub(super) fn token_ends_generic_instantiation(&self, idx: usize) -> bool {
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
    fn parse_type_after_is_as(&mut self) -> CompletedMarker {
        self.with_ctx(
            |ctx| ctx.suppress_conditional_qmark = true,
            Self::parse_type,
        )
    }

    fn parse_relational(&mut self) -> CompletedMarker {
        let lhs = self.parse_cast();
        if !matches!(
            self.cur().kind,
            TokenKind::Lt | TokenKind::Gt | TokenKind::LtEq | TokenKind::GtEq
        ) {
            return lhs;
        }
        let marker = lhs.precede(self);
        self.advance();
        self.parse_cast();
        marker.complete(self, NodeKind::BinaryExpr)
    }

    /// Type test / cast level (`is`, `is!`, `as`), which binds tighter than the
    /// relational comparison operators — so `a < b as C` is `a < (b as C)` and
    /// `x as int <= y` is `(x as int) <= y`. Loops to allow chains (`a as B as C`).
    fn parse_cast(&mut self) -> CompletedMarker {
        let mut lhs = self.parse_bitwise_or();
        loop {
            match self.cur().kind {
                TokenKind::Is => {
                    let marker = lhs.precede(self);
                    self.advance();
                    self.eat(TokenKind::Bang);
                    self.parse_type_after_is_as();
                    lhs = marker.complete(self, NodeKind::IsExpr);
                }
                TokenKind::As => {
                    let marker = lhs.precede(self);
                    self.advance();
                    self.parse_type_after_is_as();
                    lhs = marker.complete(self, NodeKind::AsExpr);
                }
                _ => break,
            }
        }
        lhs
    }

    pub(super) fn parse_bitwise_or(&mut self) -> CompletedMarker {
        self.parse_left_assoc(Self::parse_bitwise_xor, |kind| match kind {
            TokenKind::Pipe => Some(BinaryOp::BitOr),
            _ => None,
        })
    }

    fn parse_bitwise_xor(&mut self) -> CompletedMarker {
        self.parse_left_assoc(Self::parse_bitwise_and, |kind| match kind {
            TokenKind::Caret => Some(BinaryOp::BitXor),
            _ => None,
        })
    }

    fn parse_bitwise_and(&mut self) -> CompletedMarker {
        self.parse_left_assoc(Self::parse_shift, |kind| match kind {
            TokenKind::Amp => Some(BinaryOp::BitAnd),
            _ => None,
        })
    }

    fn parse_shift(&mut self) -> CompletedMarker {
        self.parse_left_assoc(Self::parse_additive, |kind| match kind {
            TokenKind::LtLt => Some(BinaryOp::Shl),
            TokenKind::GtGt => Some(BinaryOp::Shr),
            TokenKind::GtGtGt => Some(BinaryOp::UShr),
            _ => None,
        })
    }

    fn parse_additive(&mut self) -> CompletedMarker {
        self.parse_left_assoc(Self::parse_multiplicative, |kind| match kind {
            TokenKind::Plus => Some(BinaryOp::Add),
            TokenKind::Minus => Some(BinaryOp::Sub),
            _ => None,
        })
    }

    fn parse_multiplicative(&mut self) -> CompletedMarker {
        self.parse_left_assoc(Self::parse_unary, |kind| match kind {
            TokenKind::Star => Some(BinaryOp::Mul),
            TokenKind::Slash => Some(BinaryOp::Div),
            TokenKind::Percent => Some(BinaryOp::Mod),
            TokenKind::TildeSlash => Some(BinaryOp::IntDiv),
            _ => None,
        })
    }

    fn parse_unary(&mut self) -> CompletedMarker {
        self.guarded(
            |p| p.missing(NodeKind::ErrorExpr),
            |p| {
                if matches!(
                    p.cur().kind,
                    TokenKind::Minus
                        | TokenKind::Bang
                        | TokenKind::Tilde
                        | TokenKind::PlusPlus
                        | TokenKind::MinusMinus
                ) {
                    let marker = p.start();
                    p.advance();
                    p.parse_unary();
                    return marker.complete(p, NodeKind::UnaryExpr);
                }
                match p.cur().kind {
                    // Outside async and generator bodies `await` is an identifier,
                    // except where it plainly leads an operand (`await foo();`): the
                    // front end parses that as a misplaced await expression, and so do we.
                    TokenKind::Await
                        if p.ctx.body != AsyncMarker::Sync || p.await_operand_ahead() =>
                    {
                        let marker = p.start();
                        p.advance();
                        p.parse_unary();
                        marker.complete(p, NodeKind::AwaitExpr)
                    }
                    TokenKind::Throw => {
                        let marker = p.start();
                        p.advance();
                        p.parse_expr();
                        marker.complete(p, NodeKind::ThrowExpr)
                    }
                    _ => p.parse_postfix(),
                }
            },
        )
    }

    fn parse_postfix(&mut self) -> CompletedMarker {
        self.guarded(
            |p| p.missing(NodeKind::ErrorExpr),
            |p| {
                let mut expr = p.parse_primary();
                loop {
                    let kind = match p.cur().kind {
                        TokenKind::PlusPlus | TokenKind::MinusMinus => {
                            p.advance();
                            NodeKind::PostfixIncDecExpr
                        }
                        TokenKind::Switch => {
                            p.advance();
                            p.parse_switch_expr_arms();
                            NodeKind::SwitchExpr
                        }
                        _ => {
                            let Some(kind) = p.parse_selector(true) else {
                                break;
                            };
                            kind
                        }
                    };
                    let marker = expr.precede(p);
                    expr = marker.complete(p, kind);
                }
                expr
            },
        )
    }

    /// Shared selector grammar; cascades cannot represent bare generic tear-offs.
    fn parse_selector(&mut self, allow_instantiation: bool) -> Option<NodeKind> {
        Some(match self.cur().kind {
            TokenKind::Bang => {
                self.advance();
                NodeKind::NullAssertExpr
            }
            TokenKind::Dot | TokenKind::QmarkDot => {
                self.advance();
                self.expect_ctor_name();
                NodeKind::FieldExpr
            }
            TokenKind::LBracket | TokenKind::QmarkLBracket => {
                self.advance();
                self.parse_expr();
                self.expect(TokenKind::RBracket);
                NodeKind::IndexExpr
            }
            TokenKind::LParen => {
                self.parse_arg_list();
                NodeKind::CallExpr
            }
            TokenKind::Lt if self.is_generic_invocation() => {
                return self.speculate(|p| {
                    p.parse_type_args();
                    if p.at(TokenKind::LParen) {
                        p.parse_arg_list();
                        Some(NodeKind::CallExpr)
                    } else if allow_instantiation
                        && matches!(p.cur().kind, TokenKind::Dot | TokenKind::QmarkDot)
                    {
                        Some(NodeKind::GenericInstantiationExpr)
                    } else {
                        None
                    }
                });
            }
            TokenKind::Lt if allow_instantiation && self.is_generic_instantiation() => {
                self.parse_type_args();
                NodeKind::GenericInstantiationExpr
            }
            _ => return None,
        })
    }

    fn parse_cascade_sections(&mut self) {
        while matches!(self.cur().kind, TokenKind::DotDot | TokenKind::DotDotQmark) {
            let marker = self.start();
            self.advance();
            if !self.parse_cascade_section_ops() {
                marker.abandon(self);
                break;
            }
            marker.complete(self, NodeKind::CascadeSection);
        }
    }

    /// Parse the selector chain of a single cascade section (everything after the
    /// `..`/`?..`). Dart allows a full assignable-selector chain here — `..a.b()`,
    /// `..m().n()`, `..a[i]=x`, `..a?.b()` — not just one selector. It shares
    /// postfix selector parsing but seeds the first selector as a bare identifier
    /// or `[index]` (no leading dot) and stops at a terminal assignment.
    fn parse_cascade_section_ops(&mut self) -> bool {
        // First selector after `..`: bare identifier or `[index]`, no leading dot.
        let marker = self.start();
        if matches!(
            self.cur().kind,
            TokenKind::LBracket | TokenKind::QmarkLBracket
        ) {
            let _ = self.parse_selector(false);
            if self.try_parse_assign_op().is_some() {
                self.parse_expr_without_cascade();
                marker.complete(self, NodeKind::CascadeIndexAssign);
                return true; // assignment terminates the section
            }
            marker.complete(self, NodeKind::CascadeIndex);
        } else if self.cur().kind.is_identifier_like() {
            self.expect_ident();
            if matches!(self.cur().kind, TokenKind::LParen | TokenKind::Lt)
                && self.parse_selector(false) == Some(NodeKind::CallExpr)
            {
                marker.complete(self, NodeKind::CascadeCall);
            } else if self.try_parse_assign_op().is_some() {
                self.parse_expr_without_cascade();
                marker.complete(self, NodeKind::CascadeAssign);
                return true;
            } else {
                marker.complete(self, NodeKind::CascadeField);
            }
        } else {
            marker.abandon(self);
            return false; // empty section
        }
        loop {
            let marker = self.start();
            let Some(selector) = self.parse_selector(false) else {
                marker.abandon(self);
                break;
            };
            let kind = match selector {
                NodeKind::FieldExpr => {
                    if matches!(self.cur().kind, TokenKind::LParen | TokenKind::Lt)
                        && self.parse_selector(false) == Some(NodeKind::CallExpr)
                    {
                        NodeKind::CascadeCall
                    } else if self.try_parse_assign_op().is_some() {
                        self.parse_expr_without_cascade();
                        marker.complete(self, NodeKind::CascadeAssign);
                        break;
                    } else {
                        NodeKind::CascadeField
                    }
                }
                NodeKind::IndexExpr => {
                    if self.try_parse_assign_op().is_some() {
                        self.parse_expr_without_cascade();
                        marker.complete(self, NodeKind::CascadeIndexAssign);
                        break;
                    }
                    NodeKind::CascadeIndex
                }
                NodeKind::NullAssertExpr => NodeKind::CascadeNullAssert,
                NodeKind::CallExpr => NodeKind::CascadeInvoke,
                _ => unreachable!("cascade selector grammar returned a non-selector"),
            };
            marker.complete(self, kind);
        }
        true
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

    pub(super) fn parse_primary(&mut self) -> CompletedMarker {
        self.guarded(
            |p| p.missing(NodeKind::ErrorExpr),
            |p| {
                let start = p.cur().offset();
                match p.cur().kind {
                    TokenKind::IntLit
                    | TokenKind::DoubleLit
                    | TokenKind::True
                    | TokenKind::False
                    | TokenKind::Null
                    | TokenKind::This
                    | TokenKind::Super => {
                        let marker = p.start();
                        let kind = match p.cur().kind {
                            TokenKind::IntLit => NodeKind::IntLitExpr,
                            TokenKind::DoubleLit => NodeKind::DoubleLitExpr,
                            TokenKind::True | TokenKind::False => NodeKind::BoolLitExpr,
                            TokenKind::Null => NodeKind::NullLitExpr,
                            TokenKind::This => NodeKind::ThisExpr,
                            TokenKind::Super => NodeKind::SuperExpr,
                            _ => unreachable!(),
                        };
                        p.advance();
                        marker.complete(p, kind)
                    }
                    TokenKind::StringLit => p.parse_adjacent_strings(),
                    TokenKind::Switch => p.parse_switch_expr_primary(start),
                    TokenKind::LParen => {
                        if p.looks_like_function_expr() {
                            let marker = p.start_at(start);
                            p.parse_function_expr(marker)
                        } else {
                            p.parse_paren_or_record(start)
                        }
                    }
                    TokenKind::Lt => p
                        .speculate(|p| {
                            let errors = p.errors.len();
                            let marker = p.start_at(start);
                            p.parse_type_params();
                            if p.errors.len() != errors
                                || !p.at(TokenKind::LParen)
                                || !p.looks_like_function_expr()
                            {
                                return None;
                            }
                            Some(p.parse_function_expr(marker))
                        })
                        .unwrap_or_else(|| {
                            let marker = p.start_at(start);
                            p.parse_typed_collection(marker)
                        }),
                    TokenKind::LBracket => {
                        let marker = p.start();
                        p.parse_list_literal(marker)
                    }
                    TokenKind::LBrace => {
                        let marker = p.start();
                        p.parse_map_or_set_literal(0, marker)
                    }
                    // Dot shorthand (Dart 3.9): `.name` / `.new` in a context position.
                    TokenKind::Dot => p.parse_dot_shorthand(start),
                    TokenKind::Const => {
                        let marker = p.start();
                        p.advance();
                        p.parse_const_expr(marker)
                    }
                    TokenKind::New => {
                        let marker = p.start();
                        p.advance();
                        p.parse_type();
                        if p.eat(TokenKind::Dot).is_some() {
                            p.expect_ident();
                        }
                        p.parse_arg_list();
                        marker.complete(p, NodeKind::NewExpr)
                    }
                    TokenKind::Hash => p.parse_symbol_literal(start),
                    _ if p.cur().kind.is_identifier_like() => p.expect_ident(),
                    _ => {
                        let marker = p.start();
                        p.error(format!(
                            "unexpected token in expression: {:?}",
                            p.cur().kind
                        ));
                        p.advance();
                        marker.complete(p, NodeKind::ErrorExpr)
                    }
                }
            },
        )
    }

    pub(super) fn parse_paren_or_record(&mut self, start: usize) -> CompletedMarker {
        let marker = self.start_at(start);
        self.parse_paren_or_record_inner(marker)
    }

    pub(super) fn parse_paren_or_record_inner(&mut self, marker: Marker) -> CompletedMarker {
        self.with_ctx(
            |ctx| ctx.in_ctor_init_value = false,
            |p| p.parse_paren_or_record_contents(marker),
        )
    }

    fn parse_paren_or_record_contents(&mut self, marker: Marker) -> CompletedMarker {
        self.advance();
        if self.at(TokenKind::RParen) {
            self.advance();
            return marker.complete(self, NodeKind::RecordExpr);
        }
        // Detect named first field before calling parse_expr, so `:` stays in stream.
        let field = self.start();
        let first_named =
            self.cur().kind.is_identifier_like() && self.peek(1).kind == TokenKind::Colon;
        if first_named {
            self.expect_ident();
            self.advance();
        }
        self.parse_expr();
        if self.eat(TokenKind::RParen).is_some() {
            if !first_named {
                field.abandon(self);
                return marker.complete(self, NodeKind::ParenExpr);
            }
            field.complete(self, NodeKind::RecordField);
            return marker.complete(self, NodeKind::RecordExpr);
        }
        field.complete(self, NodeKind::RecordField);
        while self.eat(TokenKind::Comma).is_some() && !self.at(TokenKind::RParen) {
            let field = self.start();
            if self.cur().kind.is_identifier_like() && self.peek(1).kind == TokenKind::Colon {
                self.expect_ident();
                self.advance();
            }
            self.parse_expr();
            field.complete(self, NodeKind::RecordField);
        }
        self.eat(TokenKind::Comma);
        self.expect(TokenKind::RParen);
        marker.complete(self, NodeKind::RecordExpr)
    }

    /// Returns `true` when the `(` at `self.pos` starts a function expression
    /// rather than a parenthesised expression or record literal.
    ///
    /// A switch guard's top-level arm arrow takes precedence over lambda
    /// lookahead, including contextual modifier keywords used as expressions.
    /// Otherwise a modifier keyword (`final`, `var`, `required`, `covariant`)
    /// immediately inside the parens signals a parameter list.
    fn looks_like_function_expr(&self) -> bool {
        debug_assert_eq!(self.cur().kind, TokenKind::LParen);

        let close = self.matching_close(
            self.pos,
            Brackets::ParenOnly,
            |kind| kind == TokenKind::Semicolon,
            None,
        );
        if close.is_some_and(|i| Some(i + 1) == self.ctx.switch_guard_arrow) {
            return false;
        }
        if self.ctx.in_ctor_init_value
            && close.is_some_and(|i| self.kind_at(i + 1) == TokenKind::LBrace)
        {
            return false;
        }

        if matches!(self.peek(1).kind, TokenKind::Final | TokenKind::Var) {
            return true;
        }

        close.is_some_and(|i| {
            matches!(
                self.kind_at(i + 1),
                TokenKind::Arrow | TokenKind::Async | TokenKind::Sync | TokenKind::LBrace
            )
        })
    }

    fn parse_function_expr(&mut self, marker: Marker) -> CompletedMarker {
        self.parse_formal_param_list();
        let async_marker = self.parse_async_marker();
        // Closure arrows leave the enclosing declaration's semicolon untouched.
        self.with_body(async_marker, |p| match p.cur().kind {
            TokenKind::Arrow => {
                let marker = p.start();
                p.advance();
                p.parse_expr();
                marker.complete(p, NodeKind::ArrowBody)
            }
            TokenKind::LBrace => p.parse_block(),
            _ => {
                p.error("expected function body after parameter list");
                p.missing(NodeKind::Block)
            }
        });
        marker.complete(self, NodeKind::FuncExpr)
    }

    fn parse_switch_expr_primary(&mut self, start: usize) -> CompletedMarker {
        let marker = self.start_at(start);
        self.advance();
        self.expect(TokenKind::LParen);
        self.parse_expr();
        self.expect(TokenKind::RParen);
        self.parse_switch_expr_arms();
        marker.complete(self, NodeKind::SwitchExpr)
    }

    fn parse_list_literal(&mut self, marker: Marker) -> CompletedMarker {
        self.with_ctx(
            |ctx| ctx.in_ctor_init_value = false,
            |p| p.parse_list_literal_contents(marker),
        )
    }

    fn parse_list_literal_contents(&mut self, marker: Marker) -> CompletedMarker {
        self.advance();
        while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
            self.parse_collection_element();
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect(TokenKind::RBracket);
        marker.complete(self, NodeKind::ListExpr)
    }

    fn parse_map_or_set_literal(
        &mut self,
        type_arg_count: usize,
        marker: Marker,
    ) -> CompletedMarker {
        self.with_ctx(
            |ctx| ctx.in_ctor_init_value = false,
            |p| p.parse_map_or_set_literal_contents(type_arg_count, marker),
        )
    }

    fn parse_map_or_set_literal_contents(
        &mut self,
        type_arg_count: usize,
        marker: Marker,
    ) -> CompletedMarker {
        self.advance();
        if self.at(TokenKind::RBrace) {
            self.advance();
            // Empty braces: a single type arg (`<int>{}`) is a Set; no type args
            // (`{}`) or two (`<K, V>{}`) is a Map.
            return marker.complete(
                self,
                if type_arg_count == 1 {
                    NodeKind::SetExpr
                } else {
                    NodeKind::MapExpr
                },
            );
        }
        let mut is_map = None;
        let mut ambiguous = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            self.parse_brace_element(&mut is_map, &mut ambiguous);
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        let is_map = is_map.unwrap_or(type_arg_count == 2);
        if is_map {
            for (pos, map_kind) in ambiguous {
                let Event::Start { kind, .. } = &mut self.events[pos] else {
                    panic!("ambiguous collection element must be a completed start event");
                };
                *kind = map_kind;
            }
        }
        self.expect(TokenKind::RBrace);
        marker.complete(
            self,
            if is_map {
                NodeKind::MapExpr
            } else {
                NodeKind::SetExpr
            },
        )
    }

    fn parse_spread(&mut self) {
        self.advance();
        self.parse_expr();
    }

    fn parse_brace_element(
        &mut self,
        is_map: &mut Option<bool>,
        ambiguous: &mut Vec<(usize, NodeKind)>,
    ) -> CompletedMarker {
        if !self.at_any(&[TokenKind::If, TokenKind::For])
            && !(self.at(TokenKind::Await) && self.peek(1).kind == TokenKind::For)
        {
            return self.parse_brace_element_inner(is_map, ambiguous);
        }
        if let Some(element) = self.guarded(
            |_| None,
            |p| Some(p.parse_brace_element_inner(is_map, ambiguous)),
        ) {
            return element;
        }
        let marker = self.start();
        self.missing(NodeKind::ErrorExpr);
        self.missing(NodeKind::ErrorExpr);
        let kind = if *is_map == Some(true) {
            NodeKind::MapEntry
        } else {
            if is_map.is_none() {
                ambiguous.push((marker.pos, NodeKind::MapEntry));
            }
            NodeKind::ErrorExpr
        };
        marker.complete(self, kind)
    }

    fn parse_brace_element_inner(
        &mut self,
        is_map: &mut Option<bool>,
        ambiguous: &mut Vec<(usize, NodeKind)>,
    ) -> CompletedMarker {
        let marker = self.start();
        let (set_kind, map_kind) = match self.cur().kind {
            TokenKind::DotDotDot | TokenKind::DotDotDotQmark => {
                self.parse_spread();
                (NodeKind::SpreadElement, NodeKind::MapSpreadElement)
            }
            TokenKind::If => {
                self.advance();
                let condition = self.start();
                self.parse_if_condition();
                condition.complete(self, NodeKind::ExprCondition);
                self.parse_brace_element(is_map, ambiguous);
                if self.eat(TokenKind::Else).is_some() {
                    self.parse_brace_element(is_map, ambiguous);
                }
                (NodeKind::IfElement, NodeKind::MapIfElement)
            }
            TokenKind::For | TokenKind::Await
                if self.at(TokenKind::For) || self.peek(1).kind == TokenKind::For =>
            {
                let is_await = self.eat(TokenKind::Await).is_some();
                self.advance();
                self.parse_collection_for_header(is_await);
                self.parse_brace_element(is_map, ambiguous);
                (NodeKind::ForElement, NodeKind::MapForElement)
            }
            _ => {
                let null_aware = self.eat(TokenKind::Qmark).is_some();
                let before = self.pos;
                let element = self.parse_expr();
                if self.pos == before && is_map.is_none() {
                    self.missing(NodeKind::ErrorExpr);
                    ambiguous.push((marker.pos, NodeKind::MapEntry));
                    return marker.complete(self, NodeKind::ErrorExpr);
                }
                if self.eat(TokenKind::Colon).is_some() {
                    if *is_map == Some(false) {
                        self.error("map entry in set literal");
                    }
                    is_map.get_or_insert(true);
                    self.eat(TokenKind::Qmark);
                    self.parse_expr();
                    return marker.complete(
                        self,
                        if *is_map == Some(false) {
                            NodeKind::ErrorExpr
                        } else {
                            NodeKind::MapEntry
                        },
                    );
                }
                if *is_map == Some(true) {
                    self.expect(TokenKind::Colon);
                    self.missing(NodeKind::ErrorExpr);
                    return marker.complete(self, NodeKind::MapEntry);
                }
                is_map.get_or_insert(false);
                if null_aware {
                    return marker.complete(self, NodeKind::NullAwareElement);
                }
                marker.abandon(self);
                return element;
            }
        };
        match is_map {
            Some(true) => marker.complete(self, map_kind),
            Some(false) => marker.complete(self, set_kind),
            None => {
                ambiguous.push((marker.pos, map_kind));
                marker.complete(self, set_kind)
            }
        }
    }

    fn parse_collection_element(&mut self) -> CompletedMarker {
        if !self.at_any(&[TokenKind::If, TokenKind::For])
            && !(self.at(TokenKind::Await) && self.peek(1).kind == TokenKind::For)
        {
            return self.parse_collection_element_inner();
        }
        self.guarded(
            |p| p.missing(NodeKind::ErrorExpr),
            Self::parse_collection_element_inner,
        )
    }

    fn parse_collection_element_inner(&mut self) -> CompletedMarker {
        let marker = self.start();
        match self.cur().kind {
            // Dart 3.0 null-aware element `?expr`
            TokenKind::Qmark => {
                self.advance();
                self.parse_expr();
                marker.complete(self, NodeKind::NullAwareElement)
            }
            TokenKind::DotDotDot | TokenKind::DotDotDotQmark => {
                self.parse_spread();
                marker.complete(self, NodeKind::SpreadElement)
            }
            TokenKind::If => {
                self.advance();
                let condition_marker = self.start();
                self.parse_if_condition();
                condition_marker.complete(self, NodeKind::ExprCondition);
                self.parse_collection_element();
                if self.eat(TokenKind::Else).is_some() {
                    self.parse_collection_element();
                }
                marker.complete(self, NodeKind::IfElement)
            }
            // `await for (...)` — an asynchronous for-in element (async context).
            TokenKind::Await if self.peek(1).kind == TokenKind::For => {
                self.advance();
                self.advance();
                self.finish_for_collection_element(true);
                marker.complete(self, NodeKind::ForElement)
            }
            TokenKind::For => {
                self.advance();
                self.finish_for_collection_element(false);
                marker.complete(self, NodeKind::ForElement)
            }
            _ => {
                marker.abandon(self);
                self.parse_expr()
            }
        }
    }

    fn finish_for_collection_element(&mut self, is_await: bool) {
        self.parse_collection_for_header(is_await);
        self.parse_collection_element();
    }

    pub(super) fn parse_if_condition(&mut self) -> CompletedMarker {
        let marker = self.start();
        self.expect(TokenKind::LParen);
        let scrutinee = self.parse_expr();
        let condition = if self.eat(TokenKind::Case).is_some() {
            self.parse_pattern();
            if self.eat(TokenKind::When).is_some() {
                self.parse_expr();
            }
            marker.complete(self, NodeKind::CaseCondition)
        } else {
            marker.abandon(self);
            scrutinee
        };
        self.expect(TokenKind::RParen);
        condition
    }

    fn parse_collection_for_header(&mut self, is_await: bool) {
        self.expect(TokenKind::LParen);
        if matches!(
            self.cur().kind,
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace
        ) {
            let header = self.start();
            let init = self.start();
            self.parse_binding_pattern();
            self.expect(TokenKind::In);
            self.parse_expr();
            init.complete(self, NodeKind::PatternForInInit);
            self.expect(TokenKind::RParen);
            header.complete(self, NodeKind::ForHeader);
            return;
        }
        let is_for_in = self.parse_for_clauses();
        self.check_await_for_header(is_await, is_for_in);
    }

    pub(super) fn check_await_for_header(&mut self, is_await: bool, is_for_in: bool) {
        if is_await && !is_for_in {
            self.error(
                "'await' cannot be used with a C-style for loop; use 'await for (x in ...)'",
            );
        }
    }

    /// Parses a dot shorthand head: `.name` or `.new`. The caller has already
    /// recorded `start`; the leading `.` is still current. Any trailing
    /// invocation or selector is left for `parse_postfix`.
    pub(super) fn parse_dot_shorthand(&mut self, start: usize) -> CompletedMarker {
        let marker = self.start_at(start);
        self.parse_dot_shorthand_inner(marker)
    }

    fn parse_dot_shorthand_inner(&mut self, marker: Marker) -> CompletedMarker {
        self.advance();
        self.expect_ctor_name();
        marker.complete(self, NodeKind::DotShorthandExpr)
    }

    /// Parse a symbol literal — the leading `#` is current. The body is either a
    /// dotted identifier chain (`#foo`, `#foo.bar.baz`) or a user-definable
    /// operator (`#+`, `#==`, `#[]`, `#[]=`, `#~`).
    pub(super) fn parse_symbol_literal(&mut self, start: usize) -> CompletedMarker {
        use TokenKind::*;
        let marker = self.start_at(start);
        self.advance();
        if self.cur().kind.is_identifier_like() || self.at(Void) {
            self.advance();
            while self.at(Dot) {
                self.advance();
                if self.cur().kind.is_identifier_like() || self.at(Void) {
                    self.advance();
                } else {
                    self.error("expected identifier after '.' in symbol literal");
                    break;
                }
            }
        } else if self.at(LBracket) {
            // Index operator `[]` or index-assign `[]=`.
            self.advance();
            self.expect(RBracket);
            self.eat(Eq);
        } else if self.cur().kind.is_user_definable_operator() {
            self.advance();
        } else {
            self.error("expected symbol name after '#'");
        }
        marker.complete(self, NodeKind::SymbolLitExpr)
    }

    fn parse_const_expr(&mut self, marker: Marker) -> CompletedMarker {
        // const can prefix a constructor call, collection literal, or (Dart 3.9)
        // a dot shorthand: `const .fromLtrb(...)`.
        match self.cur().kind {
            TokenKind::Dot => self.parse_dot_shorthand_inner(marker),
            // `const ('', X)` — a const record literal. Without this the `_` arm
            // would demand a type name after `const` and choke on the `(`.
            TokenKind::LParen => self.parse_paren_or_record_inner(marker),
            TokenKind::LBracket => self.parse_list_literal(marker),
            TokenKind::LBrace => self.parse_map_or_set_literal(0, marker),
            TokenKind::Lt => self.parse_typed_collection(marker),
            _ => {
                self.parse_type();
                // A named constructor may be `.new` (`const C.new()`); `new` is a
                // keyword token, so accept it explicitly here.
                if self.eat(TokenKind::Dot).is_some() {
                    self.expect_ctor_name();
                }
                self.parse_arg_list();
                marker.complete(self, NodeKind::NewExpr)
            }
        }
    }
}
