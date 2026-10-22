//! Reports a null-checked local ignored by a branch that passes another local
//! of the same resolved type to a call. Declaration identities keep shadowed
//! names distinct. Branch-local replacements and unknown types are excluded.

use std::collections::HashMap;

use falcon_analyze::{ResolvedType, ResolvedTypeView};
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnusedAfterNullCheck;

impl Rule for AvoidUnusedAfterNullCheck {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            declarations: ctx.local_declarations(program),
            guards: Vec::new(),
            diags: Vec::new(),
        }))
    }
}

struct Guard {
    declaration: NodeId,
    checked: Span,
    branch: Span,
    ty: ResolvedType,
    used: bool,
    replacement: bool,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    declarations: &'a HashMap<usize, NodeId>,
    guards: Vec<Guard>,
    diags: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, statement: Stmt<'a>, state: &SemanticState<'a>) {
        self.flush_before(statement.span().start);
        let Stmt::If(node) = statement else { return };
        let IfCondition::Expr(condition) = node.condition() else {
            return;
        };
        let Expr::Binary(check) = condition.unparenthesized() else {
            return;
        };
        if check.op() != BinaryOp::NotEq {
            return;
        }
        let checked = match (
            check.left().unparenthesized(),
            check.right().unparenthesized(),
        ) {
            (Expr::Ident(name), Expr::NullLit(_)) | (Expr::NullLit(_), Expr::Ident(name)) => name,
            _ => return,
        };
        let Some(&declaration) = self.declarations.get(&checked.span().start) else {
            return;
        };
        let ty = state.infer(Expr::Ident(checked));
        if !matches!(ty.view(), ResolvedTypeView::Interface { .. }) || !ty.nullable() {
            return;
        }
        self.guards.push(Guard {
            declaration,
            checked: checked.span(),
            branch: node.then_branch().span(),
            ty: ty.non_null(),
            used: false,
            replacement: false,
        });
    }

    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        self.flush_before(expression.span().start);
        // Argument notifications require a resolved callable signature. Inspect
        // the call itself so SDK stubs and unresolved callees work identically.
        if let Expr::Call(call) = expression {
            let args = call.args();
            for argument in args.positional().chain(args.named().map(|arg| arg.value())) {
                let Expr::Ident(name) = argument.unparenthesized() else {
                    continue;
                };
                let Some(declaration) = self.declarations.get(&name.span().start) else {
                    continue;
                };
                let syntax = name.syntax();
                let Some(declaration_span) = syntax
                    .tree()
                    .node(syntax.source(), *declaration)
                    .map(|node| node.span())
                else {
                    continue;
                };
                let ty = state.infer(argument).non_null();
                for guard in &mut self.guards {
                    if guard.branch.contains(argument.span())
                        && guard.declaration != *declaration
                        && !guard.branch.contains(declaration_span)
                        && ty == guard.ty
                    {
                        guard.replacement = true;
                    }
                }
            }
        }
        let Expr::Ident(name) = expression else {
            return;
        };
        let Some(declaration) = self.declarations.get(&name.span().start) else {
            return;
        };
        for guard in &mut self.guards {
            if guard.declaration == *declaration && guard.branch.contains(name.span()) {
                guard.used = true;
            }
        }
    }

    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        self.flush_before(usize::MAX);
        self.diags
    }
}

impl Collector<'_> {
    fn flush_before(&mut self, start: usize) {
        while self
            .guards
            .last()
            .is_some_and(|guard| guard.branch.end <= start)
        {
            let guard = self.guards.pop().unwrap();
            if !guard.used && guard.replacement {
                self.diags.push(self.ctx.diagnostic("This null-checked variable is unused; check whether the branch uses the wrong variable.", guard.checked));
            }
        }
    }
}

