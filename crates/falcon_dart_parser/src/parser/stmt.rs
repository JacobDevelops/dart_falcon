use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;

use super::Parser;

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
        let start = self.cur().offset;

        // Labeled statement: `label: stmt`. A plain identifier directly followed
        // by `:` at statement start is a label. This never collides with a
        // ternary (`x ? a : b` — the `:` there follows `?`, not the leading
        // identifier) nor with `case`/`default:` labels (keywords, handled in
        // `parse_switch_stmt`). Labels nest, each wrapping its inner statement.
        if self.at(TokenKind::Ident) && self.peek(1).kind == TokenKind::Colon {
            let tok = self.advance(); // label identifier
            let label = Identifier::new(self.tok_text(&tok).to_string(), Self::tok_span(&tok));
            self.advance(); // :
            let stmt = self.parse_stmt();
            return Stmt::Labeled(LabeledStmt {
                label,
                stmt: Box::new(stmt),
                span: self.span_from(start),
            });
        }

        // `await for (...)` — an asynchronous for-in loop. The `await` precedes
        // the `for` keyword (unlike the loop-variable `await`), so recognise it
        // here and hand the `is_await` flag to the for-statement parser.
        if self.at(TokenKind::Await) && self.peek(1).kind == TokenKind::For {
            self.advance(); // await
            return self.parse_for_stmt(true, start);
        }

        match self.cur().kind {
            TokenKind::LBrace => {
                // Dart 3 map-pattern assignment: `{'k': a} = e;`. A balanced
                // `{...}` immediately followed by a single `=` is a destructuring
                // assignment, never a block or a map-literal expression statement.
                // Gate the trial on that `=` scan so plain blocks and map-literal
                // expression statements never enter (and roll back) the trial.
                if self.map_pattern_assign_ahead()
                    && let Some(stmt) = self.try_parse_pattern_assign(start)
                {
                    return stmt;
                }
                // Disambiguate block vs map/set literal expression statement.
                // If the first token inside is something that can't start a statement
                // (a literal value), treat the whole `{...}` as an expression — but
                // only when the brace group has no top-level `;`. A `;` at the outer
                // brace level (`{ 1; }`) can never appear in a collection literal, so
                // the group is a block of statements, not a `{1}` set literal.
                let next = self.peek(1).kind.clone();
                let looks_like_collection = matches!(
                    next,
                    TokenKind::StringLit
                        | TokenKind::IntLit
                        | TokenKind::DoubleLit
                        | TokenKind::True
                        | TokenKind::False
                        | TokenKind::Null
                ) && !self.brace_group_has_top_level_semicolon();
                if looks_like_collection {
                    self.parse_expr_or_local_decl_stmt()
                } else {
                    Stmt::Block(self.parse_block())
                }
            }
            TokenKind::If => self.parse_if_stmt(),
            TokenKind::For => self.parse_for_stmt(false, start),
            TokenKind::While => self.parse_while_stmt(),
            TokenKind::Do => self.parse_do_while_stmt(),
            TokenKind::Switch => self.parse_switch_stmt(),
            TokenKind::Try => self.parse_try_stmt(),
            TokenKind::Return => self.parse_return_stmt(),
            TokenKind::Throw => {
                self.advance();
                let value = self.parse_expr();
                self.eat(TokenKind::Semicolon);
                Stmt::Throw(ThrowStmt {
                    value,
                    is_rethrow: false,
                    span: self.span_from(start),
                })
            }
            TokenKind::Break => {
                self.advance();
                let label = if self.is_ident_like() {
                    self.parse_ident()
                } else {
                    None
                };
                self.eat(TokenKind::Semicolon);
                Stmt::Break(BreakStmt {
                    label,
                    span: self.span_from(start),
                })
            }
            TokenKind::Continue => {
                self.advance();
                let label = if self.is_ident_like() {
                    self.parse_ident()
                } else {
                    None
                };
                self.eat(TokenKind::Semicolon);
                Stmt::Continue(ContinueStmt {
                    label,
                    span: self.span_from(start),
                })
            }
            TokenKind::Assert => self.parse_assert_stmt(),
            TokenKind::Yield => self.parse_yield_stmt(),
            TokenKind::Rethrow => {
                self.advance();
                self.eat(TokenKind::Semicolon);
                Stmt::Throw(ThrowStmt {
                    value: Expr::Error {
                        span: self.span_from(start),
                    },
                    is_rethrow: true,
                    span: self.span_from(start),
                })
            }
            TokenKind::Semicolon => {
                self.advance();
                Stmt::Expr(ExprStmt {
                    expr: Expr::NullLit {
                        span: self.span_from(start),
                    },
                    span: self.span_from(start),
                })
            }
            _ => self.parse_expr_or_local_decl_stmt(),
        }
    }

    fn parse_if_stmt(&mut self) -> Stmt {
        let start = self.cur().offset;
        self.advance(); // if
        self.expect(TokenKind::LParen);
        let expr = self.parse_expr();
        let condition = if self.eat(TokenKind::Case).is_some() {
            let pattern = self.parse_pattern();
            // Optional `when <guard>` (mirrors switch-case guard parsing). The
            // pattern parser stops at the contextual `when` keyword on its own.
            let guard = if self.eat(TokenKind::When).is_some() {
                Some(Box::new(self.parse_expr()))
            } else {
                None
            };
            IfCondition::Case(expr, Box::new(pattern), guard)
        } else {
            IfCondition::Expr(expr)
        };
        self.expect(TokenKind::RParen);
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
        // `await for` is only valid over a for-in / pattern-for-in header, never a
        // C-style `for (init; cond; update)` loop.
        if is_await
            && !matches!(
                init,
                Some(ForInit::ForIn { .. }) | Some(ForInit::PatternForIn { .. })
            )
        {
            self.error(
                "'await' cannot be used with a C-style for loop; use 'await for (x in ...)'"
                    .to_string(),
            );
        }
        let body = self.parse_stmt();
        Stmt::For(ForStmt {
            is_await,
            init,
            condition,
            update,
            body: Box::new(body),
            span: self.span_from(start),
        })
    }

    /// Parses `init ; cond ; update )` or `pattern in expr )` — leaves `)` consumed.
    /// Lookahead (positioned at `final`/`var`): does an ident-led object pattern
    /// follow — a type name whose subpattern list opens with `(` before the loop
    /// clause ends? Distinguishes `final MapEntry(:k, :v) in …` (object pattern)
    /// from a plain typed decl `final Foo x in …` (no `(` before `in`).
    fn ident_led_object_pattern_ahead(&self) -> bool {
        if !Self::kind_is_ident_like(&self.peek(1).kind) {
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
        // Empty for (;;)
        if self.at(TokenKind::Semicolon) {
            self.advance();
            let cond = self.parse_optional_expr_before(TokenKind::Semicolon);
            self.expect(TokenKind::Semicolon);
            let update = self.parse_expr_list_until(TokenKind::RParen);
            self.expect(TokenKind::RParen);
            return (None, cond, update);
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
        {
            let saved_pat = self.pos;
            let saved_errs = self.errors.len();
            self.advance(); // final / var
            let pattern = self.parse_binding_pattern();
            if self.eat(TokenKind::In).is_some() {
                let iterable = self.parse_expr();
                self.expect(TokenKind::RParen);
                return (
                    Some(ForInit::PatternForIn {
                        pattern: Box::new(pattern),
                        iterable: Box::new(iterable),
                    }),
                    None,
                    Vec::new(),
                );
            }
            self.rewind_to(saved_pat);
            self.errors.truncate(saved_errs);
        }

        let saved = self.pos;

        // var / final / const → definitely a decl
        if self.at_any(&[TokenKind::Var, TokenKind::Final, TokenKind::Const]) {
            let is_const = self.at(TokenKind::Const);
            let is_final = self.at(TokenKind::Final);
            self.advance();
            let var_type = self.try_parse_type_for_for_decl();
            let name = self.expect_ident();
            if self.eat(TokenKind::In).is_some() {
                let iterable = self.parse_expr();
                self.expect(TokenKind::RParen);
                return (
                    Some(ForInit::ForIn {
                        // `ForIn` has no `is_const`; `const` is invalid here anyway.
                        is_final: is_final || is_const,
                        var_type,
                        name,
                        iterable: Box::new(iterable),
                    }),
                    None,
                    Vec::new(),
                );
            }
            let (decl, cond, update) =
                self.finish_c_style_for(is_final, is_const, var_type, name, saved);
            return (Some(ForInit::VarDecl(decl)), cond, update);
        }

        // late final Type name in / late final Type name = ...
        if self.at(TokenKind::Late) {
            let is_late = true;
            self.advance();
            let is_const = self.eat(TokenKind::Const).is_some();
            let is_final = !is_const && self.eat(TokenKind::Final).is_some();
            let _ = self.eat(TokenKind::Var);
            let var_type = self.try_parse_type_for_for_decl();
            let name = self.expect_ident();
            let (decl, cond, update) = self
                .finish_c_style_for_with_late(is_late, is_final, is_const, var_type, name, saved);
            return (Some(ForInit::VarDecl(decl)), cond, update);
        }

        // Try `Type name in` — typed for-in without var/final
        if self.is_type_start() {
            let ty = self.parse_type();
            if self.is_ident_like() {
                let name = self.expect_ident();
                if self.eat(TokenKind::In).is_some() {
                    let iterable = self.parse_expr();
                    self.expect(TokenKind::RParen);
                    return (
                        Some(ForInit::ForIn {
                            is_final: false,
                            var_type: Some(ty),
                            name,
                            iterable: Box::new(iterable),
                        }),
                        None,
                        Vec::new(),
                    );
                }
                // C-style with type+name
                let (decl, cond, update) =
                    self.finish_c_style_for(false, false, Some(ty), name, saved);
                return (Some(ForInit::VarDecl(decl)), cond, update);
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
                    is_final: false,
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
        self.expect(TokenKind::Semicolon);
        let cond = self.parse_optional_expr_before(TokenKind::Semicolon);
        self.expect(TokenKind::Semicolon);
        let update = self.parse_expr_list_until(TokenKind::RParen);
        self.expect(TokenKind::RParen);
        (Some(ForInit::Exprs(exprs)), cond, update)
    }

    fn try_parse_type_for_for_decl(&mut self) -> Option<DartType> {
        if !self.is_type_start() {
            return None;
        }
        let saved = self.pos;
        let ty = self.parse_type();
        if self.is_ident_like() {
            Some(ty)
        } else {
            self.rewind_to(saved);
            None
        }
    }

    fn finish_c_style_for(
        &mut self,
        is_final: bool,
        is_const: bool,
        var_type: Option<DartType>,
        first_name: Identifier,
        _saved: usize,
    ) -> (LocalVarDecl, Option<Expr>, Vec<Expr>) {
        let decl_start = first_name.span.start;
        let init = if self.eat(TokenKind::Eq).is_some() {
            Some(self.parse_expr())
        } else {
            None
        };
        let mut declarators = vec![VarDeclarator {
            name: first_name,
            initializer: init,
            span: self.span_from(decl_start),
        }];
        while self.eat(TokenKind::Comma).is_some() {
            let ds = self.cur().offset;
            let n = self.expect_ident();
            let iv = if self.eat(TokenKind::Eq).is_some() {
                Some(self.parse_expr())
            } else {
                None
            };
            declarators.push(VarDeclarator {
                name: n,
                initializer: iv,
                span: self.span_from(ds),
            });
        }
        let span = self.span_from(decl_start);
        self.expect(TokenKind::Semicolon);
        let cond = self.parse_optional_expr_before(TokenKind::Semicolon);
        self.expect(TokenKind::Semicolon);
        let update = self.parse_expr_list_until(TokenKind::RParen);
        self.expect(TokenKind::RParen);
        (
            LocalVarDecl {
                is_final,
                is_const,
                is_late: false,
                var_type,
                declarators,
                span,
            },
            cond,
            update,
        )
    }

    fn finish_c_style_for_with_late(
        &mut self,
        is_late: bool,
        is_final: bool,
        is_const: bool,
        var_type: Option<DartType>,
        first_name: Identifier,
        _saved: usize,
    ) -> (LocalVarDecl, Option<Expr>, Vec<Expr>) {
        let decl_start = first_name.span.start;
        let init = if self.eat(TokenKind::Eq).is_some() {
            Some(self.parse_expr())
        } else {
            None
        };
        let mut declarators = vec![VarDeclarator {
            name: first_name,
            initializer: init,
            span: self.span_from(decl_start),
        }];
        while self.eat(TokenKind::Comma).is_some() {
            let ds = self.cur().offset;
            let n = self.expect_ident();
            let iv = if self.eat(TokenKind::Eq).is_some() {
                Some(self.parse_expr())
            } else {
                None
            };
            declarators.push(VarDeclarator {
                name: n,
                initializer: iv,
                span: self.span_from(ds),
            });
        }
        let span = self.span_from(decl_start);
        self.expect(TokenKind::Semicolon);
        let cond = self.parse_optional_expr_before(TokenKind::Semicolon);
        self.expect(TokenKind::Semicolon);
        let update = self.parse_expr_list_until(TokenKind::RParen);
        self.expect(TokenKind::RParen);
        (
            LocalVarDecl {
                is_final,
                is_const,
                is_late,
                var_type,
                declarators,
                span,
            },
            cond,
            update,
        )
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
                    let pattern = self.parse_pattern();
                    let guard = if self.eat(TokenKind::When).is_some() {
                        Some(self.parse_expr())
                    } else {
                        None
                    };
                    self.expect(TokenKind::Colon);
                    case_kinds.push(SwitchCaseKind::Pattern(Box::new(pattern), Box::new(guard)));
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
        while matches!(self.tokens.get(i).map(|t| &t.kind), Some(TokenKind::Ident))
            && matches!(
                self.tokens.get(i + 1).map(|t| &t.kind),
                Some(TokenKind::Colon)
            )
        {
            i += 2;
        }
        i > self.pos
            && matches!(
                self.tokens.get(i).map(|t| &t.kind),
                Some(TokenKind::Case | TokenKind::Default)
            )
    }

    fn parse_try_stmt(&mut self) -> Stmt {
        let start = self.cur().offset;
        self.advance(); // try
        let body = self.parse_block();
        let mut catches = Vec::new();

        // `on` is a contextual keyword — lexed as Ident with text "on"
        while self.is_on_keyword() || self.at(TokenKind::Catch) {
            let catch_start = self.cur().offset;
            let exception_type = if self.is_on_keyword() {
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

    fn is_on_keyword(&self) -> bool {
        self.at(TokenKind::On)
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
        let start = self.cur().offset;

        let mut annotations = Vec::new();
        while self.at(TokenKind::At) {
            annotations.extend(self.parse_annotations());
        }

        // Dart 3 pattern-variable declaration: `(var|final) <pattern> = expr;`
        // (e.g. `final (a, b) = row;`). Only when the keyword is directly
        // followed by a `(`/`[`/`{` that opens a destructuring pattern — and the
        // pattern is followed by `=` (otherwise it is a record-typed var decl
        // such as `final (int, String) rec = ..`).
        if self.at_any(&[TokenKind::Final, TokenKind::Var])
            && matches!(
                self.peek(1).kind,
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace
            )
            && let Some(stmt) = self.try_parse_pattern_declaration(start)
        {
            return stmt;
        }

        // Dart 3 pattern-assignment: `(a, b) = expr;`, `[a, b] = expr;` — a
        // `(`/`[` destructuring pattern (no `var`/`final`) followed by `=`. The
        // trailing single `=` distinguishes it from a record/list expression
        // statement; without it we restore and fall through to expression parsing.
        if annotations.is_empty()
            && self.at_any(&[TokenKind::LParen, TokenKind::LBracket])
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

        if !annotations.is_empty() {
            // Annotations must precede a declaration
            return self.parse_local_var_or_func_after_annotations(start);
        }

        // Try to parse as typed local decl or function.
        // A leading `(` may begin a record-typed var decl (`(int, String) x = ...`)
        // or a plain parenthesized/record expression. Speculatively parse a type and
        // only commit when it is followed by `name =`/`;`/`,` (or a function body);
        // otherwise restore both position and any errors the speculation emitted.
        // `await`/`yield` lead an expression statement in async bodies; although
        // both are contextually ident-like, they are never type names, so keep
        // them out of the speculative typed-declaration path.
        if self.is_type_start() && !self.at_any(&[TokenKind::Await, TokenKind::Yield]) {
            let saved = self.pos;
            let saved_errs = self.errors.len();
            let ty = self.parse_type();
            if self.is_ident_like() {
                let name = self.expect_ident();
                // local function: name followed by ( or <
                if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) {
                    let type_params = if self.at(TokenKind::Lt) {
                        self.parse_type_params()
                    } else {
                        Vec::new()
                    };
                    let params = self.parse_formal_param_list();
                    let (is_async, is_generator) = self.parse_async_marker();
                    if let Some(body) = self.parse_function_body() {
                        return Stmt::LocalFunc(LocalFuncDecl {
                            return_type: Some(ty),
                            name,
                            type_params,
                            params,
                            is_async,
                            is_generator,
                            body,
                            span: self.span_from(start),
                        });
                    }
                    // No body — fall through as error
                    self.rewind_to(saved);
                    self.errors.truncate(saved_errs);
                } else if self.at_any(&[TokenKind::Eq, TokenKind::Semicolon, TokenKind::Comma]) {
                    // Typed var decl
                    let ds = name.span.start;
                    let init = if self.eat(TokenKind::Eq).is_some() {
                        Some(self.parse_expr())
                    } else {
                        None
                    };
                    let mut declarators = vec![VarDeclarator {
                        name,
                        initializer: init,
                        span: self.span_from(ds),
                    }];
                    while self.eat(TokenKind::Comma).is_some() {
                        let d2 = self.cur().offset;
                        let n = self.expect_ident();
                        let iv = if self.eat(TokenKind::Eq).is_some() {
                            Some(self.parse_expr())
                        } else {
                            None
                        };
                        declarators.push(VarDeclarator {
                            name: n,
                            initializer: iv,
                            span: self.span_from(d2),
                        });
                    }
                    // A real declaration ends at `;` (or the enclosing block's
                    // `}`/EOF). If instead a `:` follows, the speculative type +
                    // name + `= init` was actually a ternary whose `?` was misread
                    // as a nullable-type suffix (`a ? b = c : d = e`) — restore and
                    // parse it as an expression.
                    if self.at_any(&[TokenKind::Semicolon, TokenKind::RBrace, TokenKind::Eof]) {
                        self.eat(TokenKind::Semicolon);
                        return Stmt::LocalVar(LocalVarDecl {
                            is_final: false,
                            is_const: false,
                            is_late: false,
                            var_type: Some(ty),
                            declarators,
                            span: self.span_from(start),
                        });
                    }
                    self.rewind_to(saved);
                    self.errors.truncate(saved_errs);
                } else {
                    self.rewind_to(saved);
                    self.errors.truncate(saved_errs);
                }
            } else {
                self.rewind_to(saved);
                self.errors.truncate(saved_errs);
            }
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
        let kind = |i: usize| self.tokens.get(i).map(|t| t.kind.clone());
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
            .is_some_and(|t| Self::kind_is_ident_like(&t.kind))
        {
            return false;
        }
        i += 1;
        while matches!(kind(i), Some(TokenKind::Dot))
            && self
                .tokens
                .get(i + 1)
                .is_some_and(|t| Self::kind_is_ident_like(&t.kind))
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
        let is_const = self.eat(TokenKind::Const).is_some();
        let is_final = !is_const && self.eat(TokenKind::Final).is_some();
        let _ = self.eat(TokenKind::Var);

        let var_type = if self.is_type_start() {
            let saved = self.pos;
            let ty = self.parse_type();
            if self.is_ident_like() {
                Some(ty)
            } else {
                self.rewind_to(saved);
                None
            }
        } else {
            None
        };

        let ds = self.cur().offset;
        let name = self.expect_ident();
        let init = if self.eat(TokenKind::Eq).is_some() {
            Some(self.parse_expr())
        } else {
            None
        };
        let mut declarators = vec![VarDeclarator {
            name,
            initializer: init,
            span: self.span_from(ds),
        }];
        while self.eat(TokenKind::Comma).is_some() {
            let d2 = self.cur().offset;
            let n = self.expect_ident();
            let iv = if self.eat(TokenKind::Eq).is_some() {
                Some(self.parse_expr())
            } else {
                None
            };
            declarators.push(VarDeclarator {
                name: n,
                initializer: iv,
                span: self.span_from(d2),
            });
        }
        self.eat(TokenKind::Semicolon);
        Stmt::LocalVar(LocalVarDecl {
            is_final,
            is_const,
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
        let saved = self.pos;
        let saved_errs = self.errors.len();
        let is_final = self.at(TokenKind::Final);
        self.advance(); // final / var
        let pattern = self.parse_binding_pattern();
        // What follows the keyword did not parse cleanly as a destructuring
        // pattern — restore and let the LocalVar path handle it.
        if self.errors.len() > saved_errs {
            self.rewind_to(saved);
            self.errors.truncate(saved_errs);
            return None;
        }
        // A bare (possibly typed) variable/wildcard outer pattern — e.g.
        // `final (int, String) rec = ..` — is a record-typed variable
        // *declaration*, not a pattern declaration. Fall through to the LocalVar
        // path so the record type is preserved on the declaration.
        if matches!(pattern, Pattern::Variable { .. } | Pattern::Wildcard { .. }) {
            self.rewind_to(saved);
            self.errors.truncate(saved_errs);
            return None;
        }
        if self.eat(TokenKind::Eq).is_some() {
            let init = self.parse_expr();
            self.eat(TokenKind::Semicolon);
            return Some(Stmt::PatternDecl(PatternDeclaration {
                is_final,
                pattern,
                init,
                span: self.span_from(start),
            }));
        }
        self.rewind_to(saved);
        self.errors.truncate(saved_errs);
        None
    }

    fn parse_local_var_or_func_after_annotations(&mut self, start: usize) -> Stmt {
        // At this point annotations were consumed; just try a var decl or expr
        if self.is_type_start() {
            let saved = self.pos;
            let ty = self.parse_type();
            if self.is_ident_like() {
                let name = self.expect_ident();
                let ds = name.span.start;
                let init = if self.eat(TokenKind::Eq).is_some() {
                    Some(self.parse_expr())
                } else {
                    None
                };
                let declarators = vec![VarDeclarator {
                    name,
                    initializer: init,
                    span: self.span_from(ds),
                }];
                self.eat(TokenKind::Semicolon);
                return Stmt::LocalVar(LocalVarDecl {
                    is_final: false,
                    is_const: false,
                    is_late: false,
                    var_type: Some(ty),
                    declarators,
                    span: self.span_from(start),
                });
            }
            self.rewind_to(saved);
        }
        let expr = self.parse_expr();
        self.eat(TokenKind::Semicolon);
        Stmt::Expr(ExprStmt {
            expr,
            span: self.span_from(start),
        })
    }

    /// Balanced lookahead from a statement-leading `{`: is the brace group
    /// closed by a matching `}` that is immediately followed by a single `=`?
    /// Distinguishes a map-pattern assignment (`{'k': a} = e;`) from a plain
    /// block (`{ f(); }`) or a map-literal expression statement (`{'k': 1};`),
    /// which have no `=` after the closing brace.
    fn map_pattern_assign_ahead(&self) -> bool {
        debug_assert_eq!(self.cur().kind, TokenKind::LBrace);
        let kind = |i: usize| self.tokens.get(i).map(|t| &t.kind);
        let mut depth = 0i32;
        let mut i = self.pos;
        loop {
            match kind(i) {
                Some(TokenKind::LBrace) => depth += 1,
                Some(TokenKind::RBrace) => {
                    depth -= 1;
                    if depth == 0 {
                        return matches!(kind(i + 1), Some(TokenKind::Eq));
                    }
                }
                Some(TokenKind::Eof) | None => return false,
                _ => {}
            }
            i += 1;
        }
    }

    /// True when the balanced `{...}` group starting at the statement-leading `{`
    /// contains a `;` at its own (outer) brace level. A collection literal never
    /// holds a top-level `;`, so such a group is a block of statements
    /// (`{ 1; }`, `{ 'x'; f(); }`) rather than a `{1}` set/map literal. Nested `;`
    /// inside deeper brackets (e.g. a closure body) is ignored.
    fn brace_group_has_top_level_semicolon(&self) -> bool {
        debug_assert_eq!(self.cur().kind, TokenKind::LBrace);
        let kind = |i: usize| self.tokens.get(i).map(|t| &t.kind);
        let mut depth = 0i32;
        let mut i = self.pos;
        loop {
            match kind(i) {
                Some(TokenKind::LBrace | TokenKind::LParen | TokenKind::LBracket) => depth += 1,
                Some(TokenKind::RBrace | TokenKind::RParen | TokenKind::RBracket) => {
                    depth -= 1;
                    if depth == 0 {
                        return false;
                    }
                }
                Some(TokenKind::Semicolon) if depth == 1 => return true,
                Some(TokenKind::Eof) | None => return false,
                _ => {}
            }
            i += 1;
        }
    }

    /// Speculatively parse a Dart 3 pattern-assignment
    /// `(<pattern>|[<pattern>]|{<pattern>}) = expr;`. Returns `None` (restoring
    /// position and errors) when the `(`/`[`/`{` prefix is not a pattern followed
    /// by `=` — i.e. it is a record/list/map-literal expression.
    fn try_parse_pattern_assign(&mut self, start: usize) -> Option<Stmt> {
        let saved = self.pos;
        let saved_errs = self.errors.len();
        // Bare-identifier targets (`(a, b) = e;`) are assignable variable
        // references, not constant references — bind them in pattern-binding
        // mode so each parses as `Pattern::Variable` (which the visitor walks),
        // matching the declaration twin `var (a, b) = e;`.
        let pattern = self.parse_binding_pattern();
        // The `(`/`[`/`{` prefix did not parse cleanly as a pattern — it is an
        // ordinary parenthesised/record/collection expression whose contents a
        // pattern grammar rejects (`(v = 0) ?? 0;`, `({if (a) b = c});`). Restore
        // and let expression parsing handle it.
        if self.errors.len() > saved_errs {
            self.rewind_to(saved);
            self.errors.truncate(saved_errs);
            return None;
        }
        // A bare (possibly record-typed) variable/wildcard is a typed variable
        // *declaration* (`(String, bool) t = ..`), not a pattern assignment — let
        // it fall through to the typed-decl path.
        if matches!(pattern, Pattern::Variable { .. } | Pattern::Wildcard { .. }) {
            self.rewind_to(saved);
            self.errors.truncate(saved_errs);
            return None;
        }
        if self.at(TokenKind::Eq) {
            self.advance(); // =
            let value = self.parse_expr();
            self.eat(TokenKind::Semicolon);
            return Some(Stmt::PatternAssign(PatternAssignStmt {
                pattern,
                value,
                span: self.span_from(start),
            }));
        }
        self.rewind_to(saved);
        self.errors.truncate(saved_errs);
        None
    }

    /// Balanced lookahead from a statement-leading identifier: does a function
    /// body (`{`, `=>`, optionally after an `async`/`sync` marker) follow the
    /// `[<type params>] (params)` header? This distinguishes an untyped local
    /// function declaration from a call statement whose arguments may themselves
    /// contain closures (`g((x) => x);`) that would otherwise desync a lenient
    /// speculative parameter-list parse.
    fn untyped_local_func_ahead(&self) -> bool {
        let kind = |i: usize| self.tokens.get(i).map(|t| &t.kind);
        let mut i = self.pos + 1;

        // Optional `<type params>` — scan to the matching close angle bracket.
        if matches!(kind(i), Some(TokenKind::Lt)) {
            let mut depth = 0i32;
            loop {
                match kind(i) {
                    Some(TokenKind::Lt) => depth += 1,
                    Some(TokenKind::Gt) => depth -= 1,
                    Some(TokenKind::GtGt) => depth -= 2,
                    Some(TokenKind::GtGtGt) => depth -= 3,
                    Some(TokenKind::Eof) | None => return false,
                    _ => {}
                }
                i += 1;
                if depth <= 0 {
                    break;
                }
            }
        }

        // Parameter list — scan to its matching `)`.
        if !matches!(kind(i), Some(TokenKind::LParen)) {
            return false;
        }
        let mut depth = 0i32;
        loop {
            match kind(i) {
                Some(TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace) => depth += 1,
                Some(TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace) => depth -= 1,
                Some(TokenKind::Eof) | None => return false,
                _ => {}
            }
            i += 1;
            if depth == 0 {
                break;
            }
        }

        // Optional `async` / `sync` [`*`] marker.
        if matches!(kind(i), Some(TokenKind::Async | TokenKind::Sync)) {
            i += 1;
            if matches!(kind(i), Some(TokenKind::Star)) {
                i += 1;
            }
        }

        matches!(kind(i), Some(TokenKind::LBrace | TokenKind::Arrow))
    }

    /// Speculatively parse an untyped local function declaration
    /// `name [<T>](params) [async|sync[*]] (block | => expr)`. Returns `None`
    /// (restoring position and errors) when no function body follows the
    /// parameter list — that is a plain call statement, parsed as an expression.
    fn try_parse_untyped_local_func(&mut self, start: usize) -> Option<Stmt> {
        let saved = self.pos;
        let saved_errs = self.errors.len();
        let name = self.expect_ident();
        let type_params = if self.at(TokenKind::Lt) {
            self.parse_type_params()
        } else {
            Vec::new()
        };
        if !self.at(TokenKind::LParen) {
            self.rewind_to(saved);
            self.errors.truncate(saved_errs);
            return None;
        }
        let params = self.parse_formal_param_list();
        let (is_async, is_generator) = self.parse_async_marker();
        if let Some(body) = self.parse_function_body() {
            return Some(Stmt::LocalFunc(LocalFuncDecl {
                is_async,
                is_generator,
                return_type: None,
                name,
                type_params,
                params,
                body,
                span: self.span_from(start),
            }));
        }
        self.rewind_to(saved);
        self.errors.truncate(saved_errs);
        None
    }
}
