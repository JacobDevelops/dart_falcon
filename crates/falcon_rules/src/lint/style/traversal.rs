use falcon_syntax::ast::{Expr, Program, Stmt};
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

pub(super) fn for_each_expr(program: &Program, f: impl FnMut(&Expr)) {
    struct Expressions<F>(F);
    impl<F: FnMut(&Expr)> Visitor for Expressions<F> {
        fn visit_expr(&mut self, node: &Expr) {
            (self.0)(node);
            walk_expr(self, node);
        }
    }
    Expressions(f).visit_program(program);
}

pub(super) fn for_each_stmt(program: &Program, f: impl FnMut(&Stmt)) {
    struct Statements<F>(F);
    impl<F: FnMut(&Stmt)> Visitor for Statements<F> {
        fn visit_stmt(&mut self, node: &Stmt) {
            (self.0)(node);
            walk_stmt(self, node);
        }
    }
    Statements(f).visit_program(program);
}
