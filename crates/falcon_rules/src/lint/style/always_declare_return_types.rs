//! Flags functions and methods declared without a return type.
//!
//! Without an explicit return type, a function returns `dynamic` (or an
//! inferred override type), which hides its contract from readers and callers.
//! Top-level and local functions, getters, methods, operators, and legacy
//! `typedef F(...)` aliases are checked. Setters and `operator []=` never
//! declare a return type and are skipped, as are `test_*`/`solo_test_*`
//! methods in a package-root test directory (reflective test runners).
//! Operator diagnostics highlight only the operator name, not `operator`.
//!
//! ```dart
//! main() {}      // flagged
//! void main() {} // ok
//! ```
//!
//! Adopted from the official `always_declare_return_types` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::declarations::{in_test_directory, operator_span};

const RULE: &str = "always-declare-return-types";

#[derive(Default)]
pub struct AlwaysDeclareReturnTypes;

impl Rule for AlwaysDeclareReturnTypes {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            ctx,
            diags: Vec::new(),
            in_test_directory: in_test_directory(ctx.file_path),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
    in_test_directory: bool,
}

impl Collector<'_, '_> {
    fn function(&mut self, name: Identifier<'_>) {
        self.diags.push(self.ctx.diagnostic(
            RULE,
            format!(
                "The function '{}' should have a return type but doesn't.",
                name.name()
            ),
            name.span(),
        ));
    }

    fn method(&mut self, name: &str, span: Span) {
        if self.in_test_directory && (name.starts_with("test_") || name.starts_with("solo_test_")) {
            return;
        }
        self.diags.push(self.ctx.diagnostic(
            RULE,
            format!("The method '{name}' should have a return type but doesn't."),
            span,
        ));
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        match node {
            TopLevelDecl::Function(function)
                if function.return_type().is_none()
                    && !matches!(function.kind(), FunctionKind::Setter(_)) =>
            {
                self.function(function.name());
            }
            // Only the legacy form `typedef F(int x);` has an omittable return
            // type; its synthesized function type starts at the alias name.
            TopLevelDecl::TypeAlias(alias) => {
                if let DartType::Function(function) = alias.aliased()
                    && function.return_type().is_none()
                    && function.span().start == alias.name().span().start
                {
                    self.function(alias.name());
                }
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        match node {
            ClassMember::Method(method) if method.return_type().is_none() => {
                self.method(method.name().name(), method.name().span());
            }
            ClassMember::Getter(getter) if getter.return_type().is_none() => {
                self.method(getter.name().name(), getter.name().span());
            }
            ClassMember::Operator(operator)
                if operator.return_type().is_none() && operator.op() != "[]=" =>
            {
                let span = operator_span(operator);
                self.method(operator.op(), span);
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node
            && function.return_type().is_none()
        {
            self.function(function.name());
        }
        StructuralTraversal::Continue
    }
}
}
