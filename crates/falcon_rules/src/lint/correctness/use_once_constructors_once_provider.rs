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
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UseOnceConstructorsOnceProvider;

impl Rule for UseOnceConstructorsOnceProvider {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if !self.in_const
            && let Expr::Call(call) = node
            && let Expr::Ident(id) = call.callee().unparenthesized()
            && ONCE_PROVIDERS.contains(&id.name())
        {
            self.diags.push(self.ctx.diagnostic(
                "use-once-constructors-once-provider",
                "OnceProvider and similar Riverpod providers should use .once() factory method",
                node.span(),
            ));
        }
        self.const_stack.push(self.in_const);
        self.in_const |= match node {
            Expr::New(node) => node.is_const(),
            Expr::List(node) => node.is_const(),
            Expr::Set(node) => node.is_const(),
            Expr::Map(node) => node.is_const(),
            Expr::Record(node) => node.is_const(),
            _ => false,
        };
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: Expr<'a>) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) -> StructuralTraversal {
        self.const_stack.push(self.in_const);
        self.in_const |= node.keyword() == VarKeyword::Const;
        StructuralTraversal::Continue
    }

    fn exit_local_var(&mut self, _node: LocalVarDecl<'a>) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) -> StructuralTraversal {
        self.const_stack.push(self.in_const);
        self.in_const |= node.keyword() == VarKeyword::Const;
        StructuralTraversal::Continue
    }

    fn exit_field_decl(&mut self, _node: FieldDecl<'a>) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.const_stack.push(self.in_const);
        if let TopLevelDecl::Variable(variable) = node {
            self.in_const |= variable.keyword() == VarKeyword::Const;
        }
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, _node: TopLevelDecl<'a>) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn visit_annotation(&mut self, _node: Annotation<'a>) -> StructuralTraversal {
        self.const_stack.push(self.in_const);
        self.in_const = true;
        StructuralTraversal::Continue
    }

    fn exit_annotation(&mut self, _node: Annotation<'a>) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn visit_pattern(&mut self, node: Pattern<'a>) -> StructuralTraversal {
        self.const_stack.push(self.in_const);
        self.in_const |= matches!(node, Pattern::Const(_));
        StructuralTraversal::Continue
    }

    fn exit_pattern(&mut self, _node: Pattern<'a>) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn visit_enum_decl(&mut self, _node: EnumDecl<'a>) -> StructuralTraversal {
        self.const_stack.push(self.in_const);
        self.in_const = true;
        StructuralTraversal::Continue
    }

    fn exit_enum_decl(&mut self, _node: EnumDecl<'a>) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn visit_class_member(&mut self, _node: ClassMember<'a>) -> StructuralTraversal {
        self.const_stack.push(self.in_const);
        self.in_const = false;
        StructuralTraversal::Continue
    }

    fn exit_class_member(&mut self, _node: ClassMember<'a>) {
        self.in_const = self.const_stack.pop().unwrap();
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
