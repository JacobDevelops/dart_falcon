//! Flags inline block-body callbacks passed to Widget constructors.
//!
//! The rule uses resolved Flutter identities for both the containing Widget class
//! and each constructed Widget. Calls that cannot be resolved uniquely are left
//! alone; spelling, capitalization, and import-prefix shape are not evidence.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralNode, StructuralRuleVisitor, StructuralTraversal, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferExtractingCallbacks;

struct Cfg {
    allowed_line_count: Option<usize>,
    ignored: Vec<String>,
}

fn cfg(ctx: &AnalyzeContext) -> Cfg {
    let opts = crate::lint::options::for_rule(ctx, "prefer-extracting-callbacks");
    Cfg {
        allowed_line_count: crate::lint::options::usize(opts, "allowed_line_count"),
        ignored: crate::lint::options::strings(opts, "ignored_named_arguments")
            .into_iter()
            .flatten()
            .map(str::to_string)
            .collect(),
    }
}

impl Rule for PreferExtractingCallbacks {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(CallbackCollector {
            model: SemanticModel::new(ctx.file_path, identities, ctx.types),
            signatures,
            widget: DeclarationIdentity::flutter("Widget"),
            cfg: cfg(ctx),
            source: ctx.source,
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
    cfg: Cfg,
    source: &'a str,
    ctx: &'a AnalyzeContext<'a>,
    candidates: Vec<Callback>,
    pending: std::collections::HashMap<usize, usize>,
    active: Vec<Option<usize>>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for CallbackCollector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.candidates.iter()
            .filter(|callback| !(callback.builder_parameter && callback.returns_widget))
            .map(|callback| self.ctx.diagnostic(
                "prefer-extracting-callbacks",
                "Prefer extracting the callback to a separate widget method.",
                callback.span,
            )).collect()
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
        if matches!(node, TopLevelDecl::Class(_)) {
            StructuralTraversal::Continue
        } else {
            StructuralTraversal::SkipChildren
        }
    }

    fn visit_class_decl(&mut self, class: &ClassDecl) -> StructuralTraversal {
        let Some(identity) = self.model.resolve_name(&class.name.name) else {
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

    fn route_child(&mut self, parent: StructuralNode<'_>, child: StructuralNode<'_>) -> bool {
        !matches!(parent, StructuralNode::ClassDecl(_)) || matches!(child, StructuralNode::ClassMember(_))
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if matches!(node, Expr::FuncExpr { .. }) {
            self.active.push(self.pending.remove(&node.span().start));
        }
        if let Some((constructed, args)) = crate::lint::flutter::construction(node, &self.model)
            && self
                .signatures
                .is_subtype_of(&constructed, &self.widget, &self.model)
                == Truth::Yes
        {
            for argument in &args.positional {
                self.check_callback(argument);
            }
            for argument in &args.named {
                if !self
                    .cfg
                    .ignored
                    .iter()
                    .any(|name| name == &argument.name.name)
                {
                    self.check_callback(&argument.value);
                }
            }
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, node: &Expr) {
        if matches!(node, Expr::FuncExpr { .. }) {
            self.active.pop();
        }
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if let Some(index) = self.active.last().copied().flatten()
            && self.candidates[index].builder_parameter
            && !self.candidates[index].returns_widget
            && let Stmt::Return(ReturnStmt { value: Some(value), .. }) = node
            && expression_is_widget(value, &self.model, self.signatures, &self.widget)
        {
            self.candidates[index].returns_widget = true;
        }
        StructuralTraversal::Continue
    }
}
}

impl CallbackCollector<'_> {
    fn check_callback(&mut self, expression: &Expr) {
        let Expr::FuncExpr {
            params, body, span, ..
        } = expression.unparenthesized()
        else {
            return;
        };
        if !matches!(body.as_ref(), FunctionBody::Block(block) if !block.stmts.is_empty())
            || !is_long_enough(span, self.source, &self.cfg)
        {
            return;
        }
        let builder_parameter = params.params.first().is_none_or(|first| {
            first.param_type.as_ref().is_none_or(|ty| {
                matches!(self.model.resolve_type(ty), ResolvedType::Interface { identity, nullable: false, .. }
                    if identity.is_package("flutter", "BuildContext"))
            })
        });
        if builder_parameter {
            self.pending.insert(span.start, self.candidates.len());
        }
        self.candidates.push(Callback {
            span: *span,
            builder_parameter,
            returns_widget: false,
        });
    }
}

fn expression_is_widget(
    expression: &Expr,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    widget: &DeclarationIdentity,
) -> bool {
    crate::lint::flutter::construction(expression, model)
        .is_some_and(|(ty, _)| signatures.is_subtype_of(&ty, widget, model) == Truth::Yes)
}

fn is_long_enough(span: &Span, source: &str, cfg: &Cfg) -> bool {
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
