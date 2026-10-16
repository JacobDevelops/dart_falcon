//! Flags boolean identifiers that lack a conventional interrogative prefix.
//!
//! A boolean named `visible` reads as a value, whereas `isVisible` reads as the
//! yes/no question it answers; a consistent prefix (`is`, `has`, `can`, ...)
//! makes conditions self-documenting. Only variables and fields initialized
//! with a boolean *literal*, and `bool`-returning methods, getters, and
//! functions, are checked — parameters and uninitialized fields are not. A
//! single leading underscore is stripped before matching, and `@override`
//! members are exempt because they cannot rename an inherited declaration.
//!
//! ## Options
//!
//! `valid_prefixes` (list of strings, default: `["is", "are", "was", "were",
//! "has", "have", "had", "can", "should", "will", "do", "does", "did"]`) —
//! accepted boolean-name prefixes. User-provided entries extend the defaults
//! rather than replacing them.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_function_decl, walk_local_var, walk_top_level_decl,
};

#[derive(Default)]
pub struct BooleanPrefixes;

impl Rule for BooleanPrefixes {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut visitor = Collector {
            diags: Vec::new(),
            ctx,
            prefixes: resolve_prefixes(ctx),
        };
        visitor.visit_program(program);
        RulePreparation::Diagnostics(visitor.diags)
    }
}

/// pyramid_lint's default valid prefixes. A user-provided `valid_prefixes`
/// option EXTENDS this list rather than replacing it.
const DEFAULT_PREFIXES: &[&str] = &[
    "is", "are", "was", "were", "has", "have", "had", "can", "should", "will", "do", "does", "did",
];

const MESSAGE: &str = "Boolean should be named with a valid prefix.";

fn resolve_prefixes(ctx: &AnalyzeContext) -> Vec<String> {
    let mut prefixes: Vec<String> = DEFAULT_PREFIXES.iter().map(|s| s.to_string()).collect();
    if let Some(list) = crate::lint::options::strings(
        crate::lint::options::for_rule(ctx, "boolean-prefixes"),
        "valid_prefixes",
    ) {
        prefixes.extend(list.map(str::to_string));
    }
    prefixes
}

fn is_bool_type(ty: Option<&DartType>) -> bool {
    matches!(ty, Some(DartType::Named(nt)) if nt.segments.len() == 1 && nt.segments[0].name == "bool")
}

use crate::member_order::is_override;

/// pyramid_lint strips a single leading underscore and then checks whether the
/// name simply *starts with* a valid prefix (no camelCase boundary is required,
/// mirroring `validPrefixes.any(name.startsWith)`).
fn has_valid_boolean_prefix(name: &str, prefixes: &[String]) -> bool {
    let name = name.strip_prefix('_').unwrap_or(name);
    prefixes
        .iter()
        .any(|prefix| name.starts_with(prefix.as_str()))
}

fn check_name(
    name: &Identifier,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    prefixes: &[String],
) {
    if !has_valid_boolean_prefix(&name.name, prefixes) {
        diags.push(ctx.diagnostic("boolean-prefixes", MESSAGE, name.span));
    }
}

/// A variable/field is only inspected when its initializer is a boolean *literal*
/// (`= true` / `= false`). pyramid_lint hooks `BooleanLiteral` nodes whose parent
/// is a `VariableDeclaration`, so an uninitialized `bool` field or one assigned a
/// non-literal expression (`= !kDebugMode`) is out of scope, and the declared
/// type is irrelevant.
fn check_declarators(
    declarators: &[VarDeclarator],
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    prefixes: &[String],
) {
    for d in declarators {
        if matches!(
            d.initializer.as_ref().map(Expr::unparenthesized),
            Some(Expr::BoolLit { .. })
        ) {
            check_name(&d.name, diags, ctx, prefixes);
        }
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
    prefixes: Vec<String>,
}

impl Visitor for Collector<'_, '_> {
    fn visit_top_level_decl(&mut self, decl: &TopLevelDecl) {
        if let TopLevelDecl::Variable(v) = decl {
            check_declarators(&v.declarators, &mut self.diags, self.ctx, &self.prefixes);
        }
        walk_top_level_decl(self, decl);
    }

    fn visit_function_decl(&mut self, f: &FunctionDecl) {
        if !matches!(f.kind, FunctionKind::Setter(_)) && is_bool_type(f.return_type.as_ref()) {
            check_name(&f.name, &mut self.diags, self.ctx, &self.prefixes);
        }
        walk_function_decl(self, f);
    }

    fn visit_class_member(&mut self, member: &ClassMember) {
        let named_return = match member {
            ClassMember::Field(f) => {
                check_declarators(&f.declarators, &mut self.diags, self.ctx, &self.prefixes);
                None
            }
            ClassMember::Method(m) if !is_override(&m.annotations) => {
                Some((&m.name, m.return_type.as_ref()))
            }
            ClassMember::Getter(g) if !is_override(&g.annotations) => {
                Some((&g.name, g.return_type.as_ref()))
            }
            _ => None,
        };
        if let Some((name, ty)) = named_return
            && is_bool_type(ty)
        {
            check_name(name, &mut self.diags, self.ctx, &self.prefixes);
        }
        walk_class_member(self, member);
    }

    fn visit_local_var(&mut self, local: &LocalVarDecl) {
        check_declarators(
            &local.declarators,
            &mut self.diags,
            self.ctx,
            &self.prefixes,
        );
        walk_local_var(self, local);
    }
}
