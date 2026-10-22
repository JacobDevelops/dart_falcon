//! Sort Widget-valued child properties last, except before closure arguments.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct SortChildPropertiesLast;

impl Rule for SortChildPropertiesLast {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, ChildOrdering, ctx, diagnostics)
    }
}

struct ChildOrdering<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for ChildOrdering<'_, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if !matches!(node, Expr::New(_) | Expr::Call(_)) {
            return;
        }
        let Some((identity, constructor, args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
        else {
            return;
        };
        if !state.signatures.has_constructor(&identity, constructor)
            || state.signatures.is_subtype_of(
                &ResolvedType::interface_identity(identity),
                &DeclarationIdentity::flutter("Widget"),
                &state.model,
            ) != Truth::Yes
        {
            return;
        }
        let mut children = args.named().enumerate().filter(|(_, argument)| {
            matches!(argument.name().name(), "child" | "children")
                && widget_property(&state.infer(argument.value()), state)
        });
        let Some((index, child)) = children.next() else {
            return;
        };
        if children.next().is_some() {
            return;
        }
        if args
            .named()
            .skip(index + 1)
            .any(|argument| !matches!(argument.value(), Expr::FuncExpr(_)))
        {
            self.diagnostics.push(self.ctx.diagnostic("The child/children argument should be last in a widget constructor call.", child.span()));
        }
    }
}

fn widget_property(ty: &ResolvedType, state: &SemanticState<'_>) -> bool {
    let ResolvedType::Interface {
        identity,
        arguments,
        ..
    } = ty
    else {
        return false;
    };
    if state
        .signatures
        .is_subtype_of(ty, &DeclarationIdentity::flutter("Widget"), &state.model)
        == Truth::Yes
    {
        return true;
    }
    if state.signatures.type_parameter_count(identity) == Some(1)
        && let [element] = arguments.as_ref()
        && [
            ("dart:core", "List"),
            ("dart:core", "Map"),
            ("dart:core", "Set"),
            ("dart:collection", "LinkedHashMap"),
            ("dart:collection", "LinkedHashSet"),
        ]
        .iter()
        .any(|(uri, name)| {
            state
                .signatures
                .is_subtype_of(ty, &DeclarationIdentity::sdk(uri, name), &state.model)
                == Truth::Yes
        })
    {
        return widget_property(element, state);
    }
    false
}
