use falcon_syntax::cst::*;

pub(crate) fn logical_operands(expression: Expr<'_>) -> usize {
    use falcon_syntax::cst::visit::{Visitor, walk_expr_chain};
    struct Operands(usize);
    impl<'a> Visitor<'a> for Operands {
        fn visit_expr(&mut self, node: Expr<'a>) {
            walk_expr_chain(self, node, |visitor, node| match node {
                Expr::Binary(binary) if matches!(binary.op(), BinaryOp::And | BinaryOp::Or) => true,
                Expr::Paren(_) | Expr::Unary(_) => true,
                _ => {
                    visitor.0 += 1;
                    false
                }
            });
        }
    }
    let mut operands = Operands(0);
    operands.visit_expr(expression);
    operands.0
}
