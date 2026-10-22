//! Span index of function bodies, for rules that need the innermost enclosing
//! function of a node the semantic walker hands them.
//!
//! [`crate::semantic_scope`] reports expressions and statements without
//! their enclosing declaration. Bodies nest strictly, so the innermost body
//! containing a node is the containing entry with the greatest start offset.

use falcon_syntax::cst::visit::{
    Visitor, walk_constructor_decl, walk_expr, walk_function_decl, walk_getter_decl,
    walk_method_decl, walk_program, walk_setter_decl, walk_stmt,
};
use falcon_syntax::cst::*;

/// The declaration form that owns a function body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyOwner {
    TopLevelFunction,
    LocalFunction,
    Method,
    Getter,
    Setter,
    Operator,
    Constructor,
    Closure,
}

pub struct FunctionBodyInfo<'a> {
    pub owner: BodyOwner,
    pub async_marker: AsyncMarker,
    /// The declared return type; closures and constructors never have one.
    pub return_type: Option<DartType<'a>>,
    pub span: Span,
    /// The unparenthesized expression of an `=>` body.
    pub arrow_expression: Option<Span>,
}

pub struct FunctionBodies<'a> {
    /// Sorted by `(start, Reverse(end))`, collection order breaking ties.
    bodies: Vec<FunctionBodyInfo<'a>>,
    body_parents: Vec<Option<usize>>,
    /// Try-statement body and `finally` blocks: the official lints treat a
    /// `Block` whose parent is a `TryStatement` as a boundary. Sorted like
    /// `bodies`.
    try_blocks: Vec<Span>,
    try_parents: Vec<Option<usize>>,
}

impl<'a> FunctionBodies<'a> {
    pub fn new(program: Program<'a>) -> Self {
        let mut collector = Self {
            bodies: Vec::new(),
            body_parents: Vec::new(),
            try_blocks: Vec::new(),
            try_parents: Vec::new(),
        };
        walk_program(&mut collector, program);
        collector
            .bodies
            .sort_by_key(|info| (info.span.start, std::cmp::Reverse(info.span.end)));
        collector.body_parents = nesting_parents(collector.bodies.iter().map(|info| info.span));
        collector
            .try_blocks
            .sort_by_key(|span| (span.start, std::cmp::Reverse(span.end)));
        collector.try_parents = nesting_parents(collector.try_blocks.iter().copied());
        collector
    }

    /// The innermost function body containing `span`.
    pub fn enclosing(&self, span: &Span) -> Option<&FunctionBodyInfo<'a>> {
        innermost(&self.bodies, &self.body_parents, |info| info.span, span)
    }

    /// Whether a try-statement body or `finally` block lies between `span` and
    /// its innermost enclosing function body.
    pub fn inside_try_block(&self, span: &Span) -> bool {
        let function_start = self.enclosing(span).map_or(0, |info| info.span.start);
        // Containing blocks nest, so the innermost one has the greatest start.
        innermost(&self.try_blocks, &self.try_parents, |block| *block, span)
            .is_some_and(|block| block.start >= function_start)
    }

    fn push(
        &mut self,
        owner: BodyOwner,
        async_marker: AsyncMarker,
        return_type: Option<DartType<'a>>,
        body: Option<FunctionBody<'a>>,
    ) {
        if let Some(body) = body {
            self.bodies.push(FunctionBodyInfo {
                owner,
                async_marker,
                return_type,
                span: body.span(),
                arrow_expression: match body {
                    FunctionBody::Arrow(arrow) => Some(arrow.expr().unparenthesized().span()),
                    FunctionBody::Block(_) | FunctionBody::Native(_) => None,
                },
            });
        }
    }
}

/// The parent of each span in `spans` (sorted by `(start, Reverse(end))`):
/// the nearest earlier span containing it. Function bodies and try blocks are
/// syntax nodes, so they nest without partial overlap.
fn nesting_parents(spans: impl ExactSizeIterator<Item = Span>) -> Vec<Option<usize>> {
    let mut parents = Vec::with_capacity(spans.len());
    // The currently open chain of containers; its length is the nesting depth.
    let mut open: Vec<(usize, Span)> = Vec::new();
    for (index, span) in spans.enumerate() {
        while open.last().is_some_and(|(_, outer)| !outer.contains(span)) {
            open.pop();
        }
        parents.push(open.last().map(|&(parent, _)| parent));
        open.push((index, span));
    }
    parents
}

/// The innermost item whose span contains `span`, i.e. the containing item
/// with the greatest start. Every container starts at or before `span`, so in
/// a nested family each is an ancestor of the last item starting there.
fn innermost<'a, T>(
    items: &'a [T],
    parents: &[Option<usize>],
    span_of: impl Fn(&T) -> Span,
    span: &Span,
) -> Option<&'a T> {
    let mut candidate = items
        .partition_point(|item| span_of(item).start <= span.start)
        .checked_sub(1);
    while let Some(index) = candidate {
        if span_of(&items[index]).contains(*span) {
            return Some(&items[index]);
        }
        candidate = parents[index];
    }
    None
}

impl<'a> Visitor<'a> for FunctionBodies<'a> {
    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) {
        let owner = match node.kind() {
            FunctionKind::Function(_) => BodyOwner::TopLevelFunction,
            FunctionKind::Getter => BodyOwner::Getter,
            FunctionKind::Setter(_) => BodyOwner::Setter,
        };
        self.push(owner, node.async_marker(), node.return_type(), node.body());
        walk_function_decl(self, node);
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) {
        self.push(
            BodyOwner::Method,
            node.async_marker(),
            node.return_type(),
            node.body(),
        );
        walk_method_decl(self, node);
    }

    fn visit_getter_decl(&mut self, node: GetterDecl<'a>) {
        self.push(
            BodyOwner::Getter,
            node.async_marker(),
            node.return_type(),
            node.body(),
        );
        walk_getter_decl(self, node);
    }

    fn visit_setter_decl(&mut self, node: SetterDecl<'a>) {
        self.push(BodyOwner::Setter, AsyncMarker::Sync, None, node.body());
        walk_setter_decl(self, node);
    }

    fn visit_constructor_decl(&mut self, node: ConstructorDecl<'a>) {
        self.push(BodyOwner::Constructor, AsyncMarker::Sync, None, node.body());
        walk_constructor_decl(self, node);
    }

    fn visit_primary_constructor_body(&mut self, node: PrimaryConstructorBody<'a>) {
        self.push(BodyOwner::Constructor, AsyncMarker::Sync, None, node.body());
        falcon_syntax::cst::visit::walk_primary_constructor_body(self, node);
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) {
        if let ClassMember::Operator(operator) = node {
            self.push(
                BodyOwner::Operator,
                operator.async_marker(),
                operator.return_type(),
                operator.body(),
            );
        }
        falcon_syntax::cst::visit::walk_class_member(self, node);
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) {
        match node {
            Stmt::LocalFunc(function) => self.push(
                BodyOwner::LocalFunction,
                function.async_marker(),
                function.return_type(),
                Some(function.body()),
            ),
            Stmt::TryCatch(statement) => {
                self.try_blocks.push(statement.body().span());
                if let Some(finally) = statement.finally() {
                    self.try_blocks.push(finally.span());
                }
            }
            _ => {}
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: Expr<'a>) {
        if let Expr::FuncExpr(function) = node {
            self.push(
                BodyOwner::Closure,
                function.async_marker(),
                None,
                Some(function.body()),
            );
        }
        walk_expr(self, node);
    }
}
