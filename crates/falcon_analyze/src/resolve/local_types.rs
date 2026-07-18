//! File-local, per-function-body scope tracker.
//!
//! [`LocalTypes`] models the lexical scopes of a single function/method/getter/
//! constructor body as a stack of name→[`StaticType`] maps. A rule drives it
//! while descending the AST: push a scope on entering a block, declare bindings
//! as it meets them, note reassignments, and query [`LocalTypes::of_expr`] at
//! any point. Identifier lookup walks the stack innermost-first, so shadowing
//! works naturally.
//!
//! Soundness over precision. The tracker only records a concrete type when it is
//! certain (declared type, or a trivially-inferable initializer); anything else
//! is [`StaticType::Unknown`]. A reassignment whose new type differs from the
//! recorded one degrades the binding to `Unknown` rather than guess.
//!
//! ## What is and isn't tracked
//!
//! * Bound: signature parameters (including `this.x` field/`super.x` formals when
//!   they carry a written type), local `var`/typed declarations, for-loop
//!   variables, catch clause parameters, and simple Dart 3 pattern variable
//!   bindings ([`Pattern::Variable`]) that carry a written type.
//! * Not bound (left `Unknown`): destructured record/list/map/object pattern
//!   fields without a written type, closure captures reassigned across scopes,
//!   and field formals without a written type (resolving them needs the class's
//!   field type, which is out of this layer's file-local, non-lookup budget).

use std::collections::HashMap;

use falcon_syntax::ast::*;

use super::StaticType;

/// A stack of lexical scopes mapping local names to their coarse static type.
#[derive(Debug, Clone)]
pub struct LocalTypes {
    scopes: Vec<HashMap<String, StaticType>>,
}

impl Default for LocalTypes {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalTypes {
    /// A tracker with a single (root) scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    /// Enter a nested lexical scope.
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Leave the innermost scope. The root scope is never popped.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Declare (or shadow, in the current scope) a binding with a known type.
    pub fn declare(&mut self, name: impl Into<String>, ty: StaticType) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.into(), ty);
        }
    }

    /// Look up a name through the scope stack (innermost first). Returns
    /// [`StaticType::Unknown`] when the name is not a tracked local — it may be
    /// a field, a top-level, or simply unbound; the resolver never guesses.
    pub fn lookup(&self, name: &str) -> StaticType {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return ty.clone();
            }
        }
        StaticType::Unknown
    }

    /// Record an assignment to `name`. If `name` is a tracked local whose new
    /// type does not match the recorded one, the binding degrades to
    /// [`StaticType::Unknown`] (we can no longer state its type soundly).
    /// Assignments to unknown names (fields, globals) are ignored.
    pub fn reassign(&mut self, name: &str, ty: StaticType) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(existing) = scope.get_mut(name) {
                if *existing != ty {
                    *existing = StaticType::Unknown;
                }
                return;
            }
        }
    }

    // ── Convenience binders (single-pass AST walking) ──────────────────────────

    /// Bind every parameter of a signature into the current scope. A parameter's
    /// type comes from its written type; untyped parameters (including field/
    /// super formals without a written type) bind as [`StaticType::Unknown`].
    pub fn bind_params(&mut self, params: &FormalParamList) {
        for p in params
            .positional
            .iter()
            .chain(&params.optional_positional)
            .chain(&params.named)
        {
            let ty = p
                .param_type
                .as_ref()
                .map(StaticType::from_dart_type)
                .unwrap_or(StaticType::Unknown);
            self.declare(p.name.name.clone(), ty);
        }
    }

    /// Declare the variables of a `var`/typed local declaration. Each declarator
    /// takes the written type if present, else the inferred initializer type,
    /// else [`StaticType::Unknown`].
    pub fn declare_local(&mut self, decl: &LocalVarDecl) {
        let declared = decl.var_type.as_ref().map(StaticType::from_dart_type);
        for d in &decl.declarators {
            let ty = match &declared {
                Some(t) => t.clone(),
                None => d
                    .initializer
                    .as_ref()
                    .map(|init| self.of_expr(init))
                    .unwrap_or(StaticType::Unknown),
            };
            self.declare(d.name.name.clone(), ty);
        }
    }

    /// Bind a `for`-loop initializer's variables (both C-style `var i = 0` and
    /// `for-in` loop variables).
    pub fn bind_for_init(&mut self, init: &ForInit) {
        match init {
            ForInit::VarDecl(decl) => self.declare_local(decl),
            ForInit::ForIn { var_type, name, .. } => {
                let ty = var_type
                    .as_ref()
                    .map(StaticType::from_dart_type)
                    .unwrap_or(StaticType::Unknown);
                self.declare(name.name.clone(), ty);
            }
            // Pattern for-in / expression init: bind what is cheap (typed pattern
            // variables); the rest stays Unknown.
            ForInit::PatternForIn { pattern, .. } => self.bind_pattern(pattern),
            ForInit::Exprs(_) => {}
        }
    }

    /// Bind a catch clause's exception and stack-trace variables. The exception
    /// takes its written `on` type if present, else `Unknown`; the stack trace is
    /// always `Unknown` (its type — `StackTrace` — is irrelevant to consumers).
    pub fn bind_catch(&mut self, catch: &CatchClause) {
        if let Some(var) = &catch.exception_var {
            let ty = catch
                .exception_type
                .as_ref()
                .map(StaticType::from_dart_type)
                .unwrap_or(StaticType::Unknown);
            self.declare(var.name.clone(), ty);
        }
        if let Some(st) = &catch.stack_trace_var {
            self.declare(st.name.clone(), StaticType::Unknown);
        }
    }

    /// Bind the variables of a Dart 3 pattern. Only [`Pattern::Variable`] with a
    /// written type contributes a concrete type; every other bound variable is
    /// recorded as [`StaticType::Unknown`] (destructured element types are not
    /// inferred in this minimal layer). Recurses through composite patterns.
    pub fn bind_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Variable { type_, name, .. } => {
                let ty = type_
                    .as_ref()
                    .map(StaticType::from_dart_type)
                    .unwrap_or(StaticType::Unknown);
                self.declare(name.name.clone(), ty);
            }
            Pattern::List(l) => {
                for el in &l.elements {
                    match el {
                        ListPatternElement::Pattern(p) => self.bind_pattern(p),
                        ListPatternElement::Rest(Some(p), _) => self.bind_pattern(p),
                        ListPatternElement::Rest(None, _) => {}
                    }
                }
            }
            Pattern::Record(r) => {
                for f in &r.fields {
                    self.bind_pattern(&f.pattern);
                }
            }
            Pattern::Map(m) => {
                for e in &m.entries {
                    self.bind_pattern(&e.pattern);
                }
            }
            Pattern::Object(o) => {
                for f in &o.fields {
                    if let Some(p) = &f.pattern {
                        self.bind_pattern(p);
                    }
                }
            }
            Pattern::LogicalAnd { left, right, .. } | Pattern::LogicalOr { left, right, .. } => {
                self.bind_pattern(left);
                self.bind_pattern(right);
            }
            Pattern::Cast { inner, .. }
            | Pattern::NullCheck { inner, .. }
            | Pattern::NullAssert { inner, .. }
            | Pattern::ParenPattern { inner, .. } => self.bind_pattern(inner),
            _ => {}
        }
    }

    // ── Expression inference ───────────────────────────────────────────────────

    /// Infer the coarse static type of an expression.
    ///
    /// Handles literals, `is`/`is!` tests, comparison and logical operators,
    /// `!`, arithmetic over numeric operands, `as` casts, `!` null-assertions,
    /// collection literals, `new`/constructor calls, and identifier resolution
    /// through the scope stack. Everything else — and any case where certainty
    /// is lost — returns [`StaticType::Unknown`].
    pub fn of_expr(&self, expr: &Expr) -> StaticType {
        match expr {
            Expr::IntLit { .. } => StaticType::Int { nullable: false },
            Expr::DoubleLit { .. } => StaticType::Double { nullable: false },
            Expr::StringLit(_) => StaticType::String { nullable: false },
            Expr::BoolLit { .. } => StaticType::Bool { nullable: false },
            Expr::NullLit { .. } => StaticType::Unknown,
            Expr::Ident(id) => self.lookup(&id.name),

            Expr::Is { .. } => StaticType::Bool { nullable: false },

            Expr::Unary { op, operand, .. } => match op {
                UnaryOp::Bang => StaticType::Bool { nullable: false },
                UnaryOp::Minus | UnaryOp::Tilde | UnaryOp::PlusPlus | UnaryOp::MinusMinus => {
                    numeric_or_unknown(self.of_expr(operand))
                }
            },
            Expr::PostfixIncDec { operand, .. } => numeric_or_unknown(self.of_expr(operand)),

            Expr::Binary {
                op, left, right, ..
            } => match op {
                BinaryOp::EqEq
                | BinaryOp::NotEq
                | BinaryOp::Lt
                | BinaryOp::Gt
                | BinaryOp::LtEq
                | BinaryOp::GtEq
                | BinaryOp::And
                | BinaryOp::Or => StaticType::Bool { nullable: false },
                // `String + String` is concatenation, yielding a non-nullable
                // `String`; otherwise `+` is numeric.
                BinaryOp::Add => {
                    let l = self.of_expr(left);
                    let r = self.of_expr(right);
                    if matches!(l, StaticType::String { nullable: false })
                        && matches!(r, StaticType::String { nullable: false })
                    {
                        StaticType::String { nullable: false }
                    } else {
                        numeric_result(l, r)
                    }
                }
                BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::Mod
                | BinaryOp::IntDiv
                | BinaryOp::BitAnd
                | BinaryOp::BitOr
                | BinaryOp::BitXor
                | BinaryOp::Shl
                | BinaryOp::Shr
                | BinaryOp::UShr => numeric_result(self.of_expr(left), self.of_expr(right)),
                // `/` on num always yields double in Dart.
                BinaryOp::Div => StaticType::Double { nullable: false },
                // `a ?? b` is only sound to type when both sides agree; keep it
                // conservative rather than model the null-elimination.
                BinaryOp::NullCoalesce | BinaryOp::IfNull => StaticType::Unknown,
            },

            Expr::As { dart_type, .. } => StaticType::from_dart_type(dart_type),

            Expr::NullAssert { operand, .. } => self.of_expr(operand).with_nullable(false),

            Expr::Conditional {
                then_expr,
                else_expr,
                ..
            } => {
                let t = self.of_expr(then_expr);
                let e = self.of_expr(else_expr);
                if t == e { t } else { StaticType::Unknown }
            }

            Expr::List { .. } => StaticType::Other {
                name: "List".to_string(),
                nullable: false,
            },
            Expr::Set { .. } => StaticType::Other {
                name: "Set".to_string(),
                nullable: false,
            },
            Expr::Map { .. } => StaticType::Other {
                name: "Map".to_string(),
                nullable: false,
            },

            Expr::New { dart_type, .. } => {
                // A constructor invocation always yields a non-null instance.
                StaticType::from_dart_type(dart_type).with_nullable(false)
            }

            _ => StaticType::Unknown,
        }
    }
}

/// Keep a numeric type (int/double/num) from a unary numeric operator; anything
/// else becomes `Unknown`.
fn numeric_or_unknown(ty: StaticType) -> StaticType {
    match ty {
        StaticType::Int { .. } => StaticType::Int { nullable: false },
        StaticType::Double { .. } => StaticType::Double { nullable: false },
        StaticType::Num { .. } => StaticType::Num { nullable: false },
        _ => StaticType::Unknown,
    }
}

/// Coarse numeric join for a binary arithmetic/bitwise operator: `int op int`
/// stays `int`, any `double` operand yields `double`, otherwise `num` when both
/// are numeric, else `Unknown`.
fn numeric_result(l: StaticType, r: StaticType) -> StaticType {
    let l_num = matches!(
        l,
        StaticType::Int { .. } | StaticType::Double { .. } | StaticType::Num { .. }
    );
    let r_num = matches!(
        r,
        StaticType::Int { .. } | StaticType::Double { .. } | StaticType::Num { .. }
    );
    if !l_num || !r_num {
        return StaticType::Unknown;
    }
    let any_double =
        matches!(l, StaticType::Double { .. }) || matches!(r, StaticType::Double { .. });
    let both_int = matches!(l, StaticType::Int { .. }) && matches!(r, StaticType::Int { .. });
    if any_double {
        StaticType::Double { nullable: false }
    } else if both_int {
        StaticType::Int { nullable: false }
    } else {
        StaticType::Num { nullable: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_dart_parser::parse;

    /// Parse `src` and return the body of its first top-level function.
    fn first_fn_body(src: &str) -> FunctionBody {
        let (program, errors) = parse(src);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        for decl in program.declarations {
            if let TopLevelDecl::Function(f) = decl {
                return f.body.expect("function has a body");
            }
        }
        panic!("no top-level function found in: {src}");
    }

    /// The statements of a block-bodied first top-level function.
    fn first_fn_stmts(src: &str) -> Vec<Stmt> {
        match first_fn_body(src) {
            FunctionBody::Block(b) => b.stmts,
            other => panic!("expected block body, got {other:?}"),
        }
    }

    fn bool_t() -> StaticType {
        StaticType::Bool { nullable: false }
    }

    #[test]
    fn literals_infer_core_types() {
        let lt = LocalTypes::new();
        let cases = [
            ("true", StaticType::Bool { nullable: false }),
            ("42", StaticType::Int { nullable: false }),
            ("3.14", StaticType::Double { nullable: false }),
            ("'hi'", StaticType::String { nullable: false }),
        ];
        for (expr_src, expected) in cases {
            let stmts = first_fn_stmts(&format!("void f() {{ var x = {expr_src}; }}"));
            let init = match &stmts[0] {
                Stmt::LocalVar(lv) => lv.declarators[0].initializer.as_ref().unwrap(),
                other => panic!("expected local var, got {other:?}"),
            };
            assert_eq!(lt.of_expr(init), expected, "for `{expr_src}`");
        }
    }

    #[test]
    fn comparisons_and_logical_are_bool() {
        let lt = LocalTypes::new();
        for expr_src in [
            "1 == 2",
            "a < b",
            "a && b",
            "!a",
            "x is int",
            "x is! String",
        ] {
            let stmts = first_fn_stmts(&format!("void f() {{ var x = {expr_src}; }}"));
            let init = match &stmts[0] {
                Stmt::LocalVar(lv) => lv.declarators[0].initializer.as_ref().unwrap(),
                other => panic!("expected local var, got {other:?}"),
            };
            assert_eq!(lt.of_expr(init), bool_t(), "for `{expr_src}`");
        }
    }

    #[test]
    fn declared_bool_param_resolves_through_scope() {
        // `bool ok` param → of_expr(ok) is non-nullable bool; `bool? maybe` → nullable.
        let (program, errors) = parse("void f(bool ok, bool? maybe) {}");
        assert!(errors.is_empty());
        let TopLevelDecl::Function(f) = &program.declarations[0] else {
            panic!()
        };
        let mut lt = LocalTypes::new();
        lt.bind_params(&f.params);
        assert_eq!(lt.lookup("ok"), StaticType::Bool { nullable: false });
        assert_eq!(lt.lookup("maybe"), StaticType::Bool { nullable: true });
        assert!(lt.lookup("ok").is_non_nullable_bool());
        assert!(!lt.lookup("maybe").is_non_nullable_bool());
    }

    #[test]
    fn local_var_declared_and_inferred() {
        let stmts = first_fn_stmts("void f() { bool a = g(); final b = 1 == 2; String s = h(); }");
        let mut lt = LocalTypes::new();
        for s in &stmts {
            if let Stmt::LocalVar(lv) = s {
                lt.declare_local(lv);
            }
        }
        // Declared type wins even over an uninferable initializer.
        assert_eq!(lt.lookup("a"), StaticType::Bool { nullable: false });
        // Inferred from `1 == 2`.
        assert_eq!(lt.lookup("b"), StaticType::Bool { nullable: false });
        assert_eq!(lt.lookup("s"), StaticType::String { nullable: false });
    }

    #[test]
    fn unknown_initializer_falls_back_to_unknown() {
        let stmts = first_fn_stmts("void f() { var x = someCall(); }");
        let mut lt = LocalTypes::new();
        if let Stmt::LocalVar(lv) = &stmts[0] {
            lt.declare_local(lv);
        }
        assert_eq!(lt.lookup("x"), StaticType::Unknown);
    }

    #[test]
    fn shadowing_respects_scope_stack() {
        let mut lt = LocalTypes::new();
        lt.declare("x", StaticType::Bool { nullable: false });
        assert_eq!(lt.lookup("x"), StaticType::Bool { nullable: false });
        lt.push_scope();
        lt.declare("x", StaticType::String { nullable: false });
        assert_eq!(lt.lookup("x"), StaticType::String { nullable: false });
        lt.pop_scope();
        // Outer binding is visible again after leaving the nested scope.
        assert_eq!(lt.lookup("x"), StaticType::Bool { nullable: false });
    }

    #[test]
    fn reassignment_with_different_type_degrades_to_unknown() {
        let mut lt = LocalTypes::new();
        lt.declare("x", StaticType::Bool { nullable: false });
        lt.reassign("x", StaticType::Int { nullable: false });
        assert_eq!(lt.lookup("x"), StaticType::Unknown);
    }

    #[test]
    fn reassignment_with_same_type_is_preserved() {
        let mut lt = LocalTypes::new();
        lt.declare("x", StaticType::Bool { nullable: false });
        lt.reassign("x", StaticType::Bool { nullable: false });
        assert_eq!(lt.lookup("x"), StaticType::Bool { nullable: false });
    }

    #[test]
    fn reassignment_to_unknown_name_is_ignored() {
        let mut lt = LocalTypes::new();
        lt.reassign("field", StaticType::Bool { nullable: false });
        assert_eq!(lt.lookup("field"), StaticType::Unknown);
    }

    #[test]
    fn unbound_identifier_is_unknown() {
        let lt = LocalTypes::new();
        let stmts = first_fn_stmts("void f() { var x = unknownThing; }");
        if let Stmt::LocalVar(lv) = &stmts[0] {
            let init = lv.declarators[0].initializer.as_ref().unwrap();
            assert_eq!(lt.of_expr(init), StaticType::Unknown);
        }
    }

    #[test]
    fn as_cast_and_null_assert() {
        // `x as bool` → non-null bool; `(x as bool?)!` → non-null bool.
        let stmts = first_fn_stmts("void f() { var a = x as bool; var b = (y as bool?)!; }");
        let lt = LocalTypes::new();
        let inits: Vec<&Expr> = stmts
            .iter()
            .filter_map(|s| match s {
                Stmt::LocalVar(lv) => lv.declarators[0].initializer.as_ref(),
                _ => None,
            })
            .collect();
        assert_eq!(lt.of_expr(inits[0]), StaticType::Bool { nullable: false });
        assert_eq!(lt.of_expr(inits[1]), StaticType::Bool { nullable: false });
    }

    #[test]
    fn arithmetic_numeric_inference() {
        let stmts =
            first_fn_stmts("void f() { int a = 1; int b = 2; var c = a + b; var d = a / b; }");
        let mut lt = LocalTypes::new();
        for s in &stmts {
            if let Stmt::LocalVar(lv) = s {
                lt.declare_local(lv);
            }
        }
        assert_eq!(lt.lookup("c"), StaticType::Int { nullable: false });
        // `/` always yields double.
        assert_eq!(lt.lookup("d"), StaticType::Double { nullable: false });
    }

    #[test]
    fn string_concatenation_is_string() {
        // `String + String` is concatenation → non-nullable String; a `String + int`
        // mismatch is not provable and falls back to Unknown.
        let stmts = first_fn_stmts(
            "void f(String a, int n) { var c = a + a; var d = a + n; var e = 'x' + 'y'; }",
        );
        let mut lt = LocalTypes::new();
        let (program, _) = parse("void f(String a, int n) {}");
        let TopLevelDecl::Function(func) = &program.declarations[0] else {
            panic!()
        };
        lt.bind_params(&func.params);
        for s in &stmts {
            if let Stmt::LocalVar(lv) = s {
                lt.declare_local(lv);
            }
        }
        assert_eq!(lt.lookup("c"), StaticType::String { nullable: false });
        assert_eq!(lt.lookup("d"), StaticType::Unknown);
        assert_eq!(lt.lookup("e"), StaticType::String { nullable: false });
    }

    #[test]
    fn collection_literals_and_new_are_other_nonnull() {
        let stmts = first_fn_stmts(
            "void f() { var a = [1, 2]; var b = {1, 2}; var c = {'k': 1}; var d = Foo(); }",
        );
        let mut lt = LocalTypes::new();
        for s in &stmts {
            if let Stmt::LocalVar(lv) = s {
                lt.declare_local(lv);
            }
        }
        assert_eq!(
            lt.lookup("a"),
            StaticType::Other {
                name: "List".into(),
                nullable: false
            }
        );
        assert_eq!(
            lt.lookup("b"),
            StaticType::Other {
                name: "Set".into(),
                nullable: false
            }
        );
        assert_eq!(
            lt.lookup("c"),
            StaticType::Other {
                name: "Map".into(),
                nullable: false
            }
        );
        // `Foo()` parses as a call; only `new Foo()` guarantees a constructor.
        let _ = lt.lookup("d");
    }

    #[test]
    fn conditional_same_branch_types() {
        let stmts = first_fn_stmts("void f() { var a = c ? true : false; var b = c ? 1 : 'x'; }");
        let lt = LocalTypes::new();
        let inits: Vec<&Expr> = stmts
            .iter()
            .filter_map(|s| match s {
                Stmt::LocalVar(lv) => lv.declarators[0].initializer.as_ref(),
                _ => None,
            })
            .collect();
        assert_eq!(lt.of_expr(inits[0]), StaticType::Bool { nullable: false });
        // Divergent branch types → Unknown.
        assert_eq!(lt.of_expr(inits[1]), StaticType::Unknown);
    }

    #[test]
    fn catch_and_pattern_bindings() {
        // Typed catch var resolves; pattern variable with a written type resolves.
        let (program, errors) = parse(
            "void f() { try {} on FormatException catch (e, st) {} final (bool flag,) = r; }",
        );
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Function(func) = &program.declarations[0] else {
            panic!()
        };
        let FunctionBody::Block(block) = func.body.as_ref().unwrap() else {
            panic!()
        };
        let mut lt = LocalTypes::new();
        for s in &block.stmts {
            match s {
                Stmt::TryCatch(tc) => {
                    for c in &tc.catches {
                        lt.bind_catch(c);
                    }
                }
                Stmt::PatternDecl(pd) => lt.bind_pattern(&pd.pattern),
                _ => {}
            }
        }
        assert_eq!(
            lt.lookup("e"),
            StaticType::Other {
                name: "FormatException".into(),
                nullable: false
            }
        );
        assert_eq!(lt.lookup("st"), StaticType::Unknown);
        assert_eq!(lt.lookup("flag"), StaticType::Bool { nullable: false });
    }

    #[test]
    fn no_panic_on_empty_or_malformed() {
        let lt = LocalTypes::new();
        // Error expression → Unknown, no panic.
        assert_eq!(
            lt.of_expr(&Expr::Error {
                span: Span::default()
            }),
            StaticType::Unknown
        );
    }
}
