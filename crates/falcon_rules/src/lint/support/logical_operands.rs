use falcon_syntax::cst::*;

pub(crate) fn logical_operands(expression: Expr<'_>) -> usize {
    use falcon_syntax::cst::visit::{Visitor, walk_expr};
    struct Operands(usize);
    impl<'a> Visitor<'a> for Operands {
        fn visit_expr(&mut self, node: Expr<'a>) {
            match node {
                Expr::Binary(binary) if matches!(binary.op(), BinaryOp::And | BinaryOp::Or) => {
                    walk_expr(self, node);
                }
                Expr::Paren(_) | Expr::Unary(_) => walk_expr(self, node),
                _ => self.0 += 1,
            }
        }
    }
    let mut operands = Operands(0);
    operands.visit_expr(expression);
    operands.0
}
