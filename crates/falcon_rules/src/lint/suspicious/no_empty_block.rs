//! Flags empty blocks (`{}`).
//!
//! An empty function, method, loop, or switch-case body is frequently an
//! unfinished implementation or a leftover after code was deleted, and it hides
//! whether the emptiness is deliberate. Fill in the intended behavior, or, if a
//! no-op is genuinely correct, add a comment saying so. A block containing only
//! a comment is treated as intentional and left alone. The report lands on the
//! closing brace.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct NoEmptyBlock;

impl Rule for NoEmptyBlock {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Collector<'_, '_> {
    fn check_body(&mut self, body: Option<&FunctionBody>) {
        if let Some(FunctionBody::Block(block)) = body {
            flag_if_empty(block, &mut self.diags, self.ctx);
        }
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) -> StructuralTraversal {
        self.check_body(node.body.as_ref());
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'_>) -> StructuralTraversal {
        self.check_body(node.body);
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) -> StructuralTraversal {
        self.check_body(node.body.as_ref());
        StructuralTraversal::Continue
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) -> StructuralTraversal {
        self.check_body(node.body.as_ref());
        StructuralTraversal::Continue
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) -> StructuralTraversal {
        self.check_body(node.body.as_ref());
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        if let ClassMember::Operator(operator) = node {
            self.check_body(operator.body.as_ref());
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        match node {
            Stmt::Block(block) => flag_if_empty(block, &mut self.diags, self.ctx),
            Stmt::TryCatch(try_catch) => {
                flag_if_empty(&try_catch.body, &mut self.diags, self.ctx);
                for catch in &try_catch.catches {
                    flag_if_empty(&catch.body, &mut self.diags, self.ctx);
                }
                if let Some(finally) = &try_catch.finally {
                    flag_if_empty(finally, &mut self.diags, self.ctx);
                }
            }
            Stmt::LocalFunc(local) => self.check_body(Some(&local.body)),
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if let Expr::FuncExpr { body, .. } = node {
            self.check_body(Some(body));
        }
        StructuralTraversal::Continue
    }
}
}

fn flag_if_empty(block: &Block, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if !block.stmts.is_empty() {
        return;
    }
    let syntax = ctx.syntax.syntax_for_span(block.span);
    let open = syntax.token_range(block.span).start;
    let Some(close_id) = syntax.closing_delimiter(open) else {
        return;
    };
    let close = &syntax.tokens()[close_id.index()];
    if syntax.tokens()[open.index() + 1..close_id.index()]
        .iter()
        .any(|token| token.kind.is_comment())
    {
        return;
    }
    diags.push(ctx.diagnostic(
        "no-empty-block",
        "Avoid empty blocks — add a comment explaining the intent or remove the block",
        Span::new(close.offset(), close.offset() + close.len()),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn exact_block_end_excludes_following_comment_and_preserves_inner_comment() {
        let source = "void empty() {} /* trailing } */\nvoid intentional() { /* } */ }\n";
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
        } = falcon_dart_parser::parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(
            Path::new("/project/lib/test.dart"),
            source,
            &config,
            &syntax,
        );
        let diagnostics = NoEmptyBlock.analyze(&program, &ctx);
        let closing_brace = source.find("{}").unwrap() + 1;
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].span.start, closing_brace);
        assert_eq!(diagnostics[0].span.end, closing_brace + 1);
    }
}
