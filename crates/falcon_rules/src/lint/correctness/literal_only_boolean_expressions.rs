//! Flags conditions built only from literals, whose value never changes.
//!
//! An `if`, `while`, `do`-`while`, or `for` condition (or a pattern `when`
//! guard) made solely of literals combined with operators always evaluates the
//! same way, so the branch is either dead or unconditional. `while (true)` is
//! allowed as the idiomatic infinite loop.
//!
//! ```dart
//! if (1 > 2) {}          // flagged
//! while (true && false) {} // flagged
//! while (true) {}        // ok
//! if (x > 2) {}          // ok
//! ```
//!
//! Adopted from the official `literal_only_boolean_expressions` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_expr, walk_stmt, walk_top_level_decl,
};

const RULE: &str = "literal-only-boolean-expressions";
const MESSAGE: &str = "The Boolean expression has a constant value.";

#[derive(Default)]
pub struct LiteralOnlyBooleanExpressions;

impl Rule for LiteralOnlyBooleanExpressions {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
            type_params: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
    /// Names of the type parameters in scope; `x is T` depends on `T`.
    type_params: Vec<String>,
}

impl Collector<'_, '_> {
    fn scoped(&mut self, params: &[TypeParam], f: impl FnOnce(&mut Self)) {
        let depth = self.type_params.len();
        self.type_params
            .extend(params.iter().map(|param| param.name.name.clone()));
        f(self);
        self.type_params.truncate(depth);
    }

    fn report_if_literal(&mut self, condition: &Expr, span: Span) {
        if self.only_literals(condition) {
            self.diags.push(self.ctx.diagnostic(RULE, MESSAGE, span));
        }
    }

    fn check_guard(&mut self, guard: Option<&Expr>) {
        if let Some(guard) = guard {
            self.report_if_literal(guard, *guard.span());
        }
    }

    fn check_if_condition(&mut self, condition: &IfCondition) {
        if let IfCondition::Case(_, _, guard) = condition {
            self.check_guard(guard.as_deref());
        }
    }

    fn check_element(&mut self, element: &CollectionElement) {
        match element {
            CollectionElement::If {
                condition,
                then_elem,
                else_elem,
                ..
            } => {
                self.check_if_condition(condition);
                self.check_element(then_elem);
                if let Some(else_elem) = else_elem {
                    self.check_element(else_elem);
                }
            }
            CollectionElement::For { element, .. } => self.check_element(element),
            CollectionElement::Expr(_)
            | CollectionElement::NullAware { .. }
            | CollectionElement::Spread { .. } => {}
        }
    }

    fn check_map_element(&mut self, element: &MapElement) {
        match element {
            MapElement::If {
                condition,
                then_entry,
                else_entry,
                ..
            } => {
                self.check_if_condition(condition);
                self.check_map_element(then_entry);
                if let Some(else_entry) = else_entry {
                    self.check_map_element(else_entry);
                }
            }
            MapElement::For { entry, .. } => self.check_map_element(entry),
            MapElement::Entry(_) | MapElement::Spread { .. } => {}
        }
    }

    /// Whether `expression` is composed only of literals, mirroring the
    /// official lint: prefix and binary operators recurse (only the left side
    /// of `??`), and `is` checks recurse unless the tested type is a type
    /// parameter. Interpolated strings have no constant value.
    fn only_literals(&self, expression: &Expr) -> bool {
        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            match expression {
                Expr::IntLit { .. }
                | Expr::DoubleLit { .. }
                | Expr::BoolLit { .. }
                | Expr::NullLit { .. }
                | Expr::SymbolLit { .. }
                | Expr::List { .. }
                | Expr::Map { .. }
                | Expr::Set { .. }
                | Expr::Record { .. } => {}
                Expr::StringLit(literal) if literal.interpolations.is_empty() => {}
                Expr::Unary { operand, .. } => pending.push(operand),
                Expr::Binary {
                    op: BinaryOp::NullCoalesce,
                    left,
                    ..
                } => pending.push(left),
                Expr::Binary { left, right, .. } => {
                    pending.push(left);
                    pending.push(right);
                }
                Expr::Is {
                    expr, dart_type, ..
                } => {
                    if self.is_type_param(dart_type) {
                        return false;
                    }
                    pending.push(expr);
                }
                _ => return false,
            }
        }
        true
    }

    fn is_type_param(&self, ty: &DartType) -> bool {
        matches!(ty, DartType::Named(named)
            if matches!(named.segments.as_slice(), [segment]
                if self.type_params.contains(&segment.name)))
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        let params: &[TypeParam] = match node {
            TopLevelDecl::Class(decl) => &decl.type_params,
            TopLevelDecl::Mixin(decl) => &decl.type_params,
            TopLevelDecl::MixinClass(decl) => &decl.type_params,
            TopLevelDecl::Enum(decl) => &decl.type_params,
            TopLevelDecl::Extension(decl) => &decl.type_params,
            TopLevelDecl::ExtensionType(decl) => &decl.type_params,
            TopLevelDecl::Function(decl) => &decl.type_params,
            TopLevelDecl::ClassTypeAlias(_)
            | TopLevelDecl::Variable(_)
            | TopLevelDecl::TypeAlias(_)
            | TopLevelDecl::Error(_) => &[],
        };
        self.scoped(params, |this| walk_top_level_decl(this, node));
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        let params: &[TypeParam] = match node {
            ClassMember::Method(method) => &method.type_params,
            _ => &[],
        };
        self.scoped(params, |this| walk_class_member(this, node));
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::If(statement) => match &statement.condition {
                IfCondition::Expr(condition) => self.report_if_literal(condition, statement.span),
                IfCondition::Case(..) => self.check_if_condition(&statement.condition),
            },
            Stmt::While(statement) => {
                if !matches!(statement.condition, Expr::BoolLit { value: true, .. }) {
                    self.report_if_literal(&statement.condition, statement.span);
                }
            }
            Stmt::DoWhile(statement) => {
                self.report_if_literal(&statement.condition, statement.span);
            }
            Stmt::For(statement) => {
                let for_parts = !matches!(
                    statement.header.init,
                    Some(ForInit::ForIn { .. } | ForInit::PatternForIn { .. })
                );
                if for_parts && let Some(condition) = &statement.header.condition {
                    self.report_if_literal(condition, statement.span);
                }
            }
            Stmt::Switch(statement) => {
                for case in &statement.cases {
                    for kind in &case.cases {
                        if let SwitchCaseKind::Pattern(_, guard) = kind {
                            self.check_guard(guard.as_deref());
                        }
                    }
                }
            }
            _ => {}
        }
        if let Stmt::LocalFunc(function) = node {
            self.scoped(&function.type_params, |this| walk_stmt(this, node));
        } else {
            walk_stmt(self, node);
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Switch { arms, .. } => {
                for arm in arms {
                    self.check_guard(arm.guard.as_ref());
                }
            }
            Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                for element in elements {
                    self.check_element(element);
                }
            }
            Expr::Map { elements, .. } => {
                for element in elements {
                    self.check_map_element(element);
                }
            }
            _ => {}
        }
        if let Expr::FuncExpr { type_params, .. } = node {
            self.scoped(type_params, |this| walk_expr(this, node));
        } else {
            walk_expr(self, node);
        }
    }
}
