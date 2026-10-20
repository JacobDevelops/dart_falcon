//! Flags zero-based, unit-step index loops over a syntactically known list.
//!
//! A `for-in` loop expresses element traversal directly; use `.indexed` when the
//! index is also needed. Matches a single local initialized to `0`, a strict
//! `index < items.length` bound, and `index++`, `++index`, or `index += 1`.
//! The collection must be a local or parameter explicitly typed `List`, or a
//! local with an inferred type initialized with a list literal. Other types,
//! fields, and computed receivers are skipped because syntax alone cannot
//! establish iterability.
//! Files with unprefixed imports, library parts, or a possible local declaration
//! shadowing `List` are skipped rather than guessing the collection's identity.
//!
//! Body writes to the index, collection rebinding, element writes, collection
//! escapes or method calls, closures, cascades, and suspension are excluded.
//! Collection references in the body must be direct `items[index]` reads.
//! Collection and index references must resolve to the original declarations;
//! shadowed bindings cannot stand in for either loop variable.
//! This is a syntactic recommendation, not an automatic rewrite: indirect
//! mutation through aliases or opaque function calls is not analyzed.

use std::collections::HashMap;

use falcon_analyze::resolve::lexical_usage::local_declarations;
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferForIn;

impl Rule for PreferForIn {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            program,
            locals: local_declarations(program),
            candidates: Vec::new(),
            diags: Vec::new(),
            unshadowed_list: true,
        }))
    }
}

struct Candidate<'a> {
    span: Span,
    body: Span,
    index: &'a str,
    collection: &'a str,
    index_declaration: NodeId,
    collection_declaration: NodeId,
    valid: bool,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    program: Program<'a>,
    locals: HashMap<usize, NodeId>,
    candidates: Vec<Candidate<'a>>,
    diags: Vec<Diagnostic>,
    unshadowed_list: bool,
}

fn is_name(expression: Expr<'_>, name: &str) -> bool {
    matches!(expression.unparenthesized(), Expr::Ident(identifier) if identifier.name() == name)
}

fn is_integer(expression: Expr<'_>, value: &str) -> bool {
    matches!(expression.unparenthesized(), Expr::IntLit(literal) if literal.value() == value)
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

impl<'a> Collector<'a> {
    fn known_list(&self, identifier: Identifier<'a>) -> Option<NodeId> {
        let declaration = *self.locals.get(&identifier.span().start)?;
        let parent = self
            .ctx
            .tree()
            .node(self.ctx.source(), declaration)?
            .parent()?;
        let declared_type = if let Some(parameter) = FormalParam::cast(parent) {
            parameter.param_type()
        } else {
            let variable = VarDeclarator::cast(parent)?;
            let local = LocalVarDecl::cast(parent.parent()?)?;
            if local.var_type().is_none()
                && matches!(
                    variable.initializer().map(Expr::unparenthesized),
                    Some(Expr::List(_))
                )
            {
                return Some(declaration);
            }
            local.var_type()
        };
        let Some(DartType::Named(ty)) = declared_type else {
            return None;
        };
        let mut segments = ty.segments();
        (segments.next().is_some_and(|name| name.name() == "List") && segments.next().is_none())
            .then_some(declaration)
    }

    fn candidate(&self, statement: ForStmt<'a>) -> Option<Candidate<'a>> {
        let header = statement.header();
        let ForInit::VarDecl(initializer) = header.init()? else {
            return None;
        };
        let mut declarations = initializer.declarators();
        let declaration = declarations.next()?;
        if declarations.next().is_some() || !is_integer(declaration.initializer()?, "0") {
            return None;
        }
        let index = declaration.name().name();
        let Expr::Binary(condition) = header.condition()?.unparenthesized() else {
            return None;
        };
        if condition.op() != BinaryOp::Lt || !is_name(condition.left(), index) {
            return None;
        }
        let Expr::Field(length) = condition.right().unparenthesized() else {
            return None;
        };
        if length.field().name() != "length" || length.is_null_safe() {
            return None;
        }
        let Expr::Ident(collection) = length.object().unparenthesized() else {
            return None;
        };
        let collection_declaration = self.known_list(collection)?;
        let mut updates = header.updates();
        let update = updates.next()?.unparenthesized();
        if updates.next().is_some() {
            return None;
        }
        let unit_step = match update {
            Expr::PostfixIncDec(update) => {
                update.op() == PostfixIncDec::Increment && is_name(update.operand(), index)
            }
            Expr::Unary(update) => {
                update.op() == UnaryOp::PlusPlus && is_name(update.operand(), index)
            }
            Expr::Assign(update) => {
                update.op() == AssignOp::PlusEq
                    && is_name(update.target(), index)
                    && is_integer(update.value(), "1")
            }
            _ => false,
        };
        let body = statement.body().span();
        if !unit_step
            || self.ctx.local_writes(self.program).iter().any(|write| {
                contains(body, write.span)
                    && (write.declaration == declaration.name().syntax().id()
                        || write.declaration == collection_declaration)
            })
        {
            return None;
        }
        Some(Candidate {
            span: statement.span(),
            body,
            index,
            collection: collection.name(),
            index_declaration: declaration.name().syntax().id(),
            collection_declaration,
            valid: true,
        })
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        for candidate in &mut self.candidates {
            if contains(candidate.body, node.span())
                && (matches!(node, Stmt::PatternAssign(_) | Stmt::LocalFunc(_) | Stmt::Yield(_))
                    || matches!(node, Stmt::For(statement) if statement.is_await()))
            {
                candidate.valid = false;
            }
        }
        if let Stmt::For(statement) = node
            && let Some(candidate) = self.candidate(statement)
        {
            self.candidates.push(candidate);
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        for candidate in &mut self.candidates {
            if !candidate.valid || !contains(candidate.body, node.span()) {
                continue;
            }
            match node {
                Expr::Ident(identifier) if identifier.name() == candidate.collection => {
                    candidate.valid = self.locals.get(&identifier.span().start)
                        == Some(&candidate.collection_declaration)
                        && node.syntax().parent()
                            .and_then(IndexExpr::cast)
                            .is_some_and(|access| {
                                access.object().span() == node.span()
                                    && is_name(access.index(), candidate.index)
                                    && self.locals.get(&access.index().unparenthesized().span().start)
                                        == Some(&candidate.index_declaration)
                                    && !access.is_null_safe()
                            });
                }
                Expr::Ident(identifier) if identifier.name() == candidate.index => {
                    candidate.valid = self.locals.get(&identifier.span().start)
                        == Some(&candidate.index_declaration);
                }
                Expr::Assign(assignment) => {
                    candidate.valid = matches!(assignment.target().unparenthesized(), Expr::Ident(_));
                }
                Expr::PostfixIncDec(update) => {
                    candidate.valid = matches!(update.operand().unparenthesized(), Expr::Ident(_));
                }
                Expr::Unary(update) if matches!(update.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus) => {
                    candidate.valid = matches!(update.operand().unparenthesized(), Expr::Ident(_));
                }
                Expr::FuncExpr(_) | Expr::Await(_) | Expr::Cascade(_) => candidate.valid = false,
                _ => {}
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) -> StructuralTraversal {
        for candidate in &mut self.candidates {
            if contains(candidate.body, node.span())
                && node.declarators().any(|declaration| {
                    let name = declaration.name().name();
                    name == candidate.index || name == candidate.collection
                })
            {
                candidate.valid = false;
            }
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, node: Stmt<'a>) {
        if let Some(candidate) = self.candidates.pop_if(|candidate| candidate.span == node.span())
            && candidate.valid
        {
            self.diags.push(self.ctx.diagnostic(
                "prefer-for-in",
                "Use a 'for-in' loop; use '.indexed' when the index is needed.",
                candidate.span,
            ));
        }
    }

    fn visit_identifier(&mut self, node: Identifier<'a>) -> StructuralTraversal {
        if node.name() == "List"
            && node.syntax().parent().and_then(NamedType::cast).is_none()
        {
            self.unshadowed_list = false;
        }
        StructuralTraversal::Continue
    }

    fn visit_type_param(&mut self, node: TypeParam<'a>) -> StructuralTraversal {
        if node.name().name() == "List" {
            self.unshadowed_list = false;
        }
        StructuralTraversal::Continue
    }

    fn visit_import(&mut self, node: ImportDirective<'a>) -> StructuralTraversal {
        if node.as_name().is_none() {
            self.unshadowed_list = false;
        }
        StructuralTraversal::Continue
    }

    fn visit_part_directive(&mut self, _node: PartDirective<'a>) -> StructuralTraversal {
        self.unshadowed_list = false;
        StructuralTraversal::Continue
    }

    fn visit_part_of_directive(&mut self, _node: PartOfDirective<'a>) -> StructuralTraversal {
        self.unshadowed_list = false;
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        if self.unshadowed_list {
            self.diags
        } else {
            Vec::new()
        }
    }
}
}
