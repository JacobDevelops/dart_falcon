use falcon_syntax::cst::NodeKind;
use falcon_syntax::token::TokenKind;

use super::{CompletedMarker, Parser, expr::Brackets};

impl<'src> Parser<'src> {
    /// Parses Dart 3 logical-or patterns.
    pub(super) fn parse_pattern(&mut self) -> CompletedMarker {
        self.parse_logical_or_pattern().0
    }

    // Declaration and for-in patterns bind bare names instead of referencing constants.
    pub(super) fn parse_binding_pattern(&mut self) -> CompletedMarker {
        self.with_ctx(|ctx| ctx.pattern_binding = true, Self::parse_pattern)
    }

    fn parse_logical_or_pattern(&mut self) -> (CompletedMarker, bool) {
        self.guarded(
            |p| (p.missing(NodeKind::ErrorPattern), false),
            |p| {
                let left = p.parse_logical_and_pattern();
                p.parse_logical_pattern_tail(
                    left,
                    TokenKind::PipePipe,
                    NodeKind::LogicalOrPattern,
                    Self::parse_logical_and_pattern,
                )
            },
        )
    }

    fn parse_logical_and_pattern(&mut self) -> (CompletedMarker, bool) {
        let left = self.parse_relational_pattern();
        self.parse_logical_pattern_tail(
            left,
            TokenKind::AmpAmp,
            NodeKind::LogicalAndPattern,
            Self::parse_relational_pattern,
        )
    }

    fn parse_logical_pattern_tail(
        &mut self,
        mut left: (CompletedMarker, bool),
        operator: TokenKind,
        kind: NodeKind,
        operand: fn(&mut Self) -> (CompletedMarker, bool),
    ) -> (CompletedMarker, bool) {
        while self.at(operator) {
            let marker = left.0.precede(self);
            self.advance();
            operand(self);
            left = (marker.complete(self, kind), false);
        }
        left
    }

    fn parse_relational_pattern(&mut self) -> (CompletedMarker, bool) {
        // A `<` opening a typed collection is not a relational operator.
        if self.at(TokenKind::Lt) && self.typed_collection_ahead() {
            return self.parse_postfix_pattern();
        }
        if self.at_any(&[
            TokenKind::EqEq,
            TokenKind::BangEq,
            TokenKind::Lt,
            TokenKind::Gt,
            TokenKind::LtEq,
            TokenKind::GtEq,
        ]) {
            let marker = self.start();
            self.advance();
            // Keep logical operators at the pattern level.
            self.parse_bitwise_or();
            return (marker.complete(self, NodeKind::RelationalPattern), false);
        }
        self.parse_postfix_pattern()
    }

    // Balanced type arguments followed by a collection distinguish `<` from comparison.
    fn typed_collection_ahead(&self) -> bool {
        debug_assert_eq!(self.cur().kind, TokenKind::Lt);
        self.matching_angle_close(self.pos, |_| true, Some(513))
            .is_some_and(|close| {
                matches!(
                    self.kind_at(close + 1),
                    TokenKind::LBracket | TokenKind::LBrace
                )
            })
    }

    fn parse_postfix_pattern(&mut self) -> (CompletedMarker, bool) {
        let inner = self.parse_primary_pattern();
        self.parse_postfix_pattern_tail(inner)
    }

    fn parse_postfix_pattern_tail(
        &mut self,
        mut inner: (CompletedMarker, bool),
    ) -> (CompletedMarker, bool) {
        while self.at_any(&[TokenKind::Qmark, TokenKind::Bang, TokenKind::As]) {
            let marker = inner.0.precede(self);
            let kind = match self.advance().kind {
                TokenKind::Qmark => NodeKind::NullCheckPattern,
                TokenKind::Bang => NodeKind::NullAssertPattern,
                _ => {
                    self.parse_type();
                    NodeKind::CastPattern
                }
            };
            inner.0 = marker.complete(self, kind);
        }
        inner
    }

    fn parse_primary_pattern(&mut self) -> (CompletedMarker, bool) {
        let marker = self.start();
        let value_marker = self.start();
        let (kind, binds_name, expression) = self.parse_primary_pattern_inner();
        if expression {
            value_marker.complete(self, NodeKind::ConstPatternExpr);
        } else {
            value_marker.abandon(self);
        }
        (marker.complete(self, kind), binds_name)
    }

    fn parse_primary_pattern_inner(&mut self) -> (NodeKind, bool, bool) {
        let start = self.cur().offset();
        if self.at(TokenKind::Dot) {
            self.parse_dot_shorthand(start);
            return (NodeKind::ConstPattern, false, true);
        }

        if self.at(TokenKind::LParen) && self.record_type_var_pattern_ahead() {
            self.parse_type();
            if self.eat_wildcard() {
                return (NodeKind::WildcardPattern, false, false);
            }
            self.expect_ident();
            return (NodeKind::VariablePattern, true, false);
        }
        if self.at(TokenKind::LParen) {
            return (self.parse_paren_or_record_pattern(), false, false);
        }
        if self.at(TokenKind::LBracket) {
            return (self.parse_list_pattern(), false, false);
        }
        if self.at(TokenKind::LBrace) {
            return (self.parse_map_pattern(), false, false);
        }
        if self.at(TokenKind::Lt) {
            let type_args = self.parse_type_args();
            if self.at(TokenKind::LBracket) {
                if type_args != 1 {
                    self.error(format!(
                        "list pattern expects exactly one type argument, got {}",
                        type_args
                    ));
                }
                return (self.parse_list_pattern(), false, false);
            }
            if self.at(TokenKind::LBrace) {
                if type_args != 2 {
                    self.error(format!(
                        "map pattern expects exactly two type arguments, got {}",
                        type_args
                    ));
                }
                return (self.parse_map_pattern(), false, false);
            }
            self.error(format!(
                "expected '[' or '{{' after type arguments in collection pattern, got {:?}",
                self.cur().kind
            ));
            return (NodeKind::ErrorPattern, false, false);
        }
        if self.eat(TokenKind::Null).is_some()
            || self.eat(TokenKind::True).is_some()
            || self.eat(TokenKind::False).is_some()
        {
            return (NodeKind::LiteralPattern, false, false);
        }
        if self.at(TokenKind::Minus)
            && matches!(self.peek(1).kind, TokenKind::IntLit | TokenKind::DoubleLit)
        {
            self.advance();
            self.advance();
            return (NodeKind::LiteralPattern, false, false);
        }
        if self.at_any(&[TokenKind::IntLit, TokenKind::DoubleLit]) {
            self.advance();
            return (NodeKind::LiteralPattern, false, false);
        }
        // A member access on a string is a constant expression.
        if self.at(TokenKind::StringLit) {
            let string = self.parse_adjacent_strings();
            if !(self.at(TokenKind::Dot) && self.peek(1).kind.is_identifier_like()) {
                return (NodeKind::LiteralPattern, false, false);
            }
            self.parse_string_pattern_fields(string);
            return (NodeKind::ConstPattern, false, true);
        }
        if self.at(TokenKind::Hash) {
            self.parse_symbol_literal(start);
            return (NodeKind::ConstPattern, false, true);
        }
        if self.at(TokenKind::Const) {
            let expression = self.parse_const_pattern();
            return (NodeKind::ConstPattern, false, expression);
        }
        if self.eat_wildcard() {
            return (NodeKind::WildcardPattern, false, false);
        }
        if self.at(TokenKind::Var) {
            self.advance();
            if self.eat_wildcard() {
                return (NodeKind::WildcardPattern, false, false);
            }
            self.expect_ident();
            return (NodeKind::VariablePattern, true, false);
        }
        if self.at(TokenKind::Final) {
            self.advance();
            self.try_parse_type_followed_by(|kind| {
                kind.is_identifier_like() && !matches!(kind, TokenKind::When | TokenKind::As)
            });
            if self.eat_wildcard() {
                return (NodeKind::WildcardPattern, false, false);
            }
            self.expect_ident();
            return (NodeKind::VariablePattern, true, false);
        }
        if self.is_type_start() {
            let (kind, binds_name) = self.parse_type_led_pattern();
            return (kind, binds_name, false);
        }
        self.error(format!("expected pattern, got {:?}", self.cur().kind));
        (NodeKind::ErrorPattern, false, false)
    }

    fn parse_string_pattern_fields(&mut self, mut target: CompletedMarker) {
        while self.at(TokenKind::Dot) && self.peek(1).kind.is_identifier_like() {
            let marker = target.precede(self);
            self.advance();
            self.expect_ident();
            target = marker.complete(self, NodeKind::FieldExpr);
        }
    }

    // A binding after a balanced record type distinguishes it from a record pattern.
    fn record_type_var_pattern_ahead(&self) -> bool {
        debug_assert_eq!(self.cur().kind, TokenKind::LParen);
        let Some(close) = self.matching_close(self.pos, Brackets::Mixed, |_| false, None) else {
            return false;
        };
        let mut next = close + 1;
        if self.kind_at(next) == TokenKind::Qmark {
            next += 1;
        }
        let kind = self.kind_at(next);
        kind.is_identifier_like() && !matches!(kind, TokenKind::As | TokenKind::When)
    }

    fn parse_paren_or_record_pattern(&mut self) -> NodeKind {
        self.advance(); // (
        if self.at(TokenKind::RParen) {
            self.advance();
            return NodeKind::RecordPattern;
        }
        if self.at(TokenKind::Colon)
            || (self.cur().kind.is_identifier_like() && self.peek(1).kind == TokenKind::Colon)
        {
            return self.parse_record_pattern_body();
        }
        let first_marker = self.start();
        self.parse_pattern();
        if self.at(TokenKind::Comma) {
            first_marker.complete(self, NodeKind::RecordPatternField);
            while self.eat(TokenKind::Comma).is_some()
                && !self.at(TokenKind::RParen)
                && !self.at(TokenKind::Eof)
            {
                self.parse_record_field();
            }
            self.eat(TokenKind::Comma);
            self.expect(TokenKind::RParen);
            NodeKind::RecordPattern
        } else {
            first_marker.abandon(self);
            if !self.at(TokenKind::RParen) {
                self.pattern_recovery_span();
            }
            self.expect(TokenKind::RParen);
            NodeKind::ParenPattern
        }
    }

    fn parse_record_pattern_body(&mut self) -> NodeKind {
        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            self.parse_record_field();
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect(TokenKind::RParen);
        NodeKind::RecordPattern
    }

    fn parse_record_field(&mut self) {
        let marker = self.start();
        if self.at(TokenKind::Colon) {
            self.advance();
            self.parse_binding_pattern();
            marker.complete(self, NodeKind::RecordPatternField);
            return;
        }
        if self.cur().kind.is_identifier_like() && self.peek(1).kind == TokenKind::Colon {
            self.expect_ident();
            self.advance();
        }
        self.parse_pattern();
        marker.complete(self, NodeKind::RecordPatternField);
    }

    fn parse_list_pattern(&mut self) -> NodeKind {
        self.advance(); // [
        while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::DotDotDot) {
                let marker = self.start();
                self.advance();
                if !self.at(TokenKind::RBracket) && !self.at(TokenKind::Comma) {
                    self.parse_pattern();
                }
                marker.complete(self, NodeKind::RestPattern);
            } else {
                self.parse_pattern();
            }
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect(TokenKind::RBracket);
        NodeKind::ListPattern
    }

    fn parse_map_pattern(&mut self) -> NodeKind {
        self.advance(); // {
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::DotDotDot) {
                self.advance();
                if self.eat(TokenKind::Comma).is_none() {
                    break;
                }
                continue;
            }
            let marker = self.start();
            self.parse_expr();
            self.expect(TokenKind::Colon);
            self.parse_pattern();
            marker.complete(self, NodeKind::MapPatternEntry);
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect(TokenKind::RBrace);
        NodeKind::MapPattern
    }

    fn parse_const_pattern(&mut self) -> bool {
        if self.peek(1).kind == TokenKind::LParen {
            let marker = self.start();
            self.advance(); // const
            self.parse_paren_or_record_inner(marker);
            return true;
        }
        let is_expr_form = match self.peek(1).kind {
            TokenKind::LBracket | TokenKind::LBrace | TokenKind::Lt => true,
            _ if self.peek(1).kind.is_identifier_like() => {
                let mut i = 2;
                while self.peek(i).kind == TokenKind::Dot
                    && self.peek(i + 1).kind.is_identifier_like()
                {
                    i += 2;
                }
                matches!(self.peek(i).kind, TokenKind::LParen | TokenKind::Lt)
            }
            _ => false,
        };
        if is_expr_form {
            self.parse_primary();
            return true;
        }
        self.advance(); // const
        if self.cur().kind.is_identifier_like() {
            self.expect_ident();
            while self.at(TokenKind::Dot) && self.peek(1).kind.is_identifier_like() {
                self.advance();
                self.expect_ident();
            }
        }
        false
    }

    fn parse_type_led_pattern(&mut self) -> (NodeKind, bool) {
        let start = self.checkpoint();
        let (ty, bare_name) = self.parse_type_with_binding_name();
        if self.at(TokenKind::LParen) {
            self.advance();
            while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
                let marker = self.start();
                if self.at(TokenKind::Colon) {
                    self.advance();
                    let (_, binds_name) = self.with_ctx(
                        |ctx| ctx.pattern_binding = true,
                        Self::parse_logical_or_pattern,
                    );
                    if !binds_name {
                        self.pattern_recovery_span();
                    }
                    marker.complete(self, NodeKind::ObjectPatternField);
                    if self.eat(TokenKind::Comma).is_none() {
                        break;
                    }
                    continue;
                }
                let has_name =
                    self.cur().kind.is_identifier_like() && self.peek(1).kind == TokenKind::Colon;
                if has_name {
                    self.expect_ident();
                    self.advance();
                }
                if !self.at(TokenKind::RParen) && !self.at(TokenKind::Comma) {
                    self.parse_pattern();
                }
                if !has_name {
                    self.pattern_recovery_span();
                }
                marker.complete(self, NodeKind::ObjectPatternField);
                if self.eat(TokenKind::Comma).is_none() {
                    break;
                }
            }
            self.eat(TokenKind::Comma);
            self.expect(TokenKind::RParen);
            return (NodeKind::ObjectPattern, false);
        }
        if self.eat_wildcard() {
            return (NodeKind::WildcardPattern, false);
        }
        if self.cur().kind.is_identifier_like() && !self.at_any(&[TokenKind::When, TokenKind::As]) {
            self.expect_ident();
            return (NodeKind::VariablePattern, true);
        }
        match ty.kind {
            NodeKind::NamedType if self.ctx.pattern_binding && bare_name => {
                // The bare name binds a variable; its speculative type wrapper
                // is not part of the written pattern's type.
                self.restore(start);
                self.expect_ident();
                (NodeKind::VariablePattern, true)
            }
            NodeKind::NamedType | NodeKind::DynamicType => (NodeKind::ConstPattern, false),
            _ => (NodeKind::ErrorPattern, false),
        }
    }

    fn pattern_recovery_span(&mut self) {
        self.events
            .push(falcon_syntax::cst::Event::RecoverySpan(self.cur_span()));
    }

    fn eat_wildcard(&mut self) -> bool {
        if self.at(TokenKind::Ident) && self.cur_text() == "_" {
            self.advance();
            true
        } else {
            false
        }
    }
}
