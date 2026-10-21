//! Limits the depth of an inline Flutter widget construction tree.
//!
//! Only resolved constructors of Flutter `Widget` subtypes add a level.
//! Canonical Flutter `SizedBox` constructors are transparent and do not count.
//! Lists, parentheses, casts, null assertions and conditional branches add no levels.
//! Helper calls and closures start independent trees: their bodies/arguments are
//! not evidence of the widget returned by the call. References to extracted
//! widgets likewise do not expand a tree. Each tree is reported once at its root.
//!
//! `max-nesting` (integer, default: 8) is the maximum permitted widget depth;
//! the root is level one. This follows the widgets-nesting-level metric's default.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{Expr, Program, Span};

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Maximum depth of nested widget construction allowed in a widget tree.
    max_nesting: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { max_nesting: 8 }
    }
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct WidgetTreeDepth;

impl Rule for WidgetTreeDepth {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let threshold = crate::lint::options::get::<Options>(ctx, Self::NAME).max_nesting;
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            threshold,
            ancestors: Vec::new(),
            trees: Vec::new(),
        }))
    }
}

struct Tree {
    root: Span,
    maximum: usize,
}

struct Frame<'a> {
    expression: Expr<'a>,
    tree: Option<(usize, usize)>,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    threshold: usize,
    ancestors: Vec<Frame<'a>>,
    trees: Vec<Tree>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let span = node.span();
        // Semantic callbacks are preorder but have no expression-exit hook.
        // Source containment restores the current expression ancestry, including
        // siblings visited in evaluation order rather than source order.
        while self.ancestors.last().is_some_and(|frame| {
            let parent = frame.expression.span();
            span.start < parent.start || span.end > parent.end
        }) {
            self.ancestors.pop();
        }
        let inherited = self.ancestors.last().and_then(|frame| {
            let excluded = match frame.expression {
                Expr::Conditional(branch) => Some(branch.condition().span()),
                Expr::Switch(branch) => Some(branch.subject().span()),
                _ => None,
            };
            if excluded
                .is_some_and(|excluded| excluded.start <= span.start && span.end <= excluded.end)
            {
                None
            } else {
                frame.tree
            }
        });
        let (widget, ignored) = if matches!(node, Expr::Call(_) | Expr::New(_)) {
            crate::lint::flutter::scoped_constructor_invocation(node, state).map_or(
                (false, false),
                |(identity, _, _)| {
                    let ignored = self.ctx.identities().is_some_and(|index| {
                        index.is_package_declaration(&identity, "flutter", "SizedBox")
                    });
                    let widget = !ignored
                        && state.signatures.is_subtype_of(
                            &ResolvedType::interface_identity(identity),
                            &DeclarationIdentity::flutter("Widget"),
                            &state.model,
                        ) == Truth::Yes;
                    (widget, ignored)
                },
            )
        } else {
            (false, false)
        };
        let tree = if widget {
            let (index, depth) = inherited.map_or_else(
                || {
                    let index = self.trees.len();
                    self.trees.push(Tree {
                        root: crate::lint::flutter::constructor_span(node),
                        maximum: 1,
                    });
                    (index, 1)
                },
                |(index, depth)| (index, depth + 1),
            );
            self.trees[index].maximum = self.trees[index].maximum.max(depth);
            Some((index, depth))
        } else if ignored
            || matches!(
                node,
                Expr::Paren(_)
                    | Expr::List(_)
                    | Expr::Conditional(_)
                    | Expr::Switch(_)
                    | Expr::As(_)
                    | Expr::NullAssert(_)
            )
        {
            inherited
        } else {
            None
        };
        self.ancestors.push(Frame {
            expression: node,
            tree,
        });
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.trees
            .iter()
            .filter(|tree| tree.maximum > self.threshold)
            .map(|tree| {
                self.ctx.diagnostic(format!(
                    "Widget tree has a depth of {} (max {}). Extract a subtree into a widget.",
                    tree.maximum, self.threshold,
                ), tree.root)
            })
            .collect()
    }
}
