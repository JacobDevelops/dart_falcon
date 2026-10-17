//! Require Riverpod once-providers to be created through `.once()`.
//!
//! Flags a direct invocation of `OnceProvider`, `FutureProvider`, or
//! `StateProvider` — for example `OnceProvider(...)` — where the `.once()`
//! factory should be used instead. The `.once()` form constructs the provider's
//! value a single time rather than rebuilding it on each read, which is the
//! intended lifecycle for these providers; calling the constructor directly
//! opts out of it. Calls already written as `Provider.once(...)` are accepted,
//! and explicit `new` constructions and explicit or implicit const contexts
//! are left alone.
//! Checks calls throughout declarations, including initializers, callbacks,
//! collection literals, conditions, and nested call receivers.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct UseOnceConstructorsOnceProvider;

impl Rule for UseOnceConstructorsOnceProvider {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
            in_const: false,
            const_stack: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

const ONCE_PROVIDERS: &[&str] = &["OnceProvider", "FutureProvider", "StateProvider"];

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    in_const: bool,
    const_stack: Vec<bool>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if !self.in_const
            && let Expr::Call { callee, .. } = node
            && let Expr::Ident(id) = callee.unparenthesized()
            && ONCE_PROVIDERS.contains(&id.name.as_str())
        {
            self.diags.push(self.ctx.diagnostic(
                "use-once-constructors-once-provider",
                "OnceProvider and similar Riverpod providers should use .once() factory method",
                *node.span(),
            ));
        }
        self.const_stack.push(self.in_const);
        self.in_const |= matches!(
            node,
            Expr::New { is_const: true, .. }
                | Expr::List { is_const: true, .. }
                | Expr::Set { is_const: true, .. }
                | Expr::Map { is_const: true, .. }
                | Expr::Record { is_const: true, .. }
        );
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: &Expr) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) -> StructuralTraversal {
        self.const_stack.push(self.in_const);
        self.in_const |= node.keyword == VarKeyword::Const;
        StructuralTraversal::Continue
    }

    fn exit_local_var(&mut self, _node: &LocalVarDecl) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) -> StructuralTraversal {
        self.const_stack.push(self.in_const);
        self.in_const |= node.keyword == VarKeyword::Const;
        StructuralTraversal::Continue
    }

    fn exit_field_decl(&mut self, _node: &FieldDecl) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
        self.const_stack.push(self.in_const);
        if let TopLevelDecl::Variable(variable) = node {
            self.in_const |= variable.keyword == VarKeyword::Const;
        }
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, _node: &TopLevelDecl) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn visit_annotation(&mut self, _node: &Annotation) -> StructuralTraversal {
        self.const_stack.push(self.in_const);
        self.in_const = true;
        StructuralTraversal::Continue
    }

    fn exit_annotation(&mut self, _node: &Annotation) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn visit_pattern(&mut self, node: &Pattern) -> StructuralTraversal {
        self.const_stack.push(self.in_const);
        self.in_const |= matches!(node, Pattern::Const(_));
        StructuralTraversal::Continue
    }

    fn exit_pattern(&mut self, _node: &Pattern) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn visit_enum_decl(&mut self, _node: &EnumDecl) -> StructuralTraversal {
        self.const_stack.push(self.in_const);
        self.in_const = true;
        StructuralTraversal::Continue
    }

    fn exit_enum_decl(&mut self, _node: &EnumDecl) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn visit_class_member(&mut self, _node: &ClassMember) -> StructuralTraversal {
        self.const_stack.push(self.in_const);
        self.in_const = false;
        StructuralTraversal::Continue
    }

    fn exit_class_member(&mut self, _node: &ClassMember) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
