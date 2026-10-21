//! Flags inline block-body callbacks passed to Widget constructors.
//!
//! The rule uses resolved Flutter identities for both the containing Widget class
//! and each constructed Widget. Calls that cannot be resolved uniquely are left
//! alone; spelling, capitalization, and import-prefix shape are not evidence.
//!
//! `allowed-line-count` (optional integer, default: unset) permits callbacks up
//! to that many lines. `ignored-named-arguments` (string array, default: empty)
//! excludes matching named arguments from callback checks.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralNode, StructuralRuleVisitor, StructuralTraversal, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferExtractingCallbacks;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Callback body line threshold; unset disables the length filter.
    #[serde(deserialize_with = "crate::lint::options::optional_integer")]
    allowed_line_count: Option<usize>,
    /// Named widget-constructor arguments whose callbacks are ignored.
    ignored_named_arguments: Vec<String>,
}

crate::lint::options::impl_options!(Options);

impl Rule for PreferExtractingCallbacks {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(CallbackCollector {
            model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            signatures,
            widget: DeclarationIdentity::flutter("Widget"),
            cfg: crate::lint::options::get::<Options>(ctx, Self::NAME),
            source: ctx.source(),
            ctx,
            candidates: Vec::new(),
            pending: std::collections::HashMap::new(),
            active: Vec::new(),
        }))
    }
}

struct Callback {
    span: Span,
    builder_parameter: bool,
    returns_widget: bool,
}

struct CallbackCollector<'a> {
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    widget: DeclarationIdentity,
    cfg: &'a Options,
    source: &'a str,
    ctx: &'a AnalyzeContext<'a>,
    candidates: Vec<Callback>,
    pending: std::collections::HashMap<usize, usize>,
    active: Vec<Option<usize>>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for CallbackCollector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.candidates.iter()
            .filter(|callback| !(callback.builder_parameter && callback.returns_widget))
            .map(|callback| self.ctx.diagnostic(
                "Prefer extracting the callback to a separate widget method.",
                callback.span,
            )).collect()
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        if matches!(node, TopLevelDecl::Class(_)) {
            StructuralTraversal::Continue
        } else {
            StructuralTraversal::SkipChildren
        }
    }

    fn visit_class_decl(&mut self, class: ClassDecl<'a>) -> StructuralTraversal {
        let Some(identity) = self.model.resolve_name(class.name().name()) else {
            return StructuralTraversal::SkipChildren;
        };
        let class_type = ResolvedType::interface_identity(identity);
        if self.signatures.is_subtype_of(&class_type, &self.widget, &self.model) == Truth::Yes
            || self.signatures.is_subtype_of(&class_type, &DeclarationIdentity::flutter("State"), &self.model) == Truth::Yes
        {
            StructuralTraversal::Continue
        } else {
            StructuralTraversal::SkipChildren
        }
    }

    fn route_child(&mut self, parent: StructuralNode<'a>, child: StructuralNode<'a>) -> bool {
        !matches!(parent, StructuralNode::ClassDecl(_)) || matches!(child, StructuralNode::ClassMember(_))
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if matches!(node, Expr::FuncExpr(_)) {
            self.active.push(self.pending.remove(&node.span().start));
        }
        if let Some((constructed, args)) = crate::lint::flutter::construction(node, &self.model)
            && self
                .signatures
                .is_subtype_of(&constructed, &self.widget, &self.model)
                == Truth::Yes
        {
            for argument in args.positional() {
                self.check_callback(argument);
            }
            for argument in args.named() {
                if !self
                    .cfg
                    .ignored_named_arguments
                    .iter()
                    .any(|name| name == argument.name().name())
                {
                    self.check_callback(argument.value());
                }
            }
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, node: Expr<'a>) {
        if matches!(node, Expr::FuncExpr(_)) {
            self.active.pop();
        }
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Some(index) = self.active.last().copied().flatten()
            && self.candidates[index].builder_parameter
            && !self.candidates[index].returns_widget
            && let Stmt::Return(ret) = node
            && let Some(value) = ret.value()
            && expression_is_widget(value, &self.model, self.signatures, &self.widget)
        {
            self.candidates[index].returns_widget = true;
        }
        StructuralTraversal::Continue
    }
}
}

impl CallbackCollector<'_> {
    fn check_callback(&mut self, expression: Expr<'_>) {
        let Expr::FuncExpr(function) = expression.unparenthesized() else {
            return;
        };
        let span = function.span();
        if !matches!(function.body(), FunctionBody::Block(block) if !block.stmts().is_empty())
            || !is_long_enough(&span, self.source, self.cfg)
        {
            return;
        }
        let builder_parameter = function.params().params().first().is_none_or(|first| {
            first.param_type().is_none_or(|ty| {
                matches!(self.model.resolve_type(ty), ResolvedType::Interface { identity, nullable: false, .. }
                    if identity.is_package("flutter", "BuildContext"))
            })
        });
        if builder_parameter {
            self.pending.insert(span.start, self.candidates.len());
        }
        self.candidates.push(Callback {
            span,
            builder_parameter,
            returns_widget: false,
        });
    }
}

fn expression_is_widget(
    expression: Expr<'_>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    widget: &DeclarationIdentity,
) -> bool {
    crate::lint::flutter::construction(expression, model)
        .is_some_and(|(ty, _)| signatures.is_subtype_of(&ty, widget, model) == Truth::Yes)
}

fn is_long_enough(span: &Span, source: &str, cfg: &Options) -> bool {
    match cfg.allowed_line_count {
        None => true,
        Some(limit) => {
            let end = span.end.min(source.len());
            let Some(text) = source.get(span.start..end) else {
                return false;
            };
            text.bytes().filter(|byte| *byte == b'\n').count() + 1 > limit
        }
    }
}
