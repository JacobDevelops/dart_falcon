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
        Block { stmts, span: self.span_from(start) }
    }

    pub(super) fn parse_stmt(&mut self) -> Stmt {
        let start = self.cur().offset;
        match self.cur().kind {
            TokenKind::LBrace => {
                // Disambiguate block vs map/set literal expression statement.
                // If the first token inside is something that can't start a statement
                // (a literal value), treat the whole `{...}` as an expression.
                let next = self.peek(1).kind.clone();
                let looks_like_collection = matches!(
                    next,
                    TokenKind::StringLit
                        | TokenKind::IntLit
                        | TokenKind::DoubleLit
                        | TokenKind::True
                        | TokenKind::False
                        | TokenKind::Null
                );
                if looks_like_collection {
                    self.parse_expr_or_local_decl_stmt()
                } else {
                    Stmt::Block(self.parse_block())
                }
            }
            TokenKind::If => self.parse_if_stmt(),
            TokenKind::For => self.parse_for_stmt(),
            TokenKind::While => self.parse_while_stmt(),
            TokenKind::Do => self.parse_do_while_stmt(),
            TokenKind::Switch => self.parse_switch_stmt(),
            TokenKind::Try => self.parse_try_stmt(),
            TokenKind::Return => self.parse_return_stmt(),
            TokenKind::Throw => {
                self.advance();
                let value = self.parse_expr();
                self.eat(TokenKind::Semicolon);
                Stmt::Throw(ThrowStmt { value, is_rethrow: false, span: self.span_from(start) })
            }
            TokenKind::Break => {
                self.advance();
                let label = if self.is_ident_like() { self.parse_ident() } else { None };
                self.eat(TokenKind::Semicolon);
                Stmt::Break(BreakStmt { label, span: self.span_from(start) })
            }
            TokenKind::Continue => {
                self.advance();
                let label = if self.is_ident_like() { self.parse_ident() } else { None };
                self.eat(TokenKind::Semicolon);
                Stmt::Continue(ContinueStmt { label, span: self.span_from(start) })
            }
            TokenKind::Assert => self.parse_assert_stmt(),
            TokenKind::Yield => self.parse_yield_stmt(),
            TokenKind::Rethrow => {
                self.advance();
                self.eat(TokenKind::Semicolon);
                Stmt::Throw(ThrowStmt {
                    value: Expr::Error { span: self.span_from(start) },
                    is_rethrow: true,
                    span: self.span_from(start),
                })
            }
            TokenKind::Semicolon => {
                self.advance();
                Stmt::Expr(ExprStmt {
                    expr: Expr::NullLit { span: self.span_from(start) },
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
            IfCondition::Case(expr, Box::new(pattern))
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

    fn parse_for_stmt(&mut self) -> Stmt {
        let start = self.cur().offset;
        self.advance(); // for
        let is_await = self.eat(TokenKind::Await).is_some();
        self.expect(TokenKind::LParen);
        let (init, condition, update) = self.parse_for_clauses();
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
    fn parse_for_clauses(&mut self) -> (Option<ForInit>, Option<Expr>, Vec<Expr>) {
        // Empty for (;;)
        if self.at(TokenKind::Semicolon) {
            self.advance();
            let cond = self.parse_optional_expr_before(TokenKind::Semicolon);
            self.expect(TokenKind::Semicolon);
            let update = self.parse_expr_list_until(TokenKind::RParen);
            self.expect(TokenKind::RParen);
            return (None, cond, update);
        }

        let saved = self.pos;

        // var / final / const → definitely a decl
        if self.at_any(&[TokenKind::Var, TokenKind::Final, TokenKind::Const]) {
            let is_final = !self.at(TokenKind::Var);
            self.advance();
            let var_type = self.try_parse_type_for_for_decl();
            let name = self.expect_ident();
            if self.eat(TokenKind::In).is_some() {
                let iterable = self.parse_expr();
                self.expect(TokenKind::RParen);
                return (Some(ForInit::ForIn { is_final, var_type, name, iterable: Box::new(iterable) }), None, Vec::new());
            }
            let (decl, cond, update) = self.finish_c_style_for(is_final, var_type, name, saved);
            return (Some(ForInit::VarDecl(decl)), cond, update);
        }

        // late final Type name in / late final Type name = ...
        if self.at(TokenKind::Late) {
            let is_late = true;
            self.advance();
            let is_final = self.eat(TokenKind::Final).is_some();
            let _ = self.eat(TokenKind::Var);
            let var_type = self.try_parse_type_for_for_decl();
            let name = self.expect_ident();
            let (decl, cond, update) = self.finish_c_style_for_with_late(is_late, is_final, var_type, name, saved);
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
                        Some(ForInit::ForIn { is_final: false, var_type: Some(ty), name, iterable: Box::new(iterable) }),
                        None,
                        Vec::new(),
                    );
                }
                // C-style with type+name
                let (decl, cond, update) = self.finish_c_style_for(false, Some(ty), name, saved);
                return (Some(ForInit::VarDecl(decl)), cond, update);
            }
            // Rollback — couldn't parse as decl
            self.pos = saved;
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
                Some(ForInit::ForIn { is_final: false, var_type: None, name, iterable: Box::new(iterable) }),
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
        if !self.is_type_start() { return None; }
        let saved = self.pos;
        let ty = self.parse_type();
        if self.is_ident_like() { Some(ty) } else { self.pos = saved; None }
    }

    fn finish_c_style_for(
        &mut self,
        is_final: bool,
        var_type: Option<DartType>,
        first_name: Identifier,
        _saved: usize,
    ) -> (LocalVarDecl, Option<Expr>, Vec<Expr>) {
        let decl_start = first_name.span.start;
        let init = if self.eat(TokenKind::Eq).is_some() { Some(self.parse_expr()) } else { None };
        let mut declarators = vec![VarDeclarator { name: first_name, initializer: init, span: self.span_from(decl_start) }];
        while self.eat(TokenKind::Comma).is_some() {
            let ds = self.cur().offset;
            let n = self.expect_ident();
            let iv = if self.eat(TokenKind::Eq).is_some() { Some(self.parse_expr()) } else { None };
            declarators.push(VarDeclarator { name: n, initializer: iv, span: self.span_from(ds) });
        }
        let span = self.span_from(decl_start);
        self.expect(TokenKind::Semicolon);
        let cond = self.parse_optional_expr_before(TokenKind::Semicolon);
        self.expect(TokenKind::Semicolon);
        let update = self.parse_expr_list_until(TokenKind::RParen);
        self.expect(TokenKind::RParen);
        (LocalVarDecl { is_final, is_const: false, is_late: false, var_type, declarators, span }, cond, update)
    }

    fn finish_c_style_for_with_late(
        &mut self,
        is_late: bool,
        is_final: bool,
        var_type: Option<DartType>,
        first_name: Identifier,
        _saved: usize,
    ) -> (LocalVarDecl, Option<Expr>, Vec<Expr>) {
        let decl_start = first_name.span.start;
        let init = if self.eat(TokenKind::Eq).is_some() { Some(self.parse_expr()) } else { None };
        let mut declarators = vec![VarDeclarator { name: first_name, initializer: init, span: self.span_from(decl_start) }];
        while self.eat(TokenKind::Comma).is_some() {
            let ds = self.cur().offset;
            let n = self.expect_ident();
            let iv = if self.eat(TokenKind::Eq).is_some() { Some(self.parse_expr()) } else { None };
            declarators.push(VarDeclarator { name: n, initializer: iv, span: self.span_from(ds) });
        }
        let span = self.span_from(decl_start);
        self.expect(TokenKind::Semicolon);
        let cond = self.parse_optional_expr_before(TokenKind::Semicolon);
        self.expect(TokenKind::Semicolon);
        let update = self.parse_expr_list_until(TokenKind::RParen);
        self.expect(TokenKind::RParen);
        (LocalVarDecl { is_final, is_const: false, is_late, var_type, declarators, span }, cond, update)
    }

    fn parse_optional_expr_before(&mut self, stop: TokenKind) -> Option<Expr> {
        if self.at(stop) { None } else { Some(self.parse_expr()) }
    }

    fn parse_expr_list_until(&mut self, stop: TokenKind) -> Vec<Expr> {
        let mut exprs = Vec::new();
        while self.cur().kind != stop && !self.at(TokenKind::Eof) {
            exprs.push(self.parse_expr());
            if self.eat(TokenKind::Comma).is_none() { break; }
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
        Stmt::While(WhileStmt { condition, body: Box::new(body), span: self.span_from(start) })
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
        Stmt::DoWhile(DoWhileStmt { body: Box::new(body), condition, span: self.span_from(start) })
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

            loop {
                if self.at(TokenKind::Case) {
                    self.advance();
                    let pattern = self.parse_pattern();
                    let guard = if self.eat(TokenKind::When).is_some() { Some(self.parse_expr()) } else { None };
                    self.expect(TokenKind::Colon);
                    case_kinds.push(SwitchCaseKind::Pattern(Box::new(pattern), Box::new(guard)));
                } else if self.at(TokenKind::Default) {
                    self.advance();
                    self.expect(TokenKind::Colon);
                    case_kinds.push(SwitchCaseKind::Default);
                } else {
                    break;
                }
                if !self.at(TokenKind::Case) && !self.at(TokenKind::Default) {
                    break;
                }
            }

            if case_kinds.is_empty() {
                self.error(format!("expected case or default, got {:?}", self.cur().kind));
                self.advance();
                continue;
            }

            let mut body = Vec::new();
            while !self.at(TokenKind::Case)
                && !self.at(TokenKind::Default)
                && !self.at(TokenKind::RBrace)
                && !self.at(TokenKind::Eof)
            {
                body.push(self.parse_stmt());
            }
            cases.push(SwitchCase { cases: case_kinds, body, span: self.span_from(case_start) });
        }

        self.expect(TokenKind::RBrace);
        Stmt::Switch(SwitchStmt { subject, cases, span: self.span_from(start) })
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
                let s = if self.eat(TokenKind::Comma).is_some() { Some(self.expect_ident()) } else { None };
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

        Stmt::TryCatch(TryCatchStmt { body, catches, finally, span: self.span_from(start) })
    }

    fn is_on_keyword(&self) -> bool {
        self.at(TokenKind::On)
    }

    fn parse_return_stmt(&mut self) -> Stmt {
        let start = self.cur().offset;
        self.advance(); // return
        let value = if !self.at(TokenKind::Semicolon) && !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            Some(self.parse_expr())
        } else {
            None
        };
        self.eat(TokenKind::Semicolon);
        Stmt::Return(ReturnStmt { value, span: self.span_from(start) })
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
        Stmt::Assert(AssertStmt { condition, message, span: self.span_from(start) })
    }

    fn parse_yield_stmt(&mut self) -> Stmt {
        let start = self.cur().offset;
        self.advance(); // yield
        let is_star = self.eat(TokenKind::Star).is_some();
        let value = self.parse_expr();
        self.eat(TokenKind::Semicolon);
        Stmt::Yield(YieldStmt { is_star, value, span: self.span_from(start) })
    }

    fn parse_expr_or_local_decl_stmt(&mut self) -> Stmt {
        let start = self.cur().offset;

        let mut annotations = Vec::new();
        while self.at(TokenKind::At) {
            annotations.extend(self.parse_annotations());
        }

        // late / final / var → definitely a var decl
        if self.at(TokenKind::Late)
            || self.at_any(&[TokenKind::Final, TokenKind::Var])
        {
            return self.parse_local_var_decl(start);
        }
        // `const` can be either a var decl or a const expression (constructor/collection).
        // Disambiguate by peeking at the token after the type name:
        //   `const Type name` or `const name =` → var decl
        //   `const Type(` or `const Type.` or `const [` or `const {` → expr
        if self.at(TokenKind::Const) {
            let p1 = self.peek(1).kind.clone();
            let p2 = self.peek(2).kind.clone();
            let is_const_expr = matches!(p1, TokenKind::LBracket | TokenKind::LBrace)
                || p2 == TokenKind::LParen
                || p2 == TokenKind::Dot;
            if !is_const_expr {
                return self.parse_local_var_decl(start);
            }
            // Fall through to expression parsing
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
        if self.is_type_start() {
            let saved = self.pos;
            let saved_errs = self.errors.len();
            let ty = self.parse_type();
            if self.is_ident_like() {
                let name = self.expect_ident();
                // local function: name followed by ( or <
                if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) {
                    let type_params = if self.at(TokenKind::Lt) { self.parse_type_params() } else { Vec::new() };
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
                    self.pos = saved;
                    self.errors.truncate(saved_errs);
                } else if self.at_any(&[TokenKind::Eq, TokenKind::Semicolon, TokenKind::Comma]) {
                    // Typed var decl
                    let ds = name.span.start;
                    let init = if self.eat(TokenKind::Eq).is_some() { Some(self.parse_expr()) } else { None };
                    let mut declarators = vec![VarDeclarator { name, initializer: init, span: self.span_from(ds) }];
                    while self.eat(TokenKind::Comma).is_some() {
                        let d2 = self.cur().offset;
                        let n = self.expect_ident();
                        let iv = if self.eat(TokenKind::Eq).is_some() { Some(self.parse_expr()) } else { None };
                        declarators.push(VarDeclarator { name: n, initializer: iv, span: self.span_from(d2) });
                    }
                    self.eat(TokenKind::Semicolon);
                    return Stmt::LocalVar(LocalVarDecl {
                        is_final: false,
                        is_const: false,
                        is_late: false,
                        var_type: Some(ty),
                        declarators,
                        span: self.span_from(start),
                    });
                } else {
                    self.pos = saved;
                    self.errors.truncate(saved_errs);
                }
            } else {
                self.pos = saved;
                self.errors.truncate(saved_errs);
            }
        }

        let expr = self.parse_expr();
        self.eat(TokenKind::Semicolon);
        Stmt::Expr(ExprStmt { expr, span: self.span_from(start) })
    }

    fn parse_local_var_decl(&mut self, start: usize) -> Stmt {
        let is_late = self.eat(TokenKind::Late).is_some();
        let is_const = self.eat(TokenKind::Const).is_some();
        let is_final = !is_const && self.eat(TokenKind::Final).is_some();
        let _ = self.eat(TokenKind::Var);

        let var_type = if self.is_type_start() {
            let saved = self.pos;
            let ty = self.parse_type();
            if self.is_ident_like() { Some(ty) } else { self.pos = saved; None }
        } else {
            None
        };

        let ds = self.cur().offset;
        let name = self.expect_ident();
        let init = if self.eat(TokenKind::Eq).is_some() { Some(self.parse_expr()) } else { None };
        let mut declarators = vec![VarDeclarator { name, initializer: init, span: self.span_from(ds) }];
        while self.eat(TokenKind::Comma).is_some() {
            let d2 = self.cur().offset;
            let n = self.expect_ident();
            let iv = if self.eat(TokenKind::Eq).is_some() { Some(self.parse_expr()) } else { None };
            declarators.push(VarDeclarator { name: n, initializer: iv, span: self.span_from(d2) });
        }
        self.eat(TokenKind::Semicolon);
        Stmt::LocalVar(LocalVarDecl { is_final, is_const, is_late, var_type, declarators, span: self.span_from(start) })
    }

    fn parse_local_var_or_func_after_annotations(&mut self, start: usize) -> Stmt {
        // At this point annotations were consumed; just try a var decl or expr
        if self.is_type_start() {
            let saved = self.pos;
            let ty = self.parse_type();
            if self.is_ident_like() {
                let name = self.expect_ident();
                let ds = name.span.start;
                let init = if self.eat(TokenKind::Eq).is_some() { Some(self.parse_expr()) } else { None };
                let declarators = vec![VarDeclarator { name, initializer: init, span: self.span_from(ds) }];
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
            self.pos = saved;
        }
        let expr = self.parse_expr();
        self.eat(TokenKind::Semicolon);
        Stmt::Expr(ExprStmt { expr, span: self.span_from(start) })
    }
}
