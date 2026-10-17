//! Flags declared identifiers that are shorter or longer than the allowed range.
//!
//! Very short names rarely convey meaning and very long ones hurt readability, so
//! this rule keeps declared names within a configurable length band. Its scope is
//! deliberately limited to variable and field declarations, getter and setter
//! names, and enum constants; parameters, catch-clause variables, for-each loop
//! variables, and plain function and method names are never checked, because
//! short names are often idiomatic there. A single leading underscore is stripped
//! before both the length and exception checks, so `_id` is judged as `id`.
//! Declarations in initializer closures, arrow bodies, operators, and enum
//! members follow the same policy as declarations in ordinary function bodies.
//!
//! ## Options
//!
//! `min_length` (int, default: 3) — flag identifiers shorter than this.
//! `max_length` (int, default: 300) — flag identifiers longer than this.
//! `exceptions` (list of strings, default: []) — names always allowed regardless of length.

use std::collections::HashSet;

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::bound_names;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferCorrectIdentifierLength;

// No built-in exceptions: dcl has none, and short-lived bindings are exempt by scope.
struct IdentCfg {
    min_length: usize,
    max_length: usize,
    exceptions: HashSet<String>,
}

fn ident_cfg(ctx: &AnalyzeContext) -> IdentCfg {
    let opts = crate::lint::options::for_rule(ctx, "prefer-correct-identifier-length");
    let min_length = crate::lint::options::usize(opts, "min_length").unwrap_or(3);
    let max_length = crate::lint::options::usize(opts, "max_length").unwrap_or(300);
    let exceptions = crate::lint::options::strings(opts, "exceptions")
        .into_iter()
        .flatten()
        .map(str::to_string)
        .collect();

    IdentCfg {
        min_length,
        max_length,
        exceptions,
    }
}

impl Rule for PreferCorrectIdentifierLength {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Names {
            diags: Vec::new(),
            ctx,
            cfg: ident_cfg(ctx),
        }))
    }
}

// Strip exactly one leading underscore, including when matching exceptions.
fn is_invalid(name: &str, cfg: &IdentCfg) -> bool {
    let stripped = name.strip_prefix('_').unwrap_or(name);
    if cfg.exceptions.contains(stripped) {
        return false;
    }
    let len = stripped.chars().count();
    len < cfg.min_length || len > cfg.max_length
}

fn check_ident(
    ident: Identifier<'_>,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    cfg: &IdentCfg,
) {
    if is_invalid(ident.name(), cfg) {
        diags.push(make_diag(ctx, &ident.span(), ident.name(), cfg));
    }
}

struct Names<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
    cfg: IdentCfg,
}

impl Names<'_, '_> {
    fn check(&mut self, ident: Identifier<'_>) {
        check_ident(ident, &mut self.diags, self.ctx, &self.cfg);
    }
}

structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Names<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Variable(variable) = node {
            for declarator in variable.declarators() {
                self.check(declarator.name());
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_enum_decl(&mut self, node: EnumDecl<'a>) -> StructuralTraversal {
        for variant in node.variants() {
            self.check(variant.name());
        }
        StructuralTraversal::Continue
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) -> StructuralTraversal {
        for declarator in node.declarators() {
            self.check(declarator.name());
        }
        StructuralTraversal::Continue
    }

    fn visit_getter_decl(&mut self, node: GetterDecl<'a>) -> StructuralTraversal {
        self.check(node.name());
        StructuralTraversal::Continue
    }

    fn visit_setter_decl(&mut self, node: SetterDecl<'a>) -> StructuralTraversal {
        self.check(node.name());
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) -> StructuralTraversal {
        for declarator in node.declarators() {
            self.check(declarator.name());
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::PatternDecl(declaration) = node {
            for name in bound_names(declaration.pattern()) {
                self.check(name);
            }
        }
        StructuralTraversal::Continue
    }
}
}

fn make_diag(ctx: &AnalyzeContext, span: &Span, name: &str, cfg: &IdentCfg) -> Diagnostic {
    let len = name.strip_prefix('_').unwrap_or(name).chars().count();
    let message = if len > cfg.max_length {
        format!(
            "The {name} identifier is {len} characters long. It's recommended to decrease it to {} chars long.",
            cfg.max_length
        )
    } else {
        format!(
            "The {name} identifier is {len} characters long. It's recommended to increase it up to {} chars long.",
            cfg.min_length
        )
    };
    ctx.diagnostic("prefer-correct-identifier-length", message, *span)
}
