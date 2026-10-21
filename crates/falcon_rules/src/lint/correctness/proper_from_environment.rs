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
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct ProperFromEnvironment;

impl Rule for ProperFromEnvironment {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        self.contexts.push(self.in_const);
        if matches!(node, Expr::Paren(_)) {
            return StructuralTraversal::Continue;
        }
        if let Some((span, is_const_ctor)) = from_environment_invocation(node)
            && !is_const_ctor
            && !self.in_const
        {
            self.diags.push(self.ctx.diagnostic("*.fromEnvironment must be used in a const context, otherwise it returns the default at runtime.", span));
        }

        match node {
            Expr::New(expression) if expression.is_const() => self.in_const = true,
            Expr::List(expression) if expression.is_const() => self.in_const = true,
            Expr::Set(expression) if expression.is_const() => self.in_const = true,
            Expr::Map(expression) if expression.is_const() => self.in_const = true,
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: Expr<'a>) {
        self.leave_const();
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) -> StructuralTraversal {
        self.enter_const(node.keyword() == VarKeyword::Const);
        StructuralTraversal::Continue
    }

    fn exit_local_var(&mut self, _node: LocalVarDecl<'a>) {
        self.leave_const();
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) -> StructuralTraversal {
        self.enter_const(node.keyword() == VarKeyword::Const);
        StructuralTraversal::Continue
    }

    fn exit_field_decl(&mut self, _node: FieldDecl<'a>) {
        self.leave_const();
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.enter_const(matches!(node, TopLevelDecl::Variable(v) if v.keyword() == VarKeyword::Const));
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, _node: TopLevelDecl<'a>) {
        self.leave_const();
    }

    fn visit_annotation(&mut self, _node: Annotation<'a>) -> StructuralTraversal {
        self.enter_const(true);
        StructuralTraversal::Continue
    }

    fn exit_annotation(&mut self, _node: Annotation<'a>) {
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
fn from_environment_invocation(expr: Expr<'_>) -> Option<(Span, bool)> {
    match expr.unparenthesized() {
        Expr::Call(call) => {
            if let Expr::Field(field) = call.callee().unparenthesized()
                && field.field().name() == "fromEnvironment"
                && let Expr::Ident(id) = field.object().unparenthesized()
                && is_env_type(id.name())
            {
                return Some((call.span(), false));
            }
            None
        }
        Expr::New(construction)
            if construction
                .constructor_name()
                .is_some_and(|name| name.name() == "fromEnvironment")
                && matches!(construction.dart_type(), DartType::Named(named)
                    if named.segments().last().is_some_and(|segment| is_env_type(segment.name()))) =>
        {
            Some((construction.span(), construction.is_const()))
        }
        _ => None,
    }
}

fn is_env_type(name: &str) -> bool {
    matches!(name, "String" | "int" | "bool")
}
