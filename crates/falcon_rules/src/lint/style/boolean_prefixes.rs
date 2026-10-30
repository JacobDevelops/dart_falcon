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
//! `valid-prefixes` (list of strings, default: `["is", "are", "was", "were",
//! "has", "have", "had", "can", "should", "will", "do", "does", "did"]`) —
//! accepted boolean-name prefixes. User-provided entries extend the defaults
//! rather than replacing them.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct BooleanPrefixes;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Additional accepted prefixes for boolean names; these extend built-ins.
    valid_prefixes: Vec<String>,
}

crate::lint::options::impl_options!(Options);

impl Rule for BooleanPrefixes {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let visitor = Collector {
            diags: Vec::new(),
            ctx,
            model: falcon_analyze::SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            prefixes: &crate::lint::options::get::<Options>(ctx, Self::NAME).valid_prefixes,
        };
        RulePreparation::Structural(Box::new(visitor))
    }
}

/// `pyramid_lint`'s default valid prefixes. A user-provided `valid-prefixes`
/// option EXTENDS this list rather than replacing it.
const DEFAULT_PREFIXES: &[&str] = &[
    "is", "are", "was", "were", "has", "have", "had", "can", "should", "will", "do", "does", "did",
];

const MESSAGE: &str = "Boolean should be named with a valid prefix.";

fn is_bool_type(ty: Option<DartType<'_>>) -> bool {
    let Some(DartType::Named(named)) = ty else {
        return false;
    };
    let mut segments = named.segments();
    matches!((segments.next(), segments.next()), (Some(segment), None) if segment.name() == "bool")
}

use crate::lint::support::annotations::has_override;

/// `pyramid_lint` strips a single leading underscore and then checks whether the
/// name simply *starts with* a valid prefix (no camelCase boundary is required,
/// mirroring `validPrefixes.any(name.startsWith)`).
fn has_valid_boolean_prefix(name: &str, prefixes: &[String]) -> bool {
    let name = name.strip_prefix('_').unwrap_or(name);
    DEFAULT_PREFIXES
        .iter()
        .copied()
        .chain(prefixes.iter().map(String::as_str))
        .any(|prefix| name.starts_with(prefix))
}

fn check_name(
    name: Identifier<'_>,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    prefixes: &[String],
) {
    if !has_valid_boolean_prefix(name.name(), prefixes) {
        diags.push(ctx.diagnostic(MESSAGE, name.span()));
    }
}

/// A variable/field is only inspected when its initializer is a boolean *literal*
/// (`= true` / `= false`). `pyramid_lint` hooks `BooleanLiteral` nodes whose parent
/// is a `VariableDeclaration`, so an uninitialized `bool` field or one assigned a
/// non-literal expression (`= !kDebugMode`) is out of scope, and the declared
/// type is irrelevant.
fn check_declarators(
    declarators: AstChildren<'_, VarDeclarator<'_>>,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    prefixes: &[String],
) {
    for d in declarators {
        if matches!(
            d.initializer().map(Expr::unparenthesized),
            Some(Expr::BoolLit(_))
        ) {
            check_name(d.name(), diags, ctx, prefixes);
        }
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    model: falcon_analyze::SemanticModel<'a>,
    prefixes: &'a [String],
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Variable(v) = decl {
            check_declarators(v.declarators(), &mut self.diags, self.ctx, self.prefixes);
        }
        StructuralTraversal::Continue
    }

    fn visit_function_decl(&mut self, f: FunctionDecl<'a>) -> StructuralTraversal {
        if !matches!(f.kind(), FunctionKind::Setter(_)) && is_bool_type(f.return_type()) {
            check_name(f.name(), &mut self.diags, self.ctx, self.prefixes);
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, member: ClassMember<'a>) -> StructuralTraversal {
        let named_return = match member {
            ClassMember::Field(f) => {
                check_declarators(f.declarators(), &mut self.diags, self.ctx, self.prefixes);
                None
            }
            ClassMember::Method(m) if !has_override(m.annotations(), self.model) => {
                Some((m.name(), m.return_type()))
            }
            ClassMember::Getter(g) if !has_override(g.annotations(), self.model) => {
                Some((g.name(), g.return_type()))
            }
            _ => None,
        };
        if let Some((name, ty)) = named_return
            && is_bool_type(ty)
        {
            check_name(name, &mut self.diags, self.ctx, self.prefixes);
        }
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, local: LocalVarDecl<'a>) -> StructuralTraversal {
        check_declarators(
            local.declarators(),
            &mut self.diags,
            self.ctx,
            self.prefixes,
        );
        StructuralTraversal::Continue
    }
}
}
