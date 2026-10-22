use crate::lint::support::annotations::has_override;
use crate::lint::support::constants::constant_value;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ConstantValue, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use std::collections::HashMap;
#[derive(Clone, Copy)]
pub(crate) enum ReturnKind {
    Null,
    NonNull,
}

pub(crate) fn prepare<'a>(
    _program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    kind: ReturnKind,
) -> RulePreparation<'a> {
    RulePreparation::Semantic(Box::new(Collector {
        ctx,
        kind,
        values: HashMap::new(),
        diagnostics: Vec::new(),
    }))
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    kind: ReturnKind,
    // Values are captured where lexical bindings are still visible, not when
    // the enclosing callable has already left its parameter/local scopes.
    values: HashMap<NodeId, ConstantValue>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Collector<'a> {
    fn value(&self, expression: Expr<'a>, state: &SemanticState<'a>) -> Option<ConstantValue> {
        let value = constant_value(expression, state, self.ctx)?;
        // A collection that could be const is not necessarily const at runtime;
        // don't equate fresh allocations merely because their contents match.
        matches!(
            value,
            ConstantValue::Null
                | ConstantValue::Bool(_)
                | ConstantValue::Number(_)
                | ConstantValue::String(_)
        )
        .then_some(value)
    }

    fn report(
        &mut self,
        owner: SyntaxNode<'a>,
        is_null: bool,
        model: falcon_analyze::SemanticModel<'_>,
    ) {
        let Some(span) = callable_span(owner, model) else {
            return;
        };
        let message = match (self.kind, is_null) {
            (ReturnKind::Null, true) => {
                "This function returns `null` on every path; remove the return value or return a meaningful value."
            }
            (ReturnKind::NonNull, true) | (ReturnKind::Null, false) => return,
            (ReturnKind::NonNull, _) => {
                "This function returns the same constant on every path; simplify the function or return a meaningful result."
            }
        };
        self.diagnostics.push(self.ctx.diagnostic(message, span));
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        if let Stmt::Return(statement) = node
            && let Some(expression) = statement.value()
            && let Some(value) = self.value(expression, state)
        {
            self.values.insert(statement.syntax().id(), value);
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if matches!(self.kind, ReturnKind::NonNull) {
            return;
        }
        if let Some(body) = node.syntax().parent().and_then(ArrowBody::cast)
            && let Some(owner) = body.syntax().parent()
            && let Some(value) = self.value(node, state)
        {
            self.report(owner, matches!(value, ConstantValue::Null), state.model);
        }
    }

    fn visit_block_end(&mut self, node: Block<'a>, state: &SemanticState<'a>) {
        let Some(owner) = node.syntax().parent() else {
            return;
        };
        if callable_span(owner, state.model).is_none() {
            return;
        }
        // This is a secondary control-flow fold over the completed body, not a
        // syntax visitor: return values come from the shared semantic walk.
        if let Some(flow) = block_flow(node, &self.values)
            && !flow.falls_through
            && (matches!(self.kind, ReturnKind::Null) || flow.return_count >= 2)
            && let Some(value) = flow.value
        {
            self.report(owner, matches!(value, ConstantValue::Null), state.model);
        }
    }
}

/// Getters and setters are deliberately excluded: a constant getter is normal.
fn callable_span(node: SyntaxNode<'_>, model: falcon_analyze::SemanticModel<'_>) -> Option<Span> {
    let (marker, return_type, span) = if let Some(function) = FunctionDecl::cast(node) {
        if !matches!(function.kind(), FunctionKind::Function(_)) {
            return None;
        }
        (
            function.async_marker(),
            function.return_type(),
            function.name().span(),
        )
    } else if let Some(method) = MethodDecl::cast(node) {
        if has_override(method.annotations(), model) {
            return None;
        }
        (
            method.async_marker(),
            method.return_type(),
            method.name().span(),
        )
    } else {
        let function = LocalFuncDecl::cast(node)?;
        (
            function.async_marker(),
            function.return_type(),
            function.name().span(),
        )
    };
    (marker == AsyncMarker::Sync && !matches!(return_type, Some(DartType::Void(_)))).then_some(span)
}

struct Flow<'v> {
    falls_through: bool,
    value: Option<&'v ConstantValue>,
    return_count: usize,
}

impl<'v> Flow<'v> {
    fn fallthrough() -> Self {
        Self {
            falls_through: true,
            value: None,
            return_count: 0,
        }
    }

    fn merge_value(&mut self, other: Option<&'v ConstantValue>) -> Option<()> {
        if let Some(value) = other {
            if self.value.is_some_and(|previous| previous != value) {
                return None;
            }
            self.value = Some(value);
        }
        Some(())
    }
}

fn block_flow<'v>(
    block: Block<'_>,
    values: &'v HashMap<NodeId, ConstantValue>,
) -> Option<Flow<'v>> {
    let mut flow = Flow::fallthrough();
    for statement in block.stmts() {
        if !flow.falls_through {
            break;
        }
        let next = statement_flow(statement, values)?;
        flow.merge_value(next.value)?;
        flow.return_count += next.return_count;
        flow.falls_through = next.falls_through;
    }
    Some(flow)
}

fn statement_flow<'v>(
    statement: Stmt<'_>,
    values: &'v HashMap<NodeId, ConstantValue>,
) -> Option<Flow<'v>> {
    match statement {
        Stmt::Return(node) => Some(Flow {
            falls_through: false,
            // A bare return is not evidence of an always-returned value.
            value: Some(values.get(&node.syntax().id())?),
            return_count: 1,
        }),
        Stmt::Block(block) => block_flow(block, values),
        Stmt::If(node) => {
            let mut then_flow = statement_flow(node.then_branch(), values)?;
            let else_flow = match node.else_branch() {
                Some(branch) => statement_flow(branch, values)?,
                None => Flow::fallthrough(),
            };
            then_flow.merge_value(else_flow.value)?;
            then_flow.return_count += else_flow.return_count;
            then_flow.falls_through |= else_flow.falls_through;
            Some(then_flow)
        }
        // These statements cannot return from this callable. In particular, a
        // local function's returns belong exclusively to that local function.
        Stmt::Empty(_)
        | Stmt::LocalVar(_)
        | Stmt::PatternDecl(_)
        | Stmt::LocalFunc(_)
        | Stmt::Expr(_)
        | Stmt::PatternAssign(_)
        | Stmt::Assert(_) => Some(Flow::fallthrough()),
        Stmt::For(_)
        | Stmt::While(_)
        | Stmt::DoWhile(_)
        | Stmt::Switch(_)
        | Stmt::TryCatch(_)
        | Stmt::Throw(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Labeled(_)
        | Stmt::Yield(_)
        | Stmt::Error(_)
        | Stmt::Rethrow(_) => None,
    }
}
