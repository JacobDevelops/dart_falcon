use falcon_analyze::{
    AnalyzeContext, RulePreparation, StructuralRuleVisitor, StructuralTraversal, structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{StatementListKind, body_list, member_list, stmt_lists};
use falcon_syntax::cst::*;

#[derive(Clone, Copy)]
pub(super) enum Target {
    Method,
    Case,
    Throw,
    Continue,
    Break,
    Constructor,
}

pub(super) fn prepare<'a>(ctx: &'a AnalyzeContext<'a>, target: Target) -> RulePreparation<'a> {
    RulePreparation::Structural(Box::new(Collector {
        ctx,
        target,
        diags: Vec::new(),
    }))
}

pub(super) fn has_blank_line_before(node: SyntaxNode<'_>) -> bool {
    let Some(first) = node.significant_tokens().next() else {
        return false;
    };
    let mut newlines = 0;
    let mut previous = first.prev();
    while let Some(token) = previous.filter(|token| token.is_trivia()) {
        if token.kind().is_comment() {
            newlines = 0;
        } else {
            newlines += token.text().bytes().filter(|&byte| byte == b'\n').count();
            if newlines >= 2 {
                return true;
            }
        }
        previous = token.prev();
    }
    false
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    target: Target,
    diags: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn check(&mut self, node: SyntaxNode<'_>) {
        if has_blank_line_before(node) {
            return;
        }
        let message = match self.target {
            Target::Method => "Add a blank line before method declaration",
            Target::Case => "Add a blank line before case clause",
            Target::Throw => "Add a blank line before throw statement",
            Target::Continue => "Add a blank line before continue statement",
            Target::Break => "Add a blank line before break statement",
            Target::Constructor => "Add a blank line before constructor declaration",
        };
        self.diags.push(self.ctx.diagnostic(message, node.span()));
    }

    fn statements(&mut self, stmts: AstChildren<'_, Stmt<'_>>, kind: StatementListKind) {
        if kind != StatementListKind::Block
            || !matches!(
                self.target,
                Target::Throw | Target::Continue | Target::Break
            )
        {
            return;
        }
        for stmt in stmts.skip(1) {
            if matches!(
                (self.target, stmt),
                (Target::Throw, Stmt::Throw(_))
                    | (Target::Continue, Stmt::Continue(_))
                    | (Target::Break, Stmt::Break(_))
            ) {
                self.check(stmt.syntax());
            }
        }
    }

    fn members(&mut self, members: AstChildren<'_, ClassMember<'_>>, preceded: bool) {
        for member in members.skip(usize::from(!preceded)) {
            if matches!(
                (self.target, member),
                (Target::Method, ClassMember::Method(_))
                    | (Target::Constructor, ClassMember::Constructor(_))
            ) {
                self.check(member.syntax());
            }
        }
    }
}

structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        if matches!(self.target, Target::Method | Target::Constructor) {
            match node {
                TopLevelDecl::Class(node) => self.members(node.members(), false),
                TopLevelDecl::Mixin(node) => self.members(node.members(), false),
                TopLevelDecl::MixinClass(node) => self.members(node.members(), false),
                TopLevelDecl::Enum(node) => self.members(node.members(), node.variants().next().is_some()),
                TopLevelDecl::Extension(node) => self.members(node.members(), false),
                TopLevelDecl::ExtensionType(node) => self.members(node.members(), false),
                _ => {}
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        body_list(node.body(), &mut |stmts, kind| self.statements(stmts, kind));
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        member_list(node, &mut |stmts, kind| self.statements(stmts, kind));
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::Switch(switch) = node
            && matches!(self.target, Target::Case)
        {
            for case in switch.cases().skip(1) {
                self.check(case.syntax());
            }
        }
        stmt_lists(node, &mut |stmts, kind| self.statements(stmts, kind));
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::FuncExpr(function) = node {
            body_list(Some(function.body()), &mut |stmts, kind| self.statements(stmts, kind));
        }
        StructuralTraversal::Continue
    }
}
}
