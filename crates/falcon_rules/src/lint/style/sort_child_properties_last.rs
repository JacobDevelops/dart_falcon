//! Sort Widget-valued child properties last, except before closure arguments.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct SortChildPropertiesLast;

impl Rule for SortChildPropertiesLast {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut visitor = ChildOrdering {
            ctx,
            diagnostics: Vec::new(),
        };
        visit_program(&mut visitor, program, state);
        visitor.diagnostics
    }
}

struct ChildOrdering<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diagnostics: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for ChildOrdering<'_, '_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if !matches!(node, Expr::New { .. } | Expr::Call { .. }) {
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
        let mut children = args.named.iter().enumerate().filter(|(_, argument)| {
            matches!(argument.name.name.as_str(), "child" | "children")
                && widget_property(&state.infer(&argument.value), state)
        });
        let Some((index, child)) = children.next() else {
            return;
        };
        if children.next().is_some() {
            return;
        }
        if args.named[index + 1..]
            .iter()
            .any(|argument| !matches!(argument.value, Expr::FuncExpr { .. }))
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "sort-child-properties-last",
                "The child/children argument should be last in a widget constructor call.",
                child.span,
            ));
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
