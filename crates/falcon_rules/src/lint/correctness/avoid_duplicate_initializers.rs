//! Flags final/const variables with identical initializers in the same scope.
//! Calls, construction, await, mutation, collection allocation, and closures are
//! excluded. Non-declaration statements invalidate local candidates, so writes
//! or calls between reads cannot turn an intentional state snapshot into a lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst;

#[derive(Default)]
pub struct AvoidDuplicateInitializers;

impl Rule for AvoidDuplicateInitializers {
    fn prepare<'a>(
        &self,
        _program: cst::Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

struct Candidate<'a> {
    declaration: cst::VarDeclarator<'a>,
    value: cst::Expr<'a>,
    type_: Option<cst::DartType<'a>>,
    category: (bool, bool, bool),
}

impl<'a> Collector<'a> {
    fn collect(
        &mut self,
        declarations: cst::AstChildren<'a, cst::VarDeclarator<'a>>,
        type_: Option<cst::DartType<'a>>,
        category: (bool, bool, bool),
        seen: &mut Vec<Candidate<'a>>,
    ) {
        for declaration in declarations {
            let Some(value) = declaration.initializer() else {
                continue;
            };
            if !safe(value) {
                seen.clear();
                continue;
            }
            let original = seen.iter().find(|earlier| {
                earlier.category == category
                    && match (earlier.type_, type_) {
                        (Some(left), Some(right)) => super::avoid_duplicate_exports::same_syntax(
                            left.syntax(),
                            right.syntax(),
                        ),
                        (None, None) => true,
                        _ => false,
                    }
                    && super::avoid_duplicate_exports::same_syntax(
                        earlier.value.syntax(),
                        value.syntax(),
                    )
            });
            if let Some(earlier) = original {
                self.diags.push(self.ctx.diagnostic(
                    "avoid-duplicate-initializers",
                    format!("This initializer duplicates '{}' — reuse that variable or correct this initializer", earlier.declaration.name().name()),
                    declaration.name().span(),
                ));
            } else {
                seen.push(Candidate {
                    declaration,
                    value,
                    type_,
                    category,
                });
            }
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_block(&mut self, node: cst::Block<'a>) -> StructuralTraversal {
        let mut seen = Vec::new();
        for statement in node.stmts() {
            match statement {
                cst::Stmt::LocalVar(declaration) if matches!(declaration.keyword(), cst::VarKeyword::Final | cst::VarKeyword::Const) && !declaration.is_late() => {
                    self.collect(declaration.declarators(), declaration.var_type(), (false, false, declaration.keyword() == cst::VarKeyword::Const), &mut seen);
                }
                _ => seen.clear(),
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_top_level_decl(&mut self, node: cst::TopLevelDecl<'a>) -> StructuralTraversal {
        if let Some(members) = crate::lint::declarations::members_of(node) {
            let mut seen = Vec::new();
            for member in members {
                if let cst::ClassMember::Field(field) = member {
                    if matches!(field.keyword(), cst::VarKeyword::Final | cst::VarKeyword::Const) && !field.is_late() {
                        self.collect(field.declarators(), field.field_type(), (field.is_static(), field.is_late(), field.keyword() == cst::VarKeyword::Const), &mut seen);
                    } else {
                        seen.clear();
                    }
                }
            }
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }
}
}

fn safe(value: cst::Expr<'_>) -> bool {
    value
        .syntax()
        .descendants()
        .filter_map(cst::Expr::cast)
        .all(|expr| match expr {
            cst::Expr::Ident(_)
            | cst::Expr::This(_)
            | cst::Expr::Super(_)
            | cst::Expr::IntLit(_)
            | cst::Expr::DoubleLit(_)
            | cst::Expr::BoolLit(_)
            | cst::Expr::NullLit(_)
            | cst::Expr::SymbolLit(_)
            | cst::Expr::StringLit(_)
            | cst::Expr::Field(_)
            | cst::Expr::Paren(_)
            | cst::Expr::Binary(_)
            | cst::Expr::Conditional(_)
            | cst::Expr::Is(_)
            | cst::Expr::As(_)
            | cst::Expr::NullAssert(_) => true,
            cst::Expr::Unary(unary) => !matches!(
                unary.op(),
                cst::UnaryOp::PlusPlus | cst::UnaryOp::MinusMinus
            ),
            _ => false,
        })
}
