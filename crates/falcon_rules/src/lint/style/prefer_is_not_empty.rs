//! Flags `.length` comparisons equivalent to a `.isNotEmpty` check.
//!
//! Collections and strings expose an `isNotEmpty` getter that reads as plain
//! intent and, for lazy iterables, can avoid computing the full `length`.
//! Comparing `length` against a constant makes the reader infer the meaning, so
//! `list.isNotEmpty` should replace forms such as `length != 0`, `length > 0`,
//! and `length >= 1`, along with their operand-swapped mirrors (`0 != length`,
//! `0 < length`, `1 <= length`). Adopted from package:lints
//! `prefer_is_not_empty`.
//!
//! Only a plain `.length` property access matches against the relevant integer
//! literal. A null-aware `receiver?.length` is deliberately left alone: that
//! comparison also has to account for a `null` receiver, so `isNotEmpty` is not
//! a drop-in replacement.
//!
//! Type knowledge only ever *suppresses*. When a [`TypeIndex`] is on the context
//! and the receiver's static type is *positively proven* to be a concrete type
//! that (a) definitely has no `isNotEmpty` member (`member_lookup …
//! ProvenAbsent`) and (b) is definitely not a core collection/string
//! (`is_subtype … ProvenNo` for `Iterable`, `String`, `Map`), suggesting
//! `isNotEmpty` would be wrong, so the diagnostic is withheld. An `Unknown`
//! receiver — the common case, and every receiver when no type index is
//! attached — keeps firing.

use falcon_analyze::{
    AnalyzeContext, LocalTypes, MemberResult, ReceiverTypes, Rule, StaticType, SubtypeResult,
    TypeIndex,
};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_decl, walk_enum_decl, walk_expr, walk_mixin_decl, walk_program, walk_stmt,
};

pub struct PreferIsNotEmpty;

impl Rule for PreferIsNotEmpty {
    fn name(&self) -> &'static str {
        "prefer-is-not-empty"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            lt: LocalTypes::new(),
            types: ctx.types,
            enclosing: None,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    file: String,
    lt: LocalTypes,
    types: Option<&'a TypeIndex>,
    enclosing: Option<String>,
}

impl Collector<'_> {
    /// Whether the diagnostic on `receiver.length … 0` should be *suppressed*
    /// because `receiver`'s type is positively proven to lack `isNotEmpty` and to
    /// be no core collection/string. A lost or `Unknown` fact never suppresses.
    fn suppressed(&self, receiver: &Expr) -> bool {
        let Some(types) = self.types else {
            return false;
        };
        let rt = ReceiverTypes::new(&self.lt, self.types, self.enclosing.as_deref());
        let StaticType::Other { name, .. } = rt.of_expr(receiver) else {
            return false;
        };
        matches!(
            types.member_lookup(&name, "isNotEmpty"),
            MemberResult::ProvenAbsent
        ) && matches!(types.is_subtype(&name, "Iterable"), SubtypeResult::ProvenNo)
            && matches!(types.is_subtype(&name, "String"), SubtypeResult::ProvenNo)
            && matches!(types.is_subtype(&name, "Map"), SubtypeResult::ProvenNo)
    }

    /// Walk a function/method/getter/setter/closure body whose signature bindings
    /// already live in the current (innermost) scope.
    fn walk_body(&mut self, body: &FunctionBody) {
        match body {
            FunctionBody::Block(b) => {
                for s in &b.stmts {
                    self.visit_stmt(s);
                }
            }
            FunctionBody::Arrow(e, _) => self.visit_expr(e),
            FunctionBody::Native(_, _) => {}
        }
    }
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_class_decl(&mut self, node: &ClassDecl) {
        let saved = self.enclosing.replace(node.name.name.clone());
        walk_class_decl(self, node);
        self.enclosing = saved;
    }

    fn visit_mixin_decl(&mut self, node: &MixinDecl) {
        let saved = self.enclosing.replace(node.name.name.clone());
        walk_mixin_decl(self, node);
        self.enclosing = saved;
    }

    fn visit_enum_decl(&mut self, node: &EnumDecl) {
        let saved = self.enclosing.replace(node.name.name.clone());
        walk_enum_decl(self, node);
        self.enclosing = saved;
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        let saved = std::mem::replace(&mut self.lt, LocalTypes::new());
        self.lt.bind_params(&node.params);
        if let Some(body) = &node.body {
            self.walk_body(body);
        }
        self.lt = saved;
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        let saved = std::mem::replace(&mut self.lt, LocalTypes::new());
        self.lt.bind_params(&node.params);
        if let Some(body) = &node.body {
            self.walk_body(body);
        }
        self.lt = saved;
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        let saved = std::mem::replace(&mut self.lt, LocalTypes::new());
        self.lt.bind_params(&node.params);
        if let Some(body) = &node.body {
            self.walk_body(body);
        }
        self.lt = saved;
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        let saved = std::mem::replace(&mut self.lt, LocalTypes::new());
        if let Some(body) = &node.body {
            self.walk_body(body);
        }
        self.lt = saved;
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        let saved = std::mem::replace(&mut self.lt, LocalTypes::new());
        let ty = node
            .param_type
            .as_ref()
            .map(StaticType::from_dart_type)
            .unwrap_or(StaticType::Unknown);
        self.lt.declare(node.param.name.clone(), ty);
        if let Some(body) = &node.body {
            self.walk_body(body);
        }
        self.lt = saved;
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::LocalVar(lv) => {
                for d in &lv.declarators {
                    if let Some(init) = &d.initializer {
                        self.visit_expr(init);
                    }
                }
                self.lt.declare_local(lv);
            }
            Stmt::PatternDecl(pd) => {
                self.visit_expr(&pd.init);
                self.lt.bind_pattern(&pd.pattern);
            }
            Stmt::Block(b) => {
                self.lt.push_scope();
                for s in &b.stmts {
                    self.visit_stmt(s);
                }
                self.lt.pop_scope();
            }
            Stmt::For(f) => {
                self.lt.push_scope();
                if let Some(init) = &f.init {
                    if let ForInit::VarDecl(lv) = init {
                        for d in &lv.declarators {
                            if let Some(e) = &d.initializer {
                                self.visit_expr(e);
                            }
                        }
                    }
                    self.lt.bind_for_init(init);
                }
                if let Some(cond) = &f.condition {
                    self.visit_expr(cond);
                }
                for u in &f.update {
                    self.visit_expr(u);
                }
                self.visit_stmt(&f.body);
                self.lt.pop_scope();
            }
            Stmt::TryCatch(tc) => {
                self.lt.push_scope();
                for s in &tc.body.stmts {
                    self.visit_stmt(s);
                }
                self.lt.pop_scope();
                for catch in &tc.catches {
                    self.lt.push_scope();
                    self.lt.bind_catch(catch);
                    for s in &catch.body.stmts {
                        self.visit_stmt(s);
                    }
                    self.lt.pop_scope();
                }
                if let Some(fin) = &tc.finally {
                    self.lt.push_scope();
                    for s in &fin.stmts {
                        self.visit_stmt(s);
                    }
                    self.lt.pop_scope();
                }
            }
            _ => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Binary {
                op,
                left,
                right,
                span,
            } => {
                if let Some(receiver) = not_empty_comparison_receiver(op, left, right)
                    && !self.suppressed(receiver)
                {
                    self.diags.push(Diagnostic::new(
                        "prefer-is-not-empty",
                        Severity::Warning,
                        "Use 'isNotEmpty' instead of comparing 'length' to 0.",
                        self.file.clone(),
                        DiagSpan {
                            start: span.start,
                            end: span.end,
                        },
                    ));
                }
                walk_expr(self, node);
            }
            Expr::FuncExpr { params, body, .. } => {
                self.lt.push_scope();
                self.lt.bind_params(params);
                self.walk_body(body);
                self.lt.pop_scope();
            }
            Expr::Assign { target, value, .. } => {
                self.visit_expr(target);
                self.visit_expr(value);
                if let Expr::Ident(id) = target.as_ref() {
                    let ty = self.lt.of_expr(value);
                    self.lt.reassign(&id.name, ty);
                }
            }
            _ => walk_expr(self, node),
        }
    }
}

/// If this binary is a `.length` comparison equivalent to `isNotEmpty`, the
/// receiver of the `.length` access (its object); otherwise `None`.
fn not_empty_comparison_receiver<'a>(
    op: &BinaryOp,
    left: &'a Expr,
    right: &'a Expr,
) -> Option<&'a Expr> {
    match op {
        // length != 0 / 0 != length
        BinaryOp::NotEq => length_receiver(left)
            .filter(|_| is_int(right, 0))
            .or_else(|| length_receiver(right).filter(|_| is_int(left, 0))),
        // length > 0
        BinaryOp::Gt => length_receiver(left).filter(|_| is_int(right, 0)),
        // 0 < length
        BinaryOp::Lt => length_receiver(right).filter(|_| is_int(left, 0)),
        // length >= 1
        BinaryOp::GtEq => length_receiver(left).filter(|_| is_int(right, 1)),
        // 1 <= length
        BinaryOp::LtEq => length_receiver(right).filter(|_| is_int(left, 1)),
        _ => None,
    }
}

/// The receiver object of a `.length` property access, if `expr` is one.
fn length_receiver(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::Field {
            object,
            field,
            is_null_safe: false,
            ..
        } if field.name == "length" => Some(object),
        _ => None,
    }
}

/// True for an integer literal equal to `n`.
fn is_int(expr: &Expr, n: i64) -> bool {
    if let Expr::IntLit { value, .. } = expr {
        value.replace('_', "").parse::<i64>().ok() == Some(n)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    /// Diagnostic count with no type index (baseline fire-on-unknown behavior).
    fn count_no_types(source: &str) -> usize {
        let program = parse(source).0;
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, source, &config);
        PreferIsNotEmpty.analyze(&program, &ctx).len()
    }

    /// Diagnostic count with a single-file `TypeIndex` attached (production shape
    /// for a resolver-dependent rule in degraded single-file mode).
    fn count_with_types(source: &str) -> usize {
        let program = parse(source).0;
        let types = TypeIndex::from_program(&program);
        let config = FalconConfig::default();
        let path = PathBuf::from("t.dart");
        let ctx = AnalyzeContext::new(&path, source, &config).with_types(&types);
        PreferIsNotEmpty.analyze(&program, &ctx).len()
    }

    #[test]
    fn unknown_receiver_fires_with_and_without_types() {
        let src = "void f() { if (list.length != 0) return; }";
        assert_eq!(count_no_types(src), 1);
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn proven_non_collection_suppresses_only_with_types() {
        let src = "class Ruler { int get length => 3; } \
                   void f(Ruler r) { if (r.length != 0) return; }";
        assert_eq!(count_no_types(src), 1, "no type index → fire (baseline)");
        assert_eq!(count_with_types(src), 0, "proven no isNotEmpty → suppress");
    }

    #[test]
    fn class_implementing_iterable_still_fires() {
        let src = "class Bag implements Iterable<int> { int get length => 0; } \
                   void f(Bag b) { if (b.length != 0) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn supertype_leaving_project_still_fires() {
        let src = "class Offsite extends Frobnicator { int get length => 0; } \
                   void f(Offsite o) { if (o.length != 0) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn string_receiver_still_fires() {
        let src = "void f(String s) { if (s.length != 0) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn all_mirrored_forms_detected() {
        let src = "void f() { \
            if (a.length != 0) return; \
            if (0 != b.length) return; \
            if (c.length > 0) return; \
            if (0 < d.length) return; \
            if (e.length >= 1) return; \
            if (1 <= g.length) return; }";
        assert_eq!(count_no_types(src), 6);
    }
}
