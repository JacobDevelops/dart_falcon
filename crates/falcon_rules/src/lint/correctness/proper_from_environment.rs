//! Require `fromEnvironment` to be evaluated in a const context.
//!
//! Flags a `String.fromEnvironment`, `int.fromEnvironment`, or
//! `bool.fromEnvironment` invocation that is not in a const context. These
//! constructors read `--dart-define` values only when evaluated at compile time;
//! called at runtime they ignore the environment entirely and silently return
//! the default, so a missing `const` turns a configured value into its fallback
//! with no error. Prefix the call with `const`, or place it somewhere already
//! const — a `const` variable initializer or an annotation argument.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralNode, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct ProperFromEnvironment;

impl Rule for ProperFromEnvironment {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
            in_const: false,
            contexts: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    /// True while visiting a const context (const initializer / const literal /
    /// const construction), where `*.fromEnvironment` is evaluated at compile time.
    in_const: bool,
    contexts: Vec<bool>,
}

impl Collector<'_> {
    fn enter_const(&mut self, is_const: bool) {
        self.contexts.push(self.in_const);
        self.in_const |= is_const;
    }

    fn leave_const(&mut self) {
        self.in_const = self.contexts.pop().unwrap();
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        self.contexts.push(self.in_const);
        if matches!(node, Expr::Paren { .. }) {
            return StructuralTraversal::Continue;
        }
        if let Some((span, is_const_ctor)) = from_environment_invocation(node)
            && !is_const_ctor
            && !self.in_const
        {
            self.diags.push(self.ctx.diagnostic("proper-from-environment", "*.fromEnvironment must be used in a const context, otherwise it returns the default at runtime.", *span));
        }

        match node {
            Expr::New { is_const: true, .. }
            | Expr::List { is_const: true, .. }
            | Expr::Set { is_const: true, .. }
            | Expr::Map { is_const: true, .. } => self.in_const = true,
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: &Expr) {
        self.leave_const();
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) -> StructuralTraversal {
        self.enter_const(node.keyword == VarKeyword::Const);
        StructuralTraversal::Continue
    }

    fn exit_local_var(&mut self, _node: &LocalVarDecl) {
        self.leave_const();
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) -> StructuralTraversal {
        self.enter_const(node.keyword == VarKeyword::Const);
        StructuralTraversal::Continue
    }

    fn exit_field_decl(&mut self, _node: &FieldDecl) {
        self.leave_const();
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
        self.enter_const(matches!(node, TopLevelDecl::Variable(v) if v.keyword == VarKeyword::Const));
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, _node: &TopLevelDecl) {
        self.leave_const();
    }

    fn visit_annotation(&mut self, _node: &Annotation) -> StructuralTraversal {
        self.enter_const(true);
        StructuralTraversal::Continue
    }

    fn exit_annotation(&mut self, _node: &Annotation) {
        self.leave_const();
    }

    fn route_child(&mut self, parent: StructuralNode<'_>, child: StructuralNode<'_>) -> bool {
        match parent {
            StructuralNode::LocalVar(_) | StructuralNode::FieldDecl(_)
            | StructuralNode::TopLevelVar(_) => {
                matches!(child, StructuralNode::VarDeclarator(_))
            }
            StructuralNode::VarDeclarator(_) => matches!(child, StructuralNode::Expr(_)),
            StructuralNode::Annotation(_) => matches!(child, StructuralNode::ArgList(_)),
            _ => true,
        }
    }
}
}

/// Detect a `String`/`int`/`bool` `.fromEnvironment(...)` invocation, in either
/// the implicit-call form (`String.fromEnvironment(...)`) or the explicit
/// `new`/`const` form. Returns the invocation span and whether it is a `const`
/// construction.
fn from_environment_invocation(expr: &Expr) -> Option<(&Span, bool)> {
    match expr.unparenthesized() {
        Expr::Call { callee, span, .. } => {
            if let Expr::Field { object, field, .. } = callee.unparenthesized()
                && field.name == "fromEnvironment"
                && let Expr::Ident(id) = object.unparenthesized()
                && is_env_type(&id.name)
            {
                return Some((span, false));
            }
            None
        }
        Expr::New {
            dart_type: DartType::Named(nt),
            constructor_name: Some(cn),
            is_const,
            span,
            ..
        } if cn.name == "fromEnvironment"
            && nt.segments.last().is_some_and(|s| is_env_type(&s.name)) =>
        {
            Some((span, *is_const))
        }
        _ => None,
    }
}

fn is_env_type(name: &str) -> bool {
    matches!(name, "String" | "int" | "bool")
}
