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

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_annotation, walk_class_member, walk_enum_decl, walk_expr, walk_field_decl,
    walk_local_var, walk_pattern, walk_top_level_decl,
};

#[derive(Default)]
pub struct UseOnceConstructorsOnceProvider;

impl Rule for UseOnceConstructorsOnceProvider {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            in_const: false,
        };
        collector.visit_program(program);
        collector.diags
    }
}

const ONCE_PROVIDERS: &[&str] = &["OnceProvider", "FutureProvider", "StateProvider"];

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    in_const: bool,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
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
        let previous = self.in_const;
        self.in_const |= matches!(
            node,
            Expr::New { is_const: true, .. }
                | Expr::List { is_const: true, .. }
                | Expr::Set { is_const: true, .. }
                | Expr::Map { is_const: true, .. }
                | Expr::Record { is_const: true, .. }
        );
        walk_expr(self, node);
        self.in_const = previous;
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        let previous = self.in_const;
        self.in_const |= node.keyword == VarKeyword::Const;
        walk_local_var(self, node);
        self.in_const = previous;
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        let previous = self.in_const;
        self.in_const |= node.keyword == VarKeyword::Const;
        walk_field_decl(self, node);
        self.in_const = previous;
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        let previous = self.in_const;
        if let TopLevelDecl::Variable(variable) = node {
            self.in_const |= variable.keyword == VarKeyword::Const;
        }
        walk_top_level_decl(self, node);
        self.in_const = previous;
    }

    fn visit_annotation(&mut self, node: &Annotation) {
        let previous = self.in_const;
        self.in_const = true;
        walk_annotation(self, node);
        self.in_const = previous;
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        let previous = self.in_const;
        self.in_const |= matches!(node, Pattern::Const(_));
        walk_pattern(self, node);
        self.in_const = previous;
    }

    fn visit_enum_decl(&mut self, node: &EnumDecl) {
        let previous = self.in_const;
        self.in_const = true;
        walk_enum_decl(self, node);
        self.in_const = previous;
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        let previous = self.in_const;
        self.in_const = false;
        walk_class_member(self, node);
        self.in_const = previous;
    }
}
