use falcon_syntax::Feature;
use falcon_syntax::cst::NodeKind;
use falcon_syntax::token::TokenKind;

use super::{CompletedMarker, Marker, Parser, expr::Brackets};

impl<'src> Parser<'src> {
    pub(super) fn parse_block(&mut self) -> CompletedMarker {
        let marker = self.start();
        self.expect(TokenKind::LBrace);
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            self.parse_stmt();
        }
        self.expect(TokenKind::RBrace);
        marker.complete(self, NodeKind::Block)
    }

    pub(super) fn parse_stmt(&mut self) -> CompletedMarker {
        let before = self.pos;
        let marker = self.start();
        let stmt = self.guarded(
            |p| {
                let error = p.start();
                p.advance();
                error.complete(p, NodeKind::ErrorExpr);
                Err(NodeKind::ExprStmt)
            },
            |p| {
                // A statement-leading `label:` is unambiguous; labels may nest.
                if p.at(TokenKind::Ident) && p.peek(1).kind == TokenKind::Colon {
                    let name_marker = p.start();
                    p.advance();
                    name_marker.complete(p, NodeKind::Identifier);
                    p.advance(); // :
                    p.parse_stmt();
                    return Err(NodeKind::LabeledStmt);
                }

                // Reject sync-body `await for`, recovering as an ordinary for-loop.
                if p.at(TokenKind::Await) && p.peek(1).kind == TokenKind::For {
                    if !p.ctx.body.is_async() {
                        p.error("'await for' is only allowed in an async function body");
                    }
                    p.advance(); // await
                    return Err(p.parse_for_stmt(p.ctx.body.is_async()));
                }

                Err(match p.cur().kind {
                    TokenKind::LBrace => {
                        // Gate speculation on `=` so ordinary blocks retain diagnostics.
                        if p.map_pattern_assign_ahead()
                            && let Some(stmt) = p.try_parse_pattern_assign()
                        {
                            return Err(stmt);
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
                            return Ok(p.parse_block());
                        }
                    }
                    TokenKind::If => p.parse_if_stmt(),
                    TokenKind::For => p.parse_for_stmt(false),
                    TokenKind::While => p.parse_while_stmt(),
                    TokenKind::Do => p.parse_do_while_stmt(),
                    TokenKind::Switch => p.parse_switch_stmt(),
                    TokenKind::Try => p.parse_try_stmt(),
                    TokenKind::Return => p.parse_return_stmt(),
                    TokenKind::Throw => {
                        p.advance();
                        p.parse_expr();
                        p.eat(TokenKind::Semicolon);
                        NodeKind::ThrowStmt
                    }
                    TokenKind::Break => {
                        p.advance();
                        p.parse_ident();
                        p.eat(TokenKind::Semicolon);
                        NodeKind::BreakStmt
                    }
                    TokenKind::Continue => {
                        p.advance();
                        p.parse_ident();
                        p.eat(TokenKind::Semicolon);
                        NodeKind::ContinueStmt
                    }
                    TokenKind::Assert => p.parse_assert_stmt(),
                    // Outside generators, `yield` remains an ordinary identifier.
                    TokenKind::Yield if p.ctx.body.is_generator() => p.parse_yield_stmt(),
                    TokenKind::Rethrow => {
                        p.advance();
                        p.eat(TokenKind::Semicolon);
                        NodeKind::RethrowStmt
                    }
                    TokenKind::Semicolon => {
                        p.advance();
                        NodeKind::EmptyStmt
                    }
                    _ => p.parse_expr_or_local_decl_stmt(),
                })
            },
        );
        // A statement can enter just below the depth ceiling while its expression
        // returns a missing node without consuming input. Keep enclosing block
        // recovery moving rather than parsing the same token forever.
        if self.pos == before && self.depth_limit_hit && !self.at(TokenKind::Eof) {
            let error = self.start();
            self.advance();
            error.complete(self, NodeKind::ErrorExpr);
        }
        match stmt {
            Ok(block) => {
                marker.abandon(self);
                block
            }
            Err(kind) => marker.complete(self, kind),
        }
    }

    fn parse_if_stmt(&mut self) -> NodeKind {
        self.advance(); // if
        self.parse_if_condition();
        self.parse_stmt();
        if self.eat(TokenKind::Else).is_some() {
            self.parse_stmt();
        }
        NodeKind::IfStmt
    }

    fn parse_for_stmt(&mut self, is_await: bool) -> NodeKind {
        if is_await {
            self.missing(NodeKind::AwaitFor);
        }
        self.advance(); // for
        self.expect(TokenKind::LParen);
        let is_for_in = self.parse_for_clauses();
        self.check_await_for_header(is_await, is_for_in);
        self.parse_stmt();
        NodeKind::ForStmt
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

    pub(super) fn parse_for_clauses(&mut self) -> bool {
        let header = self.start();
        let init_marker = self.start();
        self.parse_annotations();
        let is_for_in = self.parse_unannotated_for_clauses(init_marker);
        header.complete(self, NodeKind::ForHeader);
        is_for_in
    }

    fn parse_unannotated_for_clauses(&mut self, init_marker: Marker) -> bool {
        // Empty for (;;)
        if self.at(TokenKind::Semicolon) {
            init_marker.abandon(self);
            return self.finish_for_clauses();
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
            && let Some(is_for_in) = self.speculate(|p| {
                p.advance();
                p.parse_binding_pattern();
                p.eat(TokenKind::In)?;
                p.parse_expr();
                init_marker.complete(p, NodeKind::PatternForInInit);
                p.expect(TokenKind::RParen);
                Some(true)
            })
        {
            return is_for_in;
        }

        let saved = self.checkpoint();

        // var / final / const → definitely a decl
        if self.at_any(&[TokenKind::Var, TokenKind::Final, TokenKind::Const]) {
            self.advance();
            self.try_parse_type_followed_by(TokenKind::is_identifier_like);
            let name = self.expect_ident();
            if self.eat(TokenKind::In).is_some() {
                self.parse_expr();
                init_marker.complete(self, NodeKind::ForInInit);
                self.expect(TokenKind::RParen);
                return true;
            }
            return self.finish_c_style_for(init_marker, name);
        }

        // late final Type name in / late final Type name = ...
        if self.at(TokenKind::Late) {
            self.advance();
            let is_const = self.eat(TokenKind::Const).is_some();
            if !is_const {
                self.eat(TokenKind::Final);
            }
            self.eat(TokenKind::Var);
            self.try_parse_type_followed_by(TokenKind::is_identifier_like);
            let name = self.expect_ident();
            return self.finish_c_style_for(init_marker, name);
        }

        // Try `Type name in` — typed for-in without var/final
        if self.is_type_start() {
            self.parse_type();
            if self.cur().kind.is_identifier_like() {
                let name = self.expect_ident();
                if self.eat(TokenKind::In).is_some() {
                    self.parse_expr();
                    init_marker.complete(self, NodeKind::ForInInit);
                    self.expect(TokenKind::RParen);
                    return true;
                }
                // C-style with type+name
                return self.finish_c_style_for(init_marker, name);
            }
            // Rollback — couldn't parse as decl
            self.restore_cursor(saved);
        }

        // Expression(s) for-loop init, or bare `ident in expr`
        let expr = self.parse_expr();
        if self.eat(TokenKind::In).is_some() {
            if expr.kind != NodeKind::Identifier {
                self.error("expected identifier in for-in");
                self.missing(NodeKind::Identifier);
            }
            self.parse_expr();
            init_marker.complete(self, NodeKind::ForInInit);
            self.expect(TokenKind::RParen);
            return true;
        }
        // C-style: expr ; cond ; update
        while self.eat(TokenKind::Comma).is_some() {
            self.parse_expr();
        }
        init_marker.complete(self, NodeKind::ForExprsInit);
        self.finish_for_clauses()
    }

    fn finish_c_style_for(&mut self, init_marker: Marker, first_name: CompletedMarker) -> bool {
        self.parse_declarators(first_name);
        init_marker.complete(self, NodeKind::LocalVarDecl);
        self.finish_for_clauses()
    }

    /// Finish a C-style header after its initializer, starting at the first `;`.
    fn finish_for_clauses(&mut self) -> bool {
        self.expect(TokenKind::Semicolon);
        let condition_marker = self.start();
        self.parse_optional_expr_before(TokenKind::Semicolon);
        condition_marker.complete(self, NodeKind::ForCondition);
        self.expect(TokenKind::Semicolon);
        let updates_marker = self.start();
        self.parse_expr_list_until(TokenKind::RParen);
        updates_marker.complete(self, NodeKind::ForUpdates);
        self.expect(TokenKind::RParen);
        false
    }

    fn parse_optional_expr_before(&mut self, stop: TokenKind) -> Option<CompletedMarker> {
        if self.at(stop) {
            None
        } else {
            Some(self.parse_expr())
        }
    }

    fn parse_expr_list_until(&mut self, stop: TokenKind) {
        while self.cur().kind != stop && !self.at(TokenKind::Eof) {
            self.parse_expr();
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
    }

    fn parse_while_stmt(&mut self) -> NodeKind {
        self.advance(); // while
        self.expect(TokenKind::LParen);
        self.parse_expr();
        self.expect(TokenKind::RParen);
        self.parse_stmt();
        NodeKind::WhileStmt
    }

    fn parse_do_while_stmt(&mut self) -> NodeKind {
        self.advance(); // do
        self.parse_stmt();
        self.expect(TokenKind::While);
        self.expect(TokenKind::LParen);
        self.parse_expr();
        self.expect(TokenKind::RParen);
        self.eat(TokenKind::Semicolon);
        NodeKind::DoWhileStmt
    }

    fn parse_switch_stmt(&mut self) -> NodeKind {
        self.advance(); // switch
        self.expect(TokenKind::LParen);
        self.parse_expr();
        self.expect(TokenKind::RParen);
        self.expect(TokenKind::LBrace);

        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let case_marker = self.start();
            let mut has_case = false;

            loop {
                // A case may carry statement labels used as `continue label;`
                // targets: `label: case 2:` / `label: default:`. Retain them.
                while self.at(TokenKind::Ident) && self.peek(1).kind == TokenKind::Colon {
                    self.expect_ident(); // label
                    self.advance(); // :
                }
                if self.at(TokenKind::Case) {
                    let label_marker = self.start();
                    self.advance();
                    // Before patterns (Dart < 3.0) a case operand is a constant
                    // expression, not a pattern: `case 1 + 2:`, `case -x:`.
                    if !self.version.supports(Feature::Patterns) {
                        self.parse_expr();
                        self.expect(TokenKind::Colon);
                        label_marker.complete(self, NodeKind::CaseExprLabel);
                    } else {
                        self.parse_pattern();
                        if self.eat(TokenKind::When).is_some() {
                            self.parse_expr();
                        }
                        self.expect(TokenKind::Colon);
                        label_marker.complete(self, NodeKind::CasePatternLabel);
                    }
                    has_case = true;
                } else if self.at(TokenKind::Default) {
                    let label_marker = self.start();
                    self.advance();
                    self.expect(TokenKind::Colon);
                    has_case = true;
                    label_marker.complete(self, NodeKind::DefaultLabel);
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

            if !has_case {
                self.error(format!(
                    "expected case or default, got {:?}",
                    self.cur().kind
                ));
                self.advance();
                case_marker.abandon(self);
                continue;
            }

            while !self.at(TokenKind::Case)
                && !self.at(TokenKind::Default)
                && !self.at(TokenKind::RBrace)
                && !self.at(TokenKind::Eof)
                && !self.at_labeled_case()
            {
                self.parse_stmt();
            }
            case_marker.complete(self, NodeKind::SwitchCase);
        }

        self.expect(TokenKind::RBrace);
        NodeKind::SwitchStmt
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

    fn parse_try_stmt(&mut self) -> NodeKind {
        self.advance(); // try
        self.parse_block();

        while self.at(TokenKind::On) || self.at(TokenKind::Catch) {
            let catch_marker = self.start();
            if self.at(TokenKind::On) {
                self.advance(); // on
                self.parse_type();
            }
            if self.eat(TokenKind::Catch).is_some() {
                self.expect(TokenKind::LParen);
                self.expect_ident();
                if self.eat(TokenKind::Comma).is_some() {
                    self.expect_ident();
                }
                self.expect(TokenKind::RParen);
            }
            self.parse_block();
            catch_marker.complete(self, NodeKind::CatchClause);
        }

        if self.eat(TokenKind::Finally).is_some() {
            self.parse_block();
        }
        NodeKind::TryCatchStmt
    }

    fn parse_return_stmt(&mut self) -> NodeKind {
        self.advance(); // return
        if !self.at(TokenKind::Semicolon) && !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof)
        {
            self.parse_expr();
        }
        self.eat(TokenKind::Semicolon);
        NodeKind::ReturnStmt
    }

    fn parse_assert_stmt(&mut self) -> NodeKind {
        self.advance(); // assert
        self.expect(TokenKind::LParen);
        self.parse_expr();
        if self.eat(TokenKind::Comma).is_some() && !self.at(TokenKind::RParen) {
            self.parse_expr();
        }
        self.eat(TokenKind::Comma); // optional trailing comma
        self.expect(TokenKind::RParen);
        self.eat(TokenKind::Semicolon);
        NodeKind::AssertStmt
    }

    fn parse_yield_stmt(&mut self) -> NodeKind {
        self.advance(); // yield
        self.eat(TokenKind::Star);
        self.parse_expr();
        self.eat(TokenKind::Semicolon);
        NodeKind::YieldStmt
    }

    fn parse_expr_or_local_decl_stmt(&mut self) -> NodeKind {
        let unannotated = !self.at(TokenKind::At);
        while self.at(TokenKind::At) {
            self.parse_annotations();
        }
        self.parse_local_decl_or_expr_stmt(unannotated)
    }

    fn parse_local_decl_or_expr_stmt(&mut self, unannotated: bool) -> NodeKind {
        // Dart 3 pattern-variable declaration: `(var|final) <pattern> = expr;`.
        // Destructuring starts with a group or an object-pattern type; require
        // `=` after the pattern to distinguish record-typed variable declarations.
        if self.at_any(&[TokenKind::Final, TokenKind::Var])
            && (matches!(
                self.peek(1).kind,
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace
            ) || self.object_pattern_assign_ahead(self.pos + 1))
            && let Some(stmt) = self.try_parse_pattern_declaration()
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
            && let Some(stmt) = self.try_parse_pattern_assign()
        {
            return stmt;
        }

        // late / final / var → definitely a var decl
        if self.at(TokenKind::Late) || self.at_any(&[TokenKind::Final, TokenKind::Var]) {
            return self.parse_local_var_decl();
        }
        // `const` can be either a var decl or a const expression (constructor/collection).
        //   `const Type name` or `const name =` → var decl
        //   `const Type(` / `const Type<...>(` / `const Type.` / `const [` /
        //   `const {` / `const <T>[` → expr
        // A `const` that does not begin a const expression is a var decl; when it
        // does (constructor/collection), fall through to expression parsing.
        if self.at(TokenKind::Const) && !self.const_starts_expr() {
            return self.parse_local_var_decl();
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
                p.parse_type();
                let nullable = p.kind_at(p.pos.saturating_sub(1)) == TokenKind::Qmark;
                let name = p.parse_ident()?;
                // Decide before parsing parameters as well as initializers: call
                // arguments can contain closure bodies, too.
                if nullable && p.nullable_decl_is_conditional() {
                    return None;
                }
                if p.at_any(&[TokenKind::LParen, TokenKind::Lt]) {
                    p.parse_type_params();
                    p.parse_formal_param_list();
                    let async_marker = p.parse_async_marker();
                    p.parse_function_body(async_marker)?;
                    Some(Err(NodeKind::LocalFuncDecl))
                } else if p.at_any(&[TokenKind::Eq, TokenKind::Semicolon, TokenKind::Comma]) {
                    Some(Ok(name))
                } else {
                    None
                }
            })
        {
            return match stmt {
                Ok(name) => {
                    self.parse_declarators(name);
                    self.eat(TokenKind::Semicolon);
                    NodeKind::LocalVarDecl
                }
                Err(kind) => kind,
            };
        }

        // Untyped local function: `name [<T>](params) [async] { .. }` / `=> e`
        // (no return type). A balanced lookahead confirms a function body
        // follows the parameter list before we commit — otherwise this is a
        // call statement (`foo();`, `g((x) => x);`), parsed as an expression.
        if self.at(TokenKind::Ident)
            && matches!(self.peek(1).kind, TokenKind::LParen | TokenKind::Lt)
            && self.untyped_local_func_ahead()
            && let Some(stmt) = self.try_parse_untyped_local_func()
        {
            return stmt;
        }

        self.parse_expr();
        self.eat(TokenKind::Semicolon);
        NodeKind::ExprStmt
    }

    /// Find the colon belonging to the `?` consumed as a nullable type suffix.
    /// Only this delimiter level matters; cached pairs skip nested closure bodies
    /// in constant time, so nested ambiguous statements do not rescan one another.
    fn nullable_decl_is_conditional(&mut self) -> bool {
        let mut i = self.pos;
        let mut function_head = self.at_any(&[TokenKind::LParen, TokenKind::Lt]);
        let mut questions = 0usize;
        loop {
            match self.kind_at(i) {
                TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::QmarkLBracket
                | TokenKind::LBrace => {
                    self.cache_statement_delimiters();
                    let close = self.statement_delimiters.as_ref().unwrap()[i];
                    if close == u32::MAX {
                        return false;
                    }
                    if function_head && self.kind_at(i) == TokenKind::LParen {
                        let mut body = close as usize + 1;
                        if matches!(self.kind_at(body), TokenKind::Async | TokenKind::Sync) {
                            body += 1;
                            if self.kind_at(body) == TokenKind::Star {
                                body += 1;
                            }
                        }
                        if self.kind_at(body) == TokenKind::LBrace {
                            return false;
                        }
                        function_head = false;
                    }
                    i = close as usize + 1;
                    continue;
                }
                TokenKind::Lt => {
                    let mut operand_end = i - 1;
                    while operand_end > 0 && self.kind_at(operand_end) == TokenKind::Bang {
                        operand_end -= 1;
                    }
                    if let Some(after) = self.after_generic_type_args(i)
                        && (self.after_type_args_is_call(after)
                            || self.token_ends_generic_instantiation(after)
                            || (matches!(
                                self.kind_at(after),
                                TokenKind::LBracket | TokenKind::LBrace
                            ) && !(self.kind_at(operand_end).is_identifier_like()
                                && !(self.kind_at(operand_end) == TokenKind::Await
                                    && self.ctx.body.is_async()))
                                && !matches!(
                                    self.kind_at(operand_end),
                                    TokenKind::RParen
                                        | TokenKind::RBracket
                                        | TokenKind::RBrace
                                        | TokenKind::IntLit
                                        | TokenKind::DoubleLit
                                        | TokenKind::StringLit
                                        | TokenKind::True
                                        | TokenKind::False
                                        | TokenKind::Null
                                        | TokenKind::This
                                        | TokenKind::Super
                                        | TokenKind::PlusPlus
                                        | TokenKind::MinusMinus
                                )))
                    {
                        i = after;
                        continue;
                    }
                }
                TokenKind::Is | TokenKind::As => {
                    // Use the same nullable-type disambiguation as expression parsing.
                    // This parses only a type, never an initializer or closure body.
                    let saved = self.checkpoint();
                    let function_keyword = self.last_function_keyword;
                    self.pos = i + 1;
                    self.eat(TokenKind::Bang);
                    self.with_ctx(
                        |ctx| ctx.suppress_conditional_qmark = true,
                        Self::parse_type,
                    );
                    i = self.pos;
                    self.restore(saved);
                    self.last_function_keyword = function_keyword;
                    continue;
                }
                TokenKind::Qmark => questions += 1,
                TokenKind::Colon => {
                    let Some(remaining) = questions.checked_sub(1) else {
                        return true;
                    };
                    questions = remaining;
                }
                TokenKind::Semicolon
                | TokenKind::RParen
                | TokenKind::RBracket
                | TokenKind::RBrace
                | TokenKind::Eof => return false,
                _ => {}
            }
            i += 1;
        }
    }

    fn cache_statement_delimiters(&mut self) {
        if self.statement_delimiters.is_some() {
            return;
        }
        let mut pairs = vec![u32::MAX; self.tokens.len()];
        let mut stack = Vec::with_capacity(Self::MAX_PARSE_DEPTH.min(self.tokens.len()));
        // Counts let unmatched closers be ignored in O(1). Every matched or
        // discarded opener is popped once, including malformed bracket groups.
        let mut open_counts = [0usize; 3];
        for (i, token) in self.tokens.iter_from(0).unwrap().enumerate() {
            let (delimiter, opening) = match token.kind {
                TokenKind::LParen => (0, true),
                TokenKind::LBracket | TokenKind::QmarkLBracket => (1, true),
                TokenKind::LBrace => (2, true),
                TokenKind::RParen => (0, false),
                TokenKind::RBracket => (1, false),
                TokenKind::RBrace => (2, false),
                _ => continue,
            };
            if opening {
                stack.push((i, delimiter));
                open_counts[delimiter] += 1;
            } else if open_counts[delimiter] > 0 {
                while let Some((open, expected)) = stack.pop() {
                    open_counts[expected] -= 1;
                    if expected == delimiter {
                        pairs[open] = u32::try_from(i).expect("lexer exceeds u32 token capacity");
                        break;
                    }
                }
            }
        }
        self.statement_delimiters = Some(pairs);
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

    fn parse_local_var_decl(&mut self) -> NodeKind {
        self.eat(TokenKind::Late);
        let is_const = self.eat(TokenKind::Const).is_some();
        if !is_const {
            self.eat(TokenKind::Final);
        }
        self.eat(TokenKind::Var);
        self.try_parse_type_followed_by(TokenKind::is_identifier_like);
        let name = self.expect_ident();
        self.parse_declarators(name);
        self.eat(TokenKind::Semicolon);
        NodeKind::LocalVarDecl
    }

    /// Speculatively parse a Dart 3 pattern-variable declaration
    /// `(var|final) <pattern> = expr ;`. Returns `None` (restoring position and
    /// errors) when what follows the keyword is not a destructuring pattern
    /// followed by `=` — e.g. a record-typed variable declaration.
    fn try_parse_pattern_declaration(&mut self) -> Option<NodeKind> {
        self.speculate(|p| {
            p.advance();
            p.parse_destructuring_assignment(NodeKind::PatternDeclaration)
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
    fn try_parse_pattern_assign(&mut self) -> Option<NodeKind> {
        self.speculate(|p| p.parse_destructuring_assignment(NodeKind::PatternAssignStmt))
    }

    fn parse_destructuring_assignment(&mut self, kind: NodeKind) -> Option<NodeKind> {
        let errors = self.errors.len();
        let pattern = self.parse_binding_pattern();
        if self.errors.len() > errors
            || matches!(
                pattern.kind,
                NodeKind::VariablePattern | NodeKind::WildcardPattern
            )
        {
            return None;
        }
        self.eat(TokenKind::Eq)?;
        self.parse_expr();
        self.eat(TokenKind::Semicolon);
        Some(kind)
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
    fn try_parse_untyped_local_func(&mut self) -> Option<NodeKind> {
        self.speculate(|p| {
            p.expect_ident();
            p.parse_type_params();
            if !p.at(TokenKind::LParen) {
                return None;
            }
            p.parse_formal_param_list();
            let async_marker = p.parse_async_marker();
            p.parse_function_body(async_marker)?;
            Some(NodeKind::LocalFuncDecl)
        })
    }
}
