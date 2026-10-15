use falcon_syntax::Feature;
use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;

use super::{Parser, expr::Brackets};
use crate::lexer::Lexer;

impl<'src> Parser<'src> {
    pub(super) fn parse_block(&mut self) -> Block {
        let start = self.cur().offset;
        self.expect(TokenKind::LBrace);
        let mut stmts = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            stmts.push(self.parse_stmt());
        }
        self.expect(TokenKind::RBrace);
        Block {
            stmts,
            span: self.span_from(start),
        }
    }

    pub(super) fn parse_stmt(&mut self) -> Stmt {
        self.guarded(
            |p| {
                let span = p.cur_span();
                p.advance();
                Stmt::Expr(ExprStmt {
                    expr: Expr::Error { span },
                    span,
                })
            },
            |p| {
                let start = p.cur().offset;

                // A statement-leading `label:` is unambiguous; labels may nest.
                if p.at(TokenKind::Ident) && p.peek(1).kind == TokenKind::Colon {
                    let tok = p.advance();
                    let label = p.tok_ident(&tok);
                    p.advance(); // :
                    let stmt = p.parse_stmt();
                    return Stmt::Labeled(LabeledStmt {
                        label,
                        stmt: Box::new(stmt),
                        span: p.span_from(start),
                    });
                }

                // Reject sync-body `await for`, recovering as an ordinary for-loop.
                if p.at(TokenKind::Await) && p.peek(1).kind == TokenKind::For {
                    if !p.ctx.body.is_async() {
                        p.error("'await for' is only allowed in an async function body");
                    }
                    p.advance(); // await
                    return p.parse_for_stmt(p.ctx.body.is_async(), start);
                }

                match p.cur().kind {
                    TokenKind::LBrace => {
                        // Gate speculation on `=` so ordinary blocks retain diagnostics.
                        if p.map_pattern_assign_ahead()
                            && let Some(stmt) = p.try_parse_pattern_assign(start)
                        {
                            return stmt;
                        }
                        // A literal-led brace group is a collection unless it contains
                        // a top-level semicolon, which only a block may contain.
                        let next = p.peek(1).kind;
                        let looks_like_collection = matches!(
                            next,
                            TokenKind::StringLit
                                | TokenKind::IntLit
                                | TokenKind::DoubleLit
                                | TokenKind::True
                                | TokenKind::False
                                | TokenKind::Null
                        ) && !p.brace_group_has_top_level_semicolon();
                        if looks_like_collection {
                            p.parse_expr_or_local_decl_stmt()
                        } else {
                            Stmt::Block(p.parse_block())
                        }
                    }
                    TokenKind::If => p.parse_if_stmt(),
                    TokenKind::For => p.parse_for_stmt(false, start),
                    TokenKind::While => p.parse_while_stmt(),
                    TokenKind::Do => p.parse_do_while_stmt(),
                    TokenKind::Switch => p.parse_switch_stmt(),
                    TokenKind::Try => p.parse_try_stmt(),
                    TokenKind::Return => p.parse_return_stmt(),
                    TokenKind::Throw => {
                        p.advance();
                        let value = p.parse_expr();
                        p.eat(TokenKind::Semicolon);
                        Stmt::Throw(ThrowStmt {
                            value,
                            span: p.span_from(start),
                        })
                    }
                    TokenKind::Break => {
                        p.advance();
                        let label = p.parse_ident();
                        p.eat(TokenKind::Semicolon);
                        Stmt::Break(BreakStmt {
                            label,
                            span: p.span_from(start),
                        })
                    }
                    TokenKind::Continue => {
                        p.advance();
                        let label = p.parse_ident();
                        p.eat(TokenKind::Semicolon);
                        Stmt::Continue(ContinueStmt {
                            label,
                            span: p.span_from(start),
                        })
                    }
                    TokenKind::Assert => p.parse_assert_stmt(),
                    // Outside generators, `yield` remains an ordinary identifier.
                    TokenKind::Yield if p.ctx.body.is_generator() => p.parse_yield_stmt(),
                    TokenKind::Rethrow => {
                        let token = p.advance();
                        let mut span = Self::tok_span(&token);
                        if let Some(semicolon) = p.eat(TokenKind::Semicolon) {
                            span.end = semicolon.offset + semicolon.len;
                        }
                        Stmt::Rethrow(span)
                    }
                    TokenKind::Semicolon => {
                        let token = p.advance();
                        Stmt::Empty(Self::tok_span(&token))
                    }
                    _ => p.parse_expr_or_local_decl_stmt(),
                }
            },
        )
    }

    fn parse_if_stmt(&mut self) -> Stmt {
        let start = self.cur().offset;
        self.advance(); // if
        let condition = self.parse_if_condition();
        let then_branch = self.parse_stmt();
        let else_branch = if self.eat(TokenKind::Else).is_some() {
            Some(Box::new(self.parse_stmt()))
        } else {
            None
        };
        Stmt::If(IfStmt {
            condition,
            then_branch: Box::new(then_branch),
            else_branch,
            span: self.span_from(start),
        })
    }

    fn parse_for_stmt(&mut self, is_await: bool, start: usize) -> Stmt {
        self.advance(); // for
        self.expect(TokenKind::LParen);
        let (init, condition, update) = self.parse_for_clauses();
        self.check_await_for_header(is_await, &init);
        let body = self.parse_stmt();
        Stmt::For(ForStmt {
            is_await,
            header: ForHeader {
                init,
                condition,
                updates: update,
            },
            body: Box::new(body),
            span: self.span_from(start),
        })
    }

    /// Lookahead (positioned at `final`/`var`): does an ident-led object pattern
    /// follow — a type name whose subpattern list opens with `(` before the loop
    /// clause ends? Distinguishes `final MapEntry(:k, :v) in …` (object pattern)
    /// from a plain typed decl `final Foo x in …` (no `(` before `in`).
    fn ident_led_object_pattern_ahead(&self) -> bool {
        if !self.peek(1).kind.is_identifier_like() {
            return false;
        }
        let mut i = 2;
        loop {
            match self.peek(i).kind {
                TokenKind::LParen => return true,
                TokenKind::In
                | TokenKind::Eq
                | TokenKind::Semicolon
                | TokenKind::RParen
                | TokenKind::Eof => return false,
                _ => i += 1,
            }
        }
    }

    pub(super) fn parse_for_clauses(&mut self) -> (Option<ForInit>, Option<Expr>, Vec<Expr>) {
        let annotations = self.parse_annotations();
        let (mut init, condition, updates) = self.parse_unannotated_for_clauses();
        match &mut init {
            Some(ForInit::VarDecl(decl)) => decl.annotations = annotations,
            Some(
                ForInit::ForIn {
                    annotations: metadata,
                    ..
                }
                | ForInit::PatternForIn {
                    annotations: metadata,
                    ..
                },
            ) => *metadata = annotations,
            _ => {}
        }
        (init, condition, updates)
    }

    fn parse_unannotated_for_clauses(&mut self) -> (Option<ForInit>, Option<Expr>, Vec<Expr>) {
        // Empty for (;;)
        if self.at(TokenKind::Semicolon) {
            return self.finish_for_clauses(None);
        }

        // Dart 3 pattern for-in: `for (final (a, b) in xs)`. The keyword must be
        // directly followed by a `(`/`[`/`{` opening a destructuring pattern, or
        // by an ident-led object pattern (`final MapEntry(:key, :value) in ...`),
        // with the pattern then followed by `in`. A plain typed `final Foo x in`
        // has no `(` before the terminator, so it stays a typed ForIn (below).
        if self.at_any(&[TokenKind::Final, TokenKind::Var])
            && (matches!(
                self.peek(1).kind,
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace
            ) || self.ident_led_object_pattern_ahead())
            && let Some(header) = self.speculate(|p| {
                let keyword = Self::var_keyword(false, p.at(TokenKind::Final));
                p.advance();
                let pattern = p.parse_binding_pattern();
                p.eat(TokenKind::In)?;
                let iterable = p.parse_expr();
                p.expect(TokenKind::RParen);
                Some((
                    Some(ForInit::PatternForIn {
                        annotations: Vec::new(),
                        keyword,
                        pattern: Box::new(pattern),
                        iterable: Box::new(iterable),
                    }),
                    None,
                    Vec::new(),
                ))
            })
        {
            return header;
        }

        let saved = self.pos;

        // var / final / const → definitely a decl
        if self.at_any(&[TokenKind::Var, TokenKind::Final, TokenKind::Const]) {
            let keyword_span = Some(self.cur_span());
            let is_const = self.at(TokenKind::Const);
            let is_final = self.at(TokenKind::Final);
            self.advance();
            let var_type = self.try_parse_type_followed_by(TokenKind::is_identifier_like);
            let name = self.expect_ident();
            if self.eat(TokenKind::In).is_some() {
                let iterable = self.parse_expr();
                self.expect(TokenKind::RParen);
                return (
                    Some(ForInit::ForIn {
                        annotations: Vec::new(),
                        declares_variable: true,
                        keyword: Self::var_keyword(is_const, is_final),
                        var_type,
                        name,
                        iterable: Box::new(iterable),
                    }),
                    None,
                    Vec::new(),
                );
            }
            return self.finish_c_style_for(
                false,
                is_final,
                is_const,
                var_type,
                keyword_span,
                name,
            );
        }

        // late final Type name in / late final Type name = ...
        if self.at(TokenKind::Late) {
            let is_late = true;
            self.advance();
            let keyword_span = self
                .at_any(&[TokenKind::Var, TokenKind::Final, TokenKind::Const])
                .then(|| self.cur_span());
            let is_const = self.eat(TokenKind::Const).is_some();
            let is_final = !is_const && self.eat(TokenKind::Final).is_some();
            let _ = self.eat(TokenKind::Var);
            let var_type = self.try_parse_type_followed_by(TokenKind::is_identifier_like);
            let name = self.expect_ident();
            return self.finish_c_style_for(
                is_late,
                is_final,
                is_const,
                var_type,
                keyword_span,
                name,
            );
        }

        // Try `Type name in` — typed for-in without var/final
        if self.is_type_start() {
            let ty = self.parse_type();
            if self.cur().kind.is_identifier_like() {
                let name = self.expect_ident();
                if self.eat(TokenKind::In).is_some() {
                    let iterable = self.parse_expr();
                    self.expect(TokenKind::RParen);
                    return (
                        Some(ForInit::ForIn {
                            annotations: Vec::new(),
                            declares_variable: true,
                            keyword: VarKeyword::Var,
                            var_type: Some(ty),
                            name,
                            iterable: Box::new(iterable),
                        }),
                        None,
                        Vec::new(),
                    );
                }
                // C-style with type+name
                return self.finish_c_style_for(false, false, false, Some(ty), None, name);
            }
            // Rollback — couldn't parse as decl
            self.rewind_to(saved);
        }

        // Expression(s) for-loop init, or bare `ident in expr`
        let expr = self.parse_expr();
        if self.eat(TokenKind::In).is_some() {
            let name = match expr {
                Expr::Ident(id) => id,
                _ => {
                    self.error("expected identifier in for-in");
                    Identifier::new("<error>", self.cur_span())
                }
            };
            let iterable = self.parse_expr();
            self.expect(TokenKind::RParen);
            return (
                Some(ForInit::ForIn {
                    annotations: Vec::new(),
                    declares_variable: false,
                    keyword: VarKeyword::Var,
                    var_type: None,
                    name,
                    iterable: Box::new(iterable),
                }),
                None,
                Vec::new(),
            );
        }
        // C-style: expr ; cond ; update
        let mut exprs = vec![expr];
        while self.eat(TokenKind::Comma).is_some() {
            exprs.push(self.parse_expr());
        }
        self.finish_for_clauses(Some(ForInit::Exprs(exprs)))
    }

    fn finish_c_style_for(
        &mut self,
        is_late: bool,
        is_final: bool,
        is_const: bool,
        var_type: Option<DartType>,
        keyword_span: Option<Span>,
        first_name: Identifier,
    ) -> (Option<ForInit>, Option<Expr>, Vec<Expr>) {
        let decl_start = first_name.span.start;
        let declarators = self.parse_declarators(first_name);
        let span = self.span_from(decl_start);
        self.finish_for_clauses(Some(ForInit::VarDecl(LocalVarDecl {
            annotations: Vec::new(),
            keyword: Self::var_keyword(is_const, is_final),
            keyword_span,
            is_late,
            var_type,
            declarators,
            span,
        })))
    }

    /// Finish a C-style header after its initializer, starting at the first `;`.
    fn finish_for_clauses(
        &mut self,
        init: Option<ForInit>,
    ) -> (Option<ForInit>, Option<Expr>, Vec<Expr>) {
        self.expect(TokenKind::Semicolon);
        let cond = self.parse_optional_expr_before(TokenKind::Semicolon);
        self.expect(TokenKind::Semicolon);
        let update = self.parse_expr_list_until(TokenKind::RParen);
        self.expect(TokenKind::RParen);
        (init, cond, update)
    }

    fn parse_optional_expr_before(&mut self, stop: TokenKind) -> Option<Expr> {
        if self.at(stop) {
            None
        } else {
            Some(self.parse_expr())
        }
    }

    fn parse_expr_list_until(&mut self, stop: TokenKind) -> Vec<Expr> {
        let mut exprs = Vec::new();
        while self.cur().kind != stop && !self.at(TokenKind::Eof) {
            exprs.push(self.parse_expr());
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        exprs
    }

    fn parse_while_stmt(&mut self) -> Stmt {
        let start = self.cur().offset;
        self.advance(); // while
        self.expect(TokenKind::LParen);
        let condition = self.parse_expr();
        self.expect(TokenKind::RParen);
        let body = self.parse_stmt();
        Stmt::While(WhileStmt {
            condition,
            body: Box::new(body),
            span: self.span_from(start),
        })
    }

    fn parse_do_while_stmt(&mut self) -> Stmt {
        let start = self.cur().offset;
        self.advance(); // do
        let body = self.parse_stmt();
        self.expect(TokenKind::While);
        self.expect(TokenKind::LParen);
        let condition = self.parse_expr();
        self.expect(TokenKind::RParen);
        self.eat(TokenKind::Semicolon);
        Stmt::DoWhile(DoWhileStmt {
            body: Box::new(body),
            condition,
            span: self.span_from(start),
        })
    }

    fn parse_switch_stmt(&mut self) -> Stmt {
        let start = self.cur().offset;
        self.advance(); // switch
        self.expect(TokenKind::LParen);
        let subject = self.parse_expr();
        self.expect(TokenKind::RParen);
        self.expect(TokenKind::LBrace);
        let mut cases = Vec::new();

        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let case_start = self.cur().offset;
            let mut case_kinds = Vec::new();
            let mut labels = Vec::new();

            loop {
                // A case may carry statement labels used as `continue label;`
                // targets: `label: case 2:` / `label: default:`. Retain them.
                while self.at(TokenKind::Ident) && self.peek(1).kind == TokenKind::Colon {
                    labels.push(self.expect_ident()); // label
                    self.advance(); // :
                }
                if self.at(TokenKind::Case) {
                    self.advance();
                    // Before patterns (Dart < 3.0) a case operand is a constant
                    // expression, not a pattern: `case 1 + 2:`, `case -x:`.
                    if !self.version.supports(Feature::Patterns) {
                        let expr = self.parse_expr();
                        self.expect(TokenKind::Colon);
                        case_kinds.push(SwitchCaseKind::Expr(Box::new(expr)));
                    } else {
                        let pattern = self.parse_pattern();
                        let guard = if self.eat(TokenKind::When).is_some() {
                            Some(Box::new(self.parse_expr()))
                        } else {
                            None
                        };
                        self.expect(TokenKind::Colon);
                        case_kinds.push(SwitchCaseKind::Pattern(Box::new(pattern), guard));
                    }
                } else if self.at(TokenKind::Default) {
                    self.advance();
                    self.expect(TokenKind::Colon);
                    case_kinds.push(SwitchCaseKind::Default);
                } else {
                    break;
                }
                if !self.at(TokenKind::Case)
                    && !self.at(TokenKind::Default)
                    && !self.at_labeled_case()
                {
                    break;
                }
            }

            if case_kinds.is_empty() {
                self.error(format!(
                    "expected case or default, got {:?}",
                    self.cur().kind
                ));
                self.advance();
                continue;
            }

            let mut body = Vec::new();
            while !self.at(TokenKind::Case)
                && !self.at(TokenKind::Default)
                && !self.at(TokenKind::RBrace)
                && !self.at(TokenKind::Eof)
                && !self.at_labeled_case()
            {
                body.push(self.parse_stmt());
            }
            cases.push(SwitchCase {
                labels,
                cases: case_kinds,
                body,
                span: self.span_from(case_start),
            });
        }

        self.expect(TokenKind::RBrace);
        Stmt::Switch(SwitchStmt {
            subject,
            cases,
            span: self.span_from(start),
        })
    }

    /// True when the cursor is at one or more `label:` prefixes that lead into a
    /// `case`/`default` — a labeled switch case rather than a labeled statement in
    /// the current case's body.
    fn at_labeled_case(&self) -> bool {
        let mut i = self.pos;
        while self.kind_at(i) == TokenKind::Ident && self.kind_at(i + 1) == TokenKind::Colon {
            i += 2;
        }
        i > self.pos && matches!(self.kind_at(i), TokenKind::Case | TokenKind::Default)
    }

    fn parse_try_stmt(&mut self) -> Stmt {
        let start = self.cur().offset;
        self.advance(); // try
        let body = self.parse_block();
        let mut catches = Vec::new();

        while self.at(TokenKind::On) || self.at(TokenKind::Catch) {
            let catch_start = self.cur().offset;
            let exception_type = if self.at(TokenKind::On) {
                self.advance(); // on
                Some(self.parse_type())
            } else {
                None
            };
            let (exception_var, stack_trace_var) = if self.eat(TokenKind::Catch).is_some() {
                self.expect(TokenKind::LParen);
                let e = self.expect_ident();
                let s = if self.eat(TokenKind::Comma).is_some() {
                    Some(self.expect_ident())
                } else {
                    None
                };
                self.expect(TokenKind::RParen);
                (Some(e), s)
            } else {
                (None, None)
            };
            let catch_body = self.parse_block();
            catches.push(CatchClause {
                exception_type,
                exception_var,
                stack_trace_var,
                body: catch_body,
                span: self.span_from(catch_start),
            });
        }

        let finally = if self.eat(TokenKind::Finally).is_some() {
            Some(self.parse_block())
        } else {
            None
        };

        Stmt::TryCatch(TryCatchStmt {
            body,
            catches,
            finally,
            span: self.span_from(start),
        })
    }

    fn parse_return_stmt(&mut self) -> Stmt {
        let start = self.cur().offset;
        self.advance(); // return
        let value = if !self.at(TokenKind::Semicolon)
            && !self.at(TokenKind::RBrace)
            && !self.at(TokenKind::Eof)
        {
            Some(self.parse_expr())
        } else {
            None
        };
        self.eat(TokenKind::Semicolon);
        Stmt::Return(ReturnStmt {
            value,
            span: self.span_from(start),
        })
    }

    fn parse_assert_stmt(&mut self) -> Stmt {
        let start = self.cur().offset;
        self.advance(); // assert
        self.expect(TokenKind::LParen);
        let condition = self.parse_expr();
        let message = if self.eat(TokenKind::Comma).is_some() && !self.at(TokenKind::RParen) {
            Some(self.parse_expr())
        } else {
            None
        };
        self.eat(TokenKind::Comma); // optional trailing comma
        self.expect(TokenKind::RParen);
        self.eat(TokenKind::Semicolon);
        Stmt::Assert(AssertStmt {
            condition,
            message,
            span: self.span_from(start),
        })
    }

    fn parse_yield_stmt(&mut self) -> Stmt {
        let start = self.cur().offset;
        self.advance(); // yield
        let is_star = self.eat(TokenKind::Star).is_some();
        let value = self.parse_expr();
        self.eat(TokenKind::Semicolon);
        Stmt::Yield(YieldStmt {
            is_star,
            value,
            span: self.span_from(start),
        })
    }

    fn parse_expr_or_local_decl_stmt(&mut self) -> Stmt {
        let start_index = self.pos;
        let start = self.cur().offset;

        let mut annotations = Vec::new();
        while self.at(TokenKind::At) {
            annotations.extend(self.parse_annotations());
        }
        let unannotated = annotations.is_empty();
        let mut stmt = self.parse_local_decl_or_expr_stmt(start, unannotated);
        // Metadata belongs to the local variable or function it precedes.
        match &mut stmt {
            Stmt::LocalVar(decl) => {
                decl.annotations = annotations;
                if let Some(start) = self.local_documentation_start(start_index) {
                    decl.span.start = start;
                }
            }
            Stmt::LocalFunc(decl) => decl.annotations = annotations,
            Stmt::PatternDecl(decl) => decl.annotations = annotations,
            _ => {}
        }
        stmt
    }

    /// Recover attached documentation from the already skipped trivia before a
    /// local declaration. Only this bounded gap is re-lexed, without allocation.
    fn local_documentation_start(&self, declaration_index: usize) -> Option<usize> {
        let trivia_start = declaration_index.checked_sub(1).map_or(0, |index| {
            let token = &self.tokens[index];
            token.offset + token.len
        });
        let trivia_end = self.tokens[declaration_index].offset;
        let trivia = &self.src[trivia_start..trivia_end];
        let mut lexer = Lexer::new(trivia);
        let mut start = None;
        let mut previous_block = false;
        loop {
            let token = lexer.next_token();
            if token.kind == TokenKind::Eof {
                break;
            }
            if token.kind == TokenKind::DocComment {
                let is_block = token.text(trivia).starts_with("/**");
                // Each block doc is its own group; line docs form a group even
                // when ordinary comments occur between them.
                if is_block || previous_block || start.is_none() {
                    start = Some(trivia_start + token.offset);
                }
                previous_block = is_block;
            }
        }
        start
    }

    fn parse_local_decl_or_expr_stmt(&mut self, start: usize, unannotated: bool) -> Stmt {
        // Dart 3 pattern-variable declaration: `(var|final) <pattern> = expr;`.
        // Destructuring starts with a group or an object-pattern type; require
        // `=` after the pattern to distinguish record-typed variable declarations.
        if self.at_any(&[TokenKind::Final, TokenKind::Var])
            && (matches!(
                self.peek(1).kind,
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace
            ) || self.object_pattern_assign_ahead(self.pos + 1))
            && let Some(stmt) = self.try_parse_pattern_declaration(start)
        {
            return stmt;
        }

        // Dart 3 pattern-assignment: `(a, b) = expr;`, `[a, b] = expr;` — a
        // `(`/`[` destructuring pattern (no `var`/`final`) followed by `=`. The
        // trailing single `=` distinguishes it from a record/list expression
        // statement; without it we restore and fall through to expression parsing.
        // An object pattern destructures too: `AsyncError(:error) = result;`.
        if unannotated
            && (self.at_any(&[TokenKind::LParen, TokenKind::LBracket])
                || self.object_pattern_assign_ahead(self.pos))
            && let Some(stmt) = self.try_parse_pattern_assign(start)
        {
            return stmt;
        }

        // late / final / var → definitely a var decl
        if self.at(TokenKind::Late) || self.at_any(&[TokenKind::Final, TokenKind::Var]) {
            return self.parse_local_var_decl(start);
        }
        // `const` can be either a var decl or a const expression (constructor/collection).
        //   `const Type name` or `const name =` → var decl
        //   `const Type(` / `const Type<...>(` / `const Type.` / `const [` /
        //   `const {` / `const <T>[` → expr
        // A `const` that does not begin a const expression is a var decl; when it
        // does (constructor/collection), fall through to expression parsing.
        if self.at(TokenKind::Const) && !self.const_starts_expr() {
            return self.parse_local_var_decl(start);
        }

        // Try to parse as typed local decl or function.
        // A leading `(` may begin a record-typed var decl (`(int, String) x = ...`)
        // or a plain parenthesized/record expression. Speculatively parse a type and
        // only commit when it is followed by `name =`/`;`/`,` (or a function body);
        // otherwise restore both position and any errors the speculation emitted.
        // `await`/`yield` lead an expression statement in async bodies; although
        // both are contextually ident-like, they are never type names, so keep
        // them out of the speculative typed-declaration path.
        if self.is_type_start()
            && !self.at_any(&[TokenKind::Await, TokenKind::Yield])
            && let Some(stmt) = self.speculate(|p| {
                let ty = p.parse_type();
                let name = p.parse_ident()?;
                if p.at_any(&[TokenKind::LParen, TokenKind::Lt]) {
                    let type_params = p.parse_type_params();
                    let params = p.parse_formal_param_list();
                    let async_marker = p.parse_async_marker();
                    let body = p.parse_function_body(async_marker)?;
                    Some(Stmt::LocalFunc(LocalFuncDecl {
                        annotations: Vec::new(),
                        return_type: Some(ty),
                        name,
                        type_params,
                        params,
                        async_marker,
                        body,
                        span: p.span_from(start),
                    }))
                } else if p.at_any(&[TokenKind::Eq, TokenKind::Semicolon, TokenKind::Comma]) {
                    let declarators = p.parse_declarators(name);
                    // A colon exposes a ternary misread as a nullable type.
                    if !p.at_any(&[TokenKind::Semicolon, TokenKind::RBrace, TokenKind::Eof]) {
                        return None;
                    }
                    p.eat(TokenKind::Semicolon);
                    Some(Stmt::LocalVar(LocalVarDecl {
                        annotations: Vec::new(),
                        keyword: VarKeyword::Var,
                        keyword_span: None,
                        is_late: false,
                        var_type: Some(ty),
                        declarators,
                        span: p.span_from(start),
                    }))
                } else {
                    None
                }
            })
        {
            return stmt;
        }

        // Untyped local function: `name [<T>](params) [async] { .. }` / `=> e`
        // (no return type). A balanced lookahead confirms a function body
        // follows the parameter list before we commit — otherwise this is a
        // call statement (`foo();`, `g((x) => x);`), parsed as an expression.
        if self.at(TokenKind::Ident)
            && matches!(self.peek(1).kind, TokenKind::LParen | TokenKind::Lt)
            && self.untyped_local_func_ahead()
            && let Some(stmt) = self.try_parse_untyped_local_func(start)
        {
            return stmt;
        }

        let expr = self.parse_expr();
        self.eat(TokenKind::Semicolon);
        Stmt::Expr(ExprStmt {
            expr,
            span: self.span_from(start),
        })
    }

    /// True when a statement-leading `const` begins a const *expression*
    /// (constructor call or collection literal) rather than a const variable
    /// declaration. Skips the whole `Type[.name][<args>]` head so a generic
    /// constructor `const Optional<int>.absent()` is told apart from a const typed
    /// declaration `const List<int> xs = ...` by the `(`/`.` (constructor) vs
    /// identifier (variable name) that follows.
    fn const_starts_expr(&self) -> bool {
        let kind = |i: usize| self.tokens.get(i).map(|t| t.kind);
        let mut i = self.pos + 1; // token after `const`
        // Collection / record literal: const [..], const {..}, const <T>[..], const (..).
        if matches!(
            kind(i),
            Some(TokenKind::LBracket | TokenKind::LBrace | TokenKind::Lt | TokenKind::LParen)
        ) {
            return true;
        }
        // Must be a (possibly dotted, possibly generic) type name.
        if !self
            .tokens
            .get(i)
            .is_some_and(|t| t.kind.is_identifier_like())
        {
            return false;
        }
        i += 1;
        while matches!(kind(i), Some(TokenKind::Dot))
            && self
                .tokens
                .get(i + 1)
                .is_some_and(|t| t.kind.is_identifier_like())
        {
            i += 2;
        }
        if matches!(kind(i), Some(TokenKind::Lt)) {
            i = self.index_after_balanced_angles(i);
        }
        // `(` (unnamed/generic ctor) or `.` (named ctor) ⇒ expression;
        // an identifier here is the declared variable's name ⇒ declaration.
        matches!(kind(i), Some(TokenKind::LParen | TokenKind::Dot))
    }

    fn parse_local_var_decl(&mut self, start: usize) -> Stmt {
        let is_late = self.eat(TokenKind::Late).is_some();
        let keyword_span = self
            .at_any(&[TokenKind::Var, TokenKind::Final, TokenKind::Const])
            .then(|| self.cur_span());
        let is_const = self.eat(TokenKind::Const).is_some();
        let is_final = !is_const && self.eat(TokenKind::Final).is_some();
        let _ = self.eat(TokenKind::Var);

        let var_type = self.try_parse_type_followed_by(TokenKind::is_identifier_like);

        let name = self.expect_ident();
        let declarators = self.parse_declarators(name);
        self.eat(TokenKind::Semicolon);
        Stmt::LocalVar(LocalVarDecl {
            annotations: Vec::new(),
            keyword: Self::var_keyword(is_const, is_final),
            keyword_span,
            is_late,
            var_type,
            declarators,
            span: self.span_from(start),
        })
    }

    /// Speculatively parse a Dart 3 pattern-variable declaration
    /// `(var|final) <pattern> = expr ;`. Returns `None` (restoring position and
    /// errors) when what follows the keyword is not a destructuring pattern
    /// followed by `=` — e.g. a record-typed variable declaration.
    fn try_parse_pattern_declaration(&mut self, start: usize) -> Option<Stmt> {
        self.speculate(|p| {
            let errors = p.errors.len();
            let is_final = p.at(TokenKind::Final);
            let keyword_span = p.cur_span();
            p.advance();
            let pattern = p.parse_binding_pattern();
            if p.errors.len() > errors
                || matches!(pattern, Pattern::Variable { .. } | Pattern::Wildcard { .. })
            {
                return None;
            }
            p.eat(TokenKind::Eq)?;
            let init = p.parse_expr();
            p.eat(TokenKind::Semicolon);
            Some(Stmt::PatternDecl(PatternDeclaration {
                annotations: Vec::new(),
                keyword: Self::var_keyword(false, is_final),
                keyword_span,
                pattern,
                init,
                span: p.span_from(start),
            }))
        })
    }

    // A closing brace followed by `=` distinguishes pattern assignment from a block.
    fn map_pattern_assign_ahead(&self) -> bool {
        debug_assert_eq!(self.cur().kind, TokenKind::LBrace);
        self.matching_close(self.pos, Brackets::BraceOnly, |_| false, None)
            .is_some_and(|close| self.kind_at(close + 1) == TokenKind::Eq)
    }

    // A top-level semicolon distinguishes a block from a collection literal.
    fn brace_group_has_top_level_semicolon(&self) -> bool {
        debug_assert_eq!(self.cur().kind, TokenKind::LBrace);
        let mut depth = 0i32;
        let mut i = self.pos;
        loop {
            match self.kind_at(i) {
                TokenKind::LBrace | TokenKind::LParen | TokenKind::LBracket => depth += 1,
                TokenKind::RBrace | TokenKind::RParen | TokenKind::RBracket => {
                    depth -= 1;
                    if depth == 0 {
                        return false;
                    }
                }
                TokenKind::Semicolon if depth == 1 => return true,
                TokenKind::Eof => return false,
                _ => {}
            }
            i += 1;
        }
    }

    // Failed pattern trials must retain neither tokens nor diagnostics.
    fn try_parse_pattern_assign(&mut self, start: usize) -> Option<Stmt> {
        self.speculate(|p| {
            let errors = p.errors.len();
            let pattern = p.parse_binding_pattern();
            if p.errors.len() > errors
                || matches!(pattern, Pattern::Variable { .. } | Pattern::Wildcard { .. })
            {
                return None;
            }
            p.eat(TokenKind::Eq)?;
            let value = p.parse_expr();
            p.eat(TokenKind::Semicolon);
            Some(Stmt::PatternAssign(PatternAssignStmt {
                pattern,
                value,
                span: p.span_from(start),
            }))
        })
    }

    /// Balanced lookahead for `Type[.Type][<args>](...) =` (not `==`): an
    /// object-pattern assignment rather than a call statement.
    fn object_pattern_assign_ahead(&self, start: usize) -> bool {
        if !self.kind_at(start).is_identifier_like() {
            return false;
        }
        let mut i = start + 1;
        while self.kind_at(i) == TokenKind::Dot
            && self
                .tokens
                .get(i + 1)
                .is_some_and(|t| t.kind.is_identifier_like())
        {
            i += 2;
        }
        if self.kind_at(i) == TokenKind::Lt {
            let Some(close) = self.matching_angle_close(i, |_| true, None) else {
                return false;
            };
            i = close + 1;
        }
        if self.kind_at(i) != TokenKind::LParen {
            return false;
        }
        self.matching_close(i, Brackets::Mixed, |_| false, None)
            .is_some_and(|close| self.kind_at(close + 1) == TokenKind::Eq)
    }

    // Balanced lookahead avoids mistaking calls containing closures for local functions.
    fn untyped_local_func_ahead(&self) -> bool {
        let mut i = self.pos + 1;
        if self.kind_at(i) == TokenKind::Lt {
            let Some(close) = self.matching_angle_close(i, |_| true, None) else {
                return false;
            };
            i = close + 1;
        }
        if self.kind_at(i) != TokenKind::LParen {
            return false;
        }
        let Some(close) = self.matching_close(i, Brackets::Mixed, |_| false, None) else {
            return false;
        };
        i = close + 1;
        if matches!(self.kind_at(i), TokenKind::Async | TokenKind::Sync) {
            i += 1;
            if self.kind_at(i) == TokenKind::Star {
                i += 1;
            }
        }
        matches!(self.kind_at(i), TokenKind::LBrace | TokenKind::Arrow)
    }

    // A header without a body falls back to an ordinary call expression.
    fn try_parse_untyped_local_func(&mut self, start: usize) -> Option<Stmt> {
        self.speculate(|p| {
            let name = p.expect_ident();
            let type_params = p.parse_type_params();
            if !p.at(TokenKind::LParen) {
                return None;
            }
            let params = p.parse_formal_param_list();
            let async_marker = p.parse_async_marker();
            let body = p.parse_function_body(async_marker)?;
            Some(Stmt::LocalFunc(LocalFuncDecl {
                annotations: Vec::new(),
                async_marker,
                return_type: None,
                name,
                type_params,
                params,
                body,
                span: p.span_from(start),
            }))
        })
    }
}
